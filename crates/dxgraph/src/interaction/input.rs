//! Dioxus events enter the pure gesture reducer through this adapter.
use super::gesture::{GestureInput, GestureTarget};
use crate::{Point, Size};
use dioxus::html::{geometry::WheelDelta, input_data::MouseButton};
use dioxus::prelude::*;

pub fn pointer_down(data: &PointerData, target: GestureTarget, timestamp_ms: u64) -> GestureInput {
    let client = data.client_coordinates();
    GestureInput::PointerDown {
        pointer: data.pointer_id(),
        client: Point {
            x: client.x,
            y: client.y,
        },
        target,
        shift: data.modifiers().shift(),
        primary: data
            .trigger_button()
            .is_none_or(|button| button == MouseButton::Primary),
        timestamp_ms,
    }
}
pub fn pointer_move(data: &PointerData) -> GestureInput {
    let p = data.client_coordinates();
    GestureInput::PointerMove {
        pointer: data.pointer_id(),
        client: Point { x: p.x, y: p.y },
    }
}
pub fn pointer_up(data: &PointerData, timestamp_ms: u64) -> GestureInput {
    let p = data.client_coordinates();
    GestureInput::PointerUp {
        pointer: data.pointer_id(),
        client: Point { x: p.x, y: p.y },
        timestamp_ms,
    }
}
pub fn wheel(data: &WheelData, container: Size) -> GestureInput {
    let p = data.client_coordinates();
    GestureInput::Wheel {
        client: Point { x: p.x, y: p.y },
        delta: normalize_wheel(data.delta(), container),
        ctrl: data.modifiers().ctrl() || data.modifiers().meta(),
    }
}
pub fn normalize_wheel(delta: WheelDelta, container: Size) -> Point {
    let multiplier = match delta {
        WheelDelta::Pixels(_) => 1.0,
        WheelDelta::Lines(_) => 16.0,
        WheelDelta::Pages(_) => container.height,
    };
    let delta = delta.strip_units();
    Point {
        x: delta.x * multiplier,
        y: delta.y * multiplier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_units() {
        let size = Size {
            width: 200.0,
            height: 300.0,
        };
        assert_eq!(
            normalize_wheel(WheelDelta::lines(1.0, 2.0, 0.0), size),
            Point { x: 16.0, y: 32.0 }
        );
        assert_eq!(
            normalize_wheel(WheelDelta::pages(1.0, 2.0, 0.0), size),
            Point { x: 300.0, y: 600.0 }
        );
    }
}
