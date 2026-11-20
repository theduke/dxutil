use super::mapping;
use crate::engine::{
    EditorEngine, EditorSelection, InlineContainerRef, TextPoint, TextRange, model,
};
use wasm_bindgen::JsCast;
use web_sys::Element;
pub fn read(root: &Element) -> Option<EditorSelection> {
    let sel = root.owner_document()?.get_selection().ok()??;
    let anchor_node = sel.anchor_node()?;
    let focus_node = sel.focus_node()?;
    let mut anchor = mapping::from_dom(root, &anchor_node, sel.anchor_offset())?;
    let mut head = mapping::from_dom(root, &focus_node, sel.focus_offset())?;
    // Browsers extend word and line selections (double and triple clicks) past
    // the end of a block to the start of the next one. Such a range ends at
    // the end of the text it visibly covers, so formatting and the block type
    // follow the selected text rather than the block after it.
    if anchor.container != head.container {
        let forward = sel.get_range_at(0).ok().is_some_and(|range| {
            range.start_container().ok().as_ref() == Some(&anchor_node)
                && range.start_offset().ok() == Some(sel.anchor_offset())
        });
        let end = if forward { &mut head } else { &mut anchor };
        if end.offset == 0 {
            let end_node = if forward { &focus_node } else { &anchor_node };
            if let Some(previous) = previous_container_end(root, end_node) {
                *end = previous;
            }
        }
    }
    Some(EditorSelection::Text(TextRange { anchor, head }))
}
/// The end of the inline container before the one holding `node`.
fn previous_container_end(root: &Element, node: &web_sys::Node) -> Option<TextPoint> {
    let (element, container) = mapping::container(node, root)?;
    if !matches!(container, InlineContainerRef::Block(_)) {
        return None;
    }
    let block = element.closest(".dx-block[data-uid]").ok()??;
    let previous = block.previous_element_sibling()?;
    if !previous.matches(".dx-block[data-uid]").ok()? {
        return None;
    }
    let content = previous.first_element_child()?;
    let inline = if content.has_attribute("data-inline-container") {
        content
    } else {
        content.query_selector("[data-inline-container]").ok()??
    };
    let container = serde_json::from_str(&inline.get_attribute("data-inline-container")?).ok()?;
    if !matches!(&container, InlineContainerRef::Block(uid) if Some(uid.as_str()) == previous.get_attribute("data-uid").as_deref())
    {
        return None;
    }
    Some(TextPoint::new(container, mapping::length(inline.as_ref())))
}
pub fn write(root: &Element, engine: &EditorEngine) {
    write_with(root, engine, |point| mapping::to_dom(root, point));
}
pub fn write_with(
    root: &Element,
    engine: &EditorEngine,
    resolve: impl Fn(&TextPoint) -> Option<(web_sys::Node, u32)>,
) {
    let Some(doc) = root.owner_document() else {
        return;
    };
    let Ok(Some(sel)) = doc.get_selection() else {
        return;
    };
    let range = match engine.selection() {
        EditorSelection::Text(range) => Some(range.clone()),
        EditorSelection::Blocks { anchor, .. } => Some(TextRange::caret(TextPoint::new(
            InlineContainerRef::Block(anchor.clone()),
            0,
        ))),
        EditorSelection::Node(_) | EditorSelection::Cells { .. } => None,
    };
    if let Some(range) = range {
        if let (Some((mut a, mut ao)), Some((mut h, mut ho))) =
            (resolve(&range.anchor), resolve(&range.head))
        {
            if range.is_collapsed() {
                (a, ao) = stable_end_caret(a, ao);
                h = a.clone();
                ho = ao;
            }
            if sel.anchor_node().as_ref() != Some(&a)
                || sel.anchor_offset() != ao
                || sel.focus_node().as_ref() != Some(&h)
                || sel.focus_offset() != ho
            {
                let _ = sel.set_base_and_extent(&a, ao, &h, ho);
            }
        }
    } else {
        let _ = sel.collapse_with_offset(Some(root.as_ref()), 0);
    }
}
/// Mirrors the engine's non-text selection onto the DOM: selected blocks,
/// nodes and table cells carry `data-selected`, and the native selection is
/// hidden while one of them is active.
///
/// Stale marks are always cleared first, so the decorations follow every
/// selection change, including ones the browser made natively (a click back
/// into text after selecting table cells).
pub fn decorate(root: &Element, engine: &EditorEngine) {
    let selection = engine.selection();
    let _ = root.class_list().toggle_with_force(
        "dxeditor-engine--hide-selection",
        !matches!(selection, EditorSelection::Text(_)),
    );
    if let Ok(nodes) = root.query_selector_all("[data-selected]") {
        for i in 0..nodes.length() {
            if let Some(e) = nodes.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                if mapping::owns_node(root, e.as_ref()) {
                    let _ = e.remove_attribute("data-selected");
                }
            }
        }
    }
    match selection {
        EditorSelection::Text(_) => {}
        EditorSelection::Node(uid) => mark(root, uid),
        EditorSelection::Blocks { anchor, head } => {
            let order = model::block_order(engine.document());
            if let (Some(a), Some(b)) = (
                order.iter().position(|x| x == anchor),
                order.iter().position(|x| x == head),
            ) {
                for uid in &order[a.min(b)..=a.max(b)] {
                    mark(root, uid);
                }
            }
        }
        EditorSelection::Cells {
            table,
            anchor,
            head,
        } => {
            let Ok(Some(table)) = root.query_selector(&format!("[data-uid='{table}'] table"))
            else {
                return;
            };
            let Ok(rows) = table.query_selector_all("tr") else {
                return;
            };
            for r in anchor.0.min(head.0)..=anchor.0.max(head.0) {
                let Some(row) = rows
                    .item(r as u32)
                    .and_then(|n| n.dyn_into::<Element>().ok())
                else {
                    continue;
                };
                let cells = row.children();
                for c in anchor.1.min(head.1)..=anchor.1.max(head.1) {
                    if let Some(cell) = cells.item(c as u32) {
                        let _ = cell.set_attribute("data-selected", "");
                    }
                }
            }
        }
    }
}
fn mark(root: &Element, uid: &str) {
    if let Ok(Some(el)) = root.query_selector(&format!("[data-uid='{uid}']")) {
        let _ = el.set_attribute("data-selected", "");
    }
}

/// Parent-anchored end carets survive CharacterData patches without a native
/// selection rewrite (which forces whole-editable layout in some browsers).
fn stable_end_caret(node: web_sys::Node, offset: u32) -> (web_sys::Node, u32) {
    if node
        .dyn_ref::<web_sys::Text>()
        .is_some_and(|text| offset == text.length())
    {
        if let Some(parent) = node.parent_node() {
            let children = parent.child_nodes();
            if let Some(index) =
                (0..children.length()).find(|&index| children.item(index).as_ref() == Some(&node))
            {
                return (parent, index + 1);
            }
        }
    }
    (node, offset)
}
