use crate::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyboardAction {
    ClearSelection,
    ZoomBy(f64),
    Fit,
    Pan(Point),
    Activate,
    Select,
}

/// Uses standard keyboard key names, independent of event/rendering types.
pub fn keyboard_action(key: &str) -> Option<KeyboardAction> {
    match key {
        "Escape" => Some(KeyboardAction::ClearSelection),
        "+" | "=" => Some(KeyboardAction::ZoomBy(1.2)),
        "-" => Some(KeyboardAction::ZoomBy(1.0 / 1.2)),
        "0" => Some(KeyboardAction::Fit),
        "ArrowLeft" => Some(KeyboardAction::Pan(Point { x: 50.0, y: 0.0 })),
        "ArrowRight" => Some(KeyboardAction::Pan(Point { x: -50.0, y: 0.0 })),
        "ArrowUp" => Some(KeyboardAction::Pan(Point { x: 0.0, y: 50.0 })),
        "ArrowDown" => Some(KeyboardAction::Pan(Point { x: 0.0, y: -50.0 })),
        "Enter" => Some(KeyboardAction::Activate),
        " " => Some(KeyboardAction::Select),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accessible_keyboard_map_preserves_tab_navigation() {
        assert_eq!(keyboard_action("Tab"), None);
        assert_eq!(keyboard_action("Enter"), Some(KeyboardAction::Activate));
        assert_eq!(keyboard_action(" "), Some(KeyboardAction::Select));
        assert_eq!(
            keyboard_action("Escape"),
            Some(KeyboardAction::ClearSelection)
        );
        assert_eq!(keyboard_action("0"), Some(KeyboardAction::Fit));
        assert_eq!(keyboard_action("+"), keyboard_action("="));
        assert_eq!(
            keyboard_action("-"),
            Some(KeyboardAction::ZoomBy(1.0 / 1.2))
        );
        for (key, point) in [
            ("ArrowLeft", Point { x: 50.0, y: 0.0 }),
            ("ArrowRight", Point { x: -50.0, y: 0.0 }),
            ("ArrowUp", Point { x: 0.0, y: 50.0 }),
            ("ArrowDown", Point { x: 0.0, y: -50.0 }),
        ] {
            assert_eq!(keyboard_action(key), Some(KeyboardAction::Pan(point)));
        }
    }
}
