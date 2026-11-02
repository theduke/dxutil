//! Block hit testing uses viewport geometry, including the document's gutter
//! and gaps. The engine performs the move, preserving identities and history.
use super::{Host, clipboard};
use crate::engine::{commands::BlockPlacement, model};
use wasm_bindgen::JsCast;
use web_sys::{DomRect, Element};

pub(super) fn clear_target(host: &Host) {
    let mut current = host.borrow_mut();
    current.drop_target = None;
    if let Some(indicator) = current.drag_indicator.take() {
        indicator.remove();
    }
}

pub(super) fn update_target(host: &Host, x: f64, y: f64) {
    let target = target(host, x, y);
    if let Some((uid, placement, rect)) = target {
        let inside = placement == BlockPlacement::Inside;
        let indent = if inside { 24.0 } else { 0.0 };
        let line_y = if placement == BlockPlacement::Before {
            rect.top()
        } else {
            rect.bottom()
        };
        clipboard::show_indicator(host, rect.x() + indent, line_y, rect.width() - indent);
        host.borrow_mut().drop_target = Some((uid, placement));
    } else {
        clear_target(host);
    }
}

fn target(host: &Host, x: f64, y: f64) -> Option<(String, BlockPlacement, DomRect)> {
    let current = host.borrow();
    let engine = current.engine.borrow();
    let blocks = current
        .root
        .query_selector_all(".dx-block[data-uid]")
        .ok()?;
    // Measure each block's own content, rather than its entire subtree, so
    // parents do not swallow the hit areas of their children.
    let (block, uid, content_rect, _) = (0..blocks.length())
        .filter_map(|index| {
            let block = blocks.item(index)?.dyn_into::<Element>().ok()?;
            let uid = block.get_attribute("data-uid")?;
            if current.renderer.blocks.get(&uid) != Some(&block) {
                return None; // A nested editor owns its own blocks.
            }
            let rect = block.first_element_child()?.get_bounding_client_rect();
            if rect.height() <= 0.0 || rect.width() <= 0.0 {
                return None; // Collapsed descendants are not drop targets.
            }
            let distance = (rect.top() - y).max(y - rect.bottom()).max(0.0);
            Some((block, uid, rect, distance))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))?;
    // Reject the source and its descendants before drawing any promise of a
    // drop. The command also enforces this invariant independently.
    if current.drag_uids.iter().any(|source| {
        engine
            .block(source)
            .is_some_and(|node| model::find(node, &uid).is_some())
    }) {
        return None;
    }
    let inside = x > content_rect.x() + content_rect.width() / 2.0
        && y > content_rect.top() + content_rect.height() * 0.25
        && y < content_rect.bottom() - content_rect.height() * 0.25
        && !engine.is_collapsed(&uid)
        && model::content(engine.document(), &uid)
            .is_some_and(|node| engine.schema().allows_children(&node.kind.0));
    let placement = if inside {
        BlockPlacement::Inside
    } else if y < content_rect.y() + content_rect.height() / 2.0 {
        BlockPlacement::Before
    } else {
        BlockPlacement::After
    };
    // After means after the whole subtree; Inside means below the parent's
    // own content, with an indented line above its first child.
    let rect = if inside {
        content_rect
    } else {
        block.get_bounding_client_rect()
    };
    Some((uid, placement, rect))
}

/// Native dragover repeats while the pointer rests at an edge. Scroll the
/// nearest available container, then fall through to the page at its limits.
pub(super) fn autoscroll(host: &Host, x: f64, y: f64) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let root = host.borrow().root.clone();
    let page = root
        .owner_document()
        .and_then(|doc| doc.scrolling_element());
    let mut ancestor = Some(root);
    while let Some(element) = ancestor {
        ancestor = element.parent_element();
        let Some(html) = element.dyn_ref::<web_sys::HtmlElement>() else {
            continue;
        };
        if element.scroll_height() <= element.client_height() {
            continue;
        }
        let is_page = page.as_ref() == Some(&element);
        let scrollable = is_page
            || window
                .get_computed_style(&element)
                .ok()
                .flatten()
                .and_then(|style| style.get_property_value("overflow-y").ok())
                .is_some_and(|value| matches!(value.as_str(), "auto" | "scroll"));
        if !scrollable {
            continue;
        }
        let rect = element.get_bounding_client_rect();
        let (top, bottom) = if is_page {
            (
                0.0,
                window
                    .inner_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(800.0),
            )
        } else {
            if x < rect.left() || x > rect.right() {
                continue;
            }
            (
                rect.top().max(0.0),
                rect.bottom().min(
                    window
                        .inner_height()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(800.0),
                ),
            )
        };
        let edge = 48.0_f64.min((bottom - top) / 3.0);
        if edge <= 0.0 {
            continue;
        }
        let delta = if y < top + edge {
            -((top + edge - y) / edge).clamp(0.0, 1.0) * 24.0
        } else if y > bottom - edge {
            ((y - bottom + edge) / edge).clamp(0.0, 1.0) * 24.0
        } else {
            0.0
        };
        if delta == 0.0 {
            return;
        }
        let before = element.scroll_top();
        html.set_scroll_top(before + delta.round() as i32);
        if element.scroll_top() != before {
            return;
        }
    }
}
