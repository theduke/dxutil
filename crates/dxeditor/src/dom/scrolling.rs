//! Keep an edited caret visible without moving focus or adding DOM markers.
use super::geometry;
use crate::engine::EditorEngine;
use wasm_bindgen::JsCast;
use web_sys::Element;

pub(super) fn reveal_selection(root: &Element, engine: &EditorEngine) {
    if !engine
        .selection()
        .text_range()
        .is_some_and(|r| r.is_collapsed())
    {
        return;
    }
    let Some(document) = root.owner_document() else {
        return;
    };
    let Some(window) = document.default_view() else {
        return;
    };
    let Some(caret) = geometry::caret_rect(root, engine) else {
        return;
    };
    let mut top = caret.y;
    let mut bottom = caret.y + caret.height;
    let mut left = caret.x;
    let mut right = caret.x + caret.width;
    let scrolling = document.scrolling_element();
    let mut ancestor = Some(root.clone());
    while let Some(element) = ancestor {
        ancestor = element.parent_element();
        let page = scrolling.as_ref() == Some(&element);
        let Some(html) = element.dyn_ref::<web_sys::HtmlElement>() else {
            continue;
        };
        let Ok(Some(style)) = window.get_computed_style(&element) else {
            continue;
        };
        let scrollable = |axis| {
            page || style
                .get_property_value(axis)
                .ok()
                .is_some_and(|value| matches!(value.as_str(), "auto" | "scroll"))
        };
        let bounds = element.get_bounding_client_rect();
        let sx = if html.offset_width() > 0 {
            bounds.width() / f64::from(html.offset_width())
        } else {
            1.0
        };
        let sy = if html.offset_height() > 0 {
            bounds.height() / f64::from(html.offset_height())
        } else {
            1.0
        };
        if sx <= 0.0 || sy <= 0.0 {
            continue;
        }
        let viewport_left = if page {
            0.0
        } else {
            bounds.left() + f64::from(element.client_left()) * sx
        };
        let viewport_top = if page {
            0.0
        } else {
            bounds.top() + f64::from(element.client_top()) * sy
        };
        let viewport_right =
            viewport_left + f64::from(element.client_width()) * if page { 1.0 } else { sx };
        let viewport_bottom =
            viewport_top + f64::from(element.client_height()) * if page { 1.0 } else { sy };
        if scrollable("overflow-y") && element.scroll_height() > element.client_height() {
            let delta = outside(top, bottom, viewport_top, viewport_bottom) / sy;
            let before = element.scroll_top();
            element.set_scroll_top(before + delta.round() as i32);
            let moved = f64::from(element.scroll_top() - before) * sy;
            top -= moved;
            bottom -= moved;
        }
        if scrollable("overflow-x") && element.scroll_width() > element.client_width() {
            let delta = outside(left, right, viewport_left, viewport_right) / sx;
            let before = element.scroll_left();
            element.set_scroll_left(before + delta.round() as i32);
            let moved = f64::from(element.scroll_left() - before) * sx;
            left -= moved;
            right -= moved;
        }
    }
}

fn outside(start: f64, end: f64, min: f64, max: f64) -> f64 {
    if start < min {
        start - min
    } else if end > max {
        // A line taller than the viewport keeps its start in view.
        (end - max).min((start - min).max(0.0))
    } else {
        0.0
    }
}
