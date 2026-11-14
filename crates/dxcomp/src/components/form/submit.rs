use dioxus::prelude::*;

use crate::{ButtonSize, ButtonVariant};

/// Submit action with a busy state and the shared Spinner. Keep application status
/// announcements in a separate live region; the decorative spinner is hidden from AT.
#[component]
pub fn SubmitButton(
    #[props(default)] pending: bool,
    #[props(default)] pending_label: Option<String>,
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    onclick: Option<EventHandler<MouseEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = button)]
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        crate::components::action_button::PendingButton {
            default_type: "submit", pending, pending_label, variant, size, onclick, attributes,
            {children}
        }
    }
}
