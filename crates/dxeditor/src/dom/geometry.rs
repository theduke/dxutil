use crate::engine::{
    EditorEngine,
    ui_state::{Geometry, Rect},
};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{Document, Element, Node, Range};
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends=Document,js_name=Document)]
    type CaretDocument;
    #[wasm_bindgen(method,js_name=caretRangeFromPoint)]
    fn caret_range_from_point(this: &CaretDocument, x: f32, y: f32) -> Option<Range>;
}
fn rect(r: web_sys::DomRect) -> Rect {
    Rect {
        x: r.x(),
        y: r.y(),
        width: r.width(),
        height: r.height(),
    }
}
fn visible_range_rect(range: &Range) -> Option<Rect> {
    let bounds = rect(range.get_bounding_client_rect());
    (bounds.height > 0.
        && bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite())
    .then_some(bounds)
}

/// Screen coordinates of the caret, including empty lines after hard breaks.
pub(super) fn caret_rect(root: &Element, engine: &EditorEngine) -> Option<Rect> {
    let point = &engine.selection().text_range()?.head;
    let (node, offset) = super::mapping::to_dom(root, point)?;
    let range = root.owner_document()?.create_range().ok()?;
    range.set_start(&node, offset).ok()?;
    range.collapse_with_to_start(true);
    if let Some(rect) = visible_range_rect(&range) {
        return Some(rect);
    }
    // A boundary after BR is parent-anchored. The renderer keeps a trailing
    // text anchor on the new line; measure it without rewriting selection.
    if let Some(next) = node.child_nodes().item(offset) {
        range.set_start(&next, 0).ok()?;
        range.collapse_with_to_start(true);
        if let Some(rect) = visible_range_rect(&range) {
            return Some(rect);
        }
    }
    // Empty paragraphs contain a placeholder BR with no measurable range.
    let (inline, _) = super::mapping::container(&node, root)?;
    let bounds = rect(inline.get_bounding_client_rect());
    (bounds.height > 0.0).then_some(bounds)
}
pub fn measure(root: &Element, e: &EditorEngine) -> Geometry {
    let wrapper = root
        .closest(".dxeditor, .dx-composer-editor")
        .ok()
        .flatten()
        .unwrap_or_else(|| root.clone());
    let wr = wrapper.get_bounding_client_rect();
    let (sx, sy) = wrapper
        .dyn_ref::<web_sys::HtmlElement>()
        .map(|h| {
            (
                if h.offset_width() > 0 {
                    wr.width() / h.offset_width() as f64
                } else {
                    1.
                },
                if h.offset_height() > 0 {
                    wr.height() / h.offset_height() as f64
                } else {
                    1.
                },
            )
        })
        .unwrap_or((1., 1.));
    let local = |r: Rect| Rect {
        x: (r.x - wr.x()) / sx,
        y: (r.y - wr.y()) / sy,
        width: r.width / sx,
        height: r.height / sy,
    };
    let win = web_sys::window().unwrap();
    let mut viewport = if let Some(v) = win.visual_viewport() {
        Rect {
            x: v.offset_left(),
            y: v.offset_top(),
            width: v.width(),
            height: v.height(),
        }
    } else {
        Rect {
            x: 0.,
            y: 0.,
            width: win
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(1024.),
            height: win
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(768.),
        }
    };
    // Menus stay within scroll/clip ancestors as well as the visual viewport.
    let mut ancestor = wrapper.parent_element();
    while let Some(element) = ancestor {
        if let Ok(Some(style)) = win.get_computed_style(&element) {
            let bounds = element.get_bounding_client_rect();
            let clips = |axis: &str| {
                style.get_property_value(axis).ok().is_some_and(|value| {
                    matches!(value.as_str(), "auto" | "hidden" | "scroll" | "clip")
                })
            };
            if clips("overflow-x") {
                let right = (viewport.x + viewport.width).min(bounds.right());
                viewport.x = viewport.x.max(bounds.left());
                viewport.width = (right - viewport.x).max(0.);
            }
            if clips("overflow-y") {
                let bottom = (viewport.y + viewport.height).min(bounds.bottom());
                viewport.y = viewport.y.max(bounds.top());
                viewport.height = (bottom - viewport.y).max(0.);
            }
        }
        ancestor = element.parent_element();
    }
    let selection = root
        .owner_document()
        .and_then(|d| d.get_selection().ok().flatten())
        .and_then(|s| s.get_range_at(0).ok())
        .filter(|r| {
            r.common_ancestor_container()
                .ok()
                .is_some_and(|n| root.contains(Some(&n)))
        })
        .and_then(|r| visible_range_rect(&r))
        .map(local);
    // Stable end carets are parent-anchored to avoid browser selection layout
    // during typing. Chromium reports an empty rectangle at those boundaries;
    // measure an equivalent text-node range without changing native selection.
    let caret = selection.or_else(|| caret_rect(root, e).map(local));
    let block = e
        .ui_state()
        .hover_block
        .or_else(|| crate::engine::commands::blocks::current_uid(e))
        .and_then(|u| {
            root.query_selector(&format!("[data-uid='{u}']"))
                .ok()
                .flatten()
        })
        .map(|el| local(rect(el.get_bounding_client_rect())));
    let table = crate::engine::commands::blocks::current_uid(e)
        .and_then(|u| {
            root.query_selector(&format!("[data-uid='{u}'] table"))
                .ok()
                .flatten()
        })
        .map(|el| local(rect(el.get_bounding_client_rect())));
    {
        let table_element = crate::engine::commands::blocks::current_uid(e).and_then(|u| {
            root.query_selector(&format!("[data-uid='{u}'] table"))
                .ok()
                .flatten()
        });
        let mut table_rows = Vec::new();
        let mut table_columns = Vec::new();
        if let Some(table) = table_element {
            if let Ok(rows) = table.query_selector_all("tr") {
                for i in 0..rows.length() {
                    if let Some(row) = rows.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                        table_rows.push(local(rect(row.get_bounding_client_rect())));
                        if i == 0 {
                            let cells = row.children();
                            for c in 0..cells.length() {
                                if let Some(cell) = cells.item(c) {
                                    table_columns
                                        .push(local(rect(cell.get_bounding_client_rect())));
                                }
                            }
                        }
                    }
                }
            }
        }
        let cell = e
            .selection()
            .text_range()
            .and_then(|r| super::mapping::to_dom(root, &r.head))
            .and_then(|(node, _)| {
                node.dyn_ref::<Element>()
                    .cloned()
                    .or_else(|| node.parent_element())
            })
            .and_then(|el| el.closest("td,th").ok().flatten())
            .map(|el| local(rect(el.get_bounding_client_rect())));
        Geometry {
            selection,
            caret: caret.or(block),
            block,
            table,
            table_rows,
            table_columns,
            cell,
            viewport: local(viewport),
            scale_x: sx,
            scale_y: sy,
        }
    }
}
pub fn caret_from_point(root: &Element, x: f64, y: f64) -> Option<crate::engine::TextPoint> {
    let doc = root.owner_document()?;
    if js_sys::Reflect::has(doc.as_ref(), &"caretPositionFromPoint".into()).unwrap_or(false) {
        if let Some(p) = doc.caret_position_from_point(x as f32, y as f32) {
            return super::mapping::from_dom(root, &p.offset_node()?, p.offset());
        }
    }
    if js_sys::Reflect::has(doc.as_ref(), &"caretRangeFromPoint".into()).unwrap_or(false) {
        let r = doc
            .unchecked_ref::<CaretDocument>()
            .caret_range_from_point(x as f32, y as f32)?;
        return super::mapping::from_dom(root, &r.start_container().ok()?, r.start_offset().ok()?);
    }
    None
}
pub fn element_at(root: &Element, node: &Node) -> Option<Element> {
    let e = node
        .dyn_ref::<Element>()
        .cloned()
        .or_else(|| node.parent_element())?;
    let found = e.closest(".dx-block[data-uid]").ok()??;
    root.contains(Some(&found)).then_some(found)
}
