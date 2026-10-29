use super::mapping;
use crate::engine::{
    EditorEngine, EditorSelection, InlineContainerRef, TextPoint, TextRange, model,
};
use wasm_bindgen::JsCast;
use web_sys::Element;
pub fn read(root: &Element) -> Option<EditorSelection> {
    let sel = root.owner_document()?.get_selection().ok()??;
    let anchor = mapping::from_dom(root, &sel.anchor_node()?, sel.anchor_offset())?;
    let head = mapping::from_dom(root, &sel.focus_node()?, sel.focus_offset())?;
    Some(EditorSelection::Text(TextRange { anchor, head }))
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
    let selection = engine.selection();
    let hide = !matches!(selection, EditorSelection::Text(_));
    let previous_hide = root
        .class_list()
        .contains("dxeditor-engine--hide-selection");
    let _ = root
        .class_list()
        .toggle_with_force("dxeditor-engine--hide-selection", hide);
    if hide || previous_hide {
        if let Ok(nodes) = root.query_selector_all("[data-selected]") {
            for i in 0..nodes.length() {
                if let Some(e) = nodes.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                    if mapping::owns_node(root, e.as_ref()) {
                        let _ = e.remove_attribute("data-selected");
                    }
                }
            }
        }
    }
    let range = match selection {
        EditorSelection::Text(r) => Some(r.clone()),
        EditorSelection::Node(uid) => {
            mark(root, uid);
            None
        }
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
            Some(TextRange::caret(TextPoint::new(
                InlineContainerRef::Block(anchor.clone()),
                0,
            )))
        }
        EditorSelection::Cells {
            table,
            anchor,
            head,
        } => {
            if let Ok(Some(table)) = root.query_selector(&format!("[data-uid='{table}'] table")) {
                let rows = table.query_selector_all("tr").ok();
                if let Some(rows) = rows {
                    for r in anchor.0.min(head.0)..=anchor.0.max(head.0) {
                        if let Some(row) = rows
                            .item(r as u32)
                            .and_then(|n| n.dyn_into::<Element>().ok())
                        {
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
            None
        }
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
