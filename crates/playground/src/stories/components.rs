use dioxus::prelude::*;
use dxcomp::{
    badge::Badge,
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{Dialog, DialogClose, DialogDescription, DialogTitle},
    input::Input,
    progress::Progress,
    slider::Slider,
    switch::Switch,
    virtual_list::{VirtualList, use_virtual_list_scroller},
};

#[component]
pub fn Buttons() -> Element {
    let mut clicks = use_signal(|| 0u64);
    let mut disabled = use_signal(|| false);
    let mut size = use_signal(|| "default".to_string());
    let button_size = match size().as_str() {
        "xs" => ButtonSize::Xs,
        "sm" => ButtonSize::Sm,
        "lg" => ButtonSize::Lg,
        _ => ButtonSize::Default,
    };
    rsx! {
        div { class: "controls-strip",
            label { "Size"
                select { aria_label: "Size", value: size, onchange: move |e| size.set(e.value()),
                    option { value: "xs", "Extra small" } option { value: "sm", "Small" }
                    option { value: "default", "Default" } option { value: "lg", "Large" }
                }
            }
            label { class: "check-label", input { r#type: "checkbox", checked: disabled, onchange: move |e| disabled.set(e.checked()) } "Disabled" }
        }
        div { class: "button-grid",
            for (variant, label) in [
                (ButtonVariant::Primary, "Primary"), (ButtonVariant::Secondary, "Secondary"),
                (ButtonVariant::Destructive, "Destructive"), (ButtonVariant::Outline, "Outline"),
                (ButtonVariant::Ghost, "Ghost"), (ButtonVariant::Link, "Link"),
            ] {
                div { class: "button-example",
                    Button { variant, size: button_size, disabled: disabled(), r#type: "button", onclick: move |_| clicks += 1, "{label}" }
                    code { "{variant.class()}" }
                }
            }
        }
        p { class: "state-line", role: "status", "Total clicks: {clicks}" }
    }
}

#[component]
pub fn Inputs() -> Element {
    let mut name = use_signal(|| "My next project".to_string());
    let mut notifications = use_signal(|| true);
    let mut completion = use_signal(|| 65.0f64);
    rsx! {
        div { class: "demo-columns",
            div { class: "settings-card",
                Badge { "Project settings" }
                label { r#for: "project-name", "Project name" }
                Input { id: "project-name", value: name, oninput: move |e: FormEvent| name.set(e.value()) }
                div { class: "setting-row",
                    label { r#for: "notifications", "Notifications" }
                    Switch { id: "notifications", checked: notifications(), on_checked_change: move |value| notifications.set(value) }
                }
                label { "Completion · {completion:.0}%" }
                Slider { label: "Completion", value: completion(), min: 0.0, max: 100.0, step: 1.0, on_value_change: move |value| completion.set(value) }
                Progress { value: completion(), max: 100.0, "aria-label": "Project completion" }
            }
            div { class: "state-card", h3 { "Controlled state" }
                dl { dt { "Name" } dd { "{name}" } dt { "Notifications" } dd { "{notifications}" } dt { "Completion" } dd { "{completion:.0}%" } }
            }
        }
    }
}

#[component]
pub fn DialogStory() -> Element {
    let mut open = use_signal(|| false);
    let mut result = use_signal(|| "No action taken".to_string());
    rsx! {
        div { class: "center-demo",
            div { class: "dialog-illustration", "↗" }
            h3 { "Ready to share?" }
            p { "Preview a focused confirmation flow." }
            Button { onclick: move |_| open.set(true), "Open dialog" }
            p { class: "state-line", role: "status", "{result}" }
        }
        Dialog { open: open(), on_open_change: move |value| open.set(value),
            DialogTitle { "Share this project?" }
            DialogDescription { "This is a local demo. Confirming updates the story state without publishing anything." }
            div { class: "dialog-actions",
                DialogClose { "Cancel" }
                Button { onclick: move |_| { result.set("Confirmed — project shared locally".into()); open.set(false); }, "Confirm share" }
            }
        }
    }
}

#[component]
pub fn VirtualListStory() -> Element {
    let scroller = use_virtual_list_scroller();
    let mut selected = use_signal(|| None::<usize>);
    rsx! {
        div { class: "controls-strip",
            Button { variant: ButtonVariant::Outline, onclick: move |_| scroller.scroll_to(0), "Jump to first" }
            Button { variant: ButtonVariant::Outline, onclick: move |_| scroller.scroll_to(8999), "Jump to row 9,000" }
            span { class: "muted", "10,000 rows" }
        }
        VirtualList {
            count: 10_000usize,
            estimate_size: move |_| 52u32,
            style: "height: 360px; overflow-y: auto;",
            render_item: move |index: usize| rsx! {
                button { class: "virtual-row", "aria-pressed": selected() == Some(index), onclick: move |_| selected.set(Some(index)),
                    span { class: "row-number", "{index + 1:05}" }
                    span { "Project activity {index + 1}" }
                    span { class: "muted", if index.is_multiple_of(3) { "Updated" } else { "Reviewed" } }
                }
            },
        }
        p { class: "state-line", role: "status",
            if let Some(index) = selected() { "Selected row {index + 1} (index {index})" } else { "Select a row to inspect it." }
        }
    }
}
