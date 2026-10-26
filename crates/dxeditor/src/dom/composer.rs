//! Composer atom deletion uses a visible two-step selection: the first
//! Backspace selects the chip, the second removes it and informs the host.
use super::{DomHost, Host, mapping};
use crate::engine::{Command, EditorSelection, Profile, TextPoint, TextRange, model, text};
use crate::profiles::composer::{ComposerEvent, chip_from_node};
use wasm_bindgen::JsCast;
use web_sys::{Element, InputEvent};

pub(crate) fn beforeinput(host: &Host, input: &InputEvent) -> bool {
    if !input.cancelable()
        || !matches!(
            input.input_type().as_str(),
            "deleteContentBackward" | "deleteContentForward" | "deleteByCut"
        )
    {
        return false;
    }
    let (root, range, nodes) = {
        let current = host.borrow();
        let engine = current.engine.borrow();
        if engine.profile() != Profile::Composer || engine.read_only() {
            return false;
        }
        let Some(range) = engine.selection().text_range() else {
            return false;
        };
        if range.anchor.container != range.head.container {
            return false;
        }
        let Some(nodes) = model::inline(engine.document(), &range.head.container) else {
            return false;
        };
        (current.root.clone(), range.clone(), nodes.clone())
    };
    let start = range.anchor.offset.min(range.head.offset);
    let end = range.anchor.offset.max(range.head.offset);
    if start != end {
        let removed = text::slice(&nodes, start, end)
            .iter()
            .filter_map(chip_from_node)
            .collect::<Vec<_>>();
        if removed.is_empty() {
            return false;
        }
        input.prevent_default();
        DomHost::run(
            host,
            Command::ReplaceRange {
                range,
                text: String::new(),
            },
        );
        clear_selected(&root);
        if let Some(callback) = host.borrow().composer_callback.clone() {
            for chip in removed {
                callback(ComposerEvent::ChipRemoved(chip));
            }
        }
        return true;
    }
    if input.input_type() != "deleteContentBackward" || start == 0 {
        return false;
    }
    let previous = text::slice(&nodes, start - 1, start);
    if previous.len() != 1 || chip_from_node(&previous[0]).is_none() {
        return false;
    }
    input.prevent_default();
    let selected = TextRange {
        anchor: TextPoint::new(range.head.container.clone(), start - 1),
        head: TextPoint::new(range.head.container, start),
    };
    DomHost::run(
        host,
        Command::SetSelection(EditorSelection::Text(selected.clone())),
    );
    clear_selected(&root);
    if let Ok(atoms) = root.query_selector_all("[data-composer-chip]") {
        for index in 0..atoms.length() {
            let Some(atom) = atoms
                .item(index)
                .and_then(|node| node.dyn_into::<Element>().ok())
            else {
                continue;
            };
            let Some(parent) = atom.parent_node() else {
                continue;
            };
            let children = parent.child_nodes();
            let Some(child_index) = (0..children.length())
                .find(|index| children.item(*index).as_ref() == Some(atom.as_ref()))
            else {
                continue;
            };
            if mapping::from_dom(&root, &parent, child_index).as_ref() == Some(&selected.anchor) {
                let _ = atom.set_attribute("data-selected", "");
                break;
            }
        }
    }
    true
}

fn clear_selected(root: &Element) {
    if let Ok(selected) = root.query_selector_all("[data-composer-chip][data-selected]") {
        for index in 0..selected.length() {
            if let Some(element) = selected
                .item(index)
                .and_then(|node| node.dyn_into::<Element>().ok())
            {
                let _ = element.remove_attribute("data-selected");
            }
        }
    }
}
