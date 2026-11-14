use dioxus::prelude::*;
use dxcomp::{ActionButton, Button, ButtonVariant, CopyButton, CopyRequest, form};

/// Deliberately controlled completions make asynchronous states inspectable without timers.
#[component]
pub fn ActionsFields() -> Element {
    let mut name = use_signal(|| "Release notes".to_owned());
    let mut notes = use_signal(String::new);
    let mut read_only = use_signal(|| false);
    let mut disabled = use_signal(|| false);
    let mut invalid = use_signal(|| false);
    let mut pending = use_signal(|| false);
    let mut saved = use_signal(|| false);
    let mut confirm_open = use_signal(|| false);
    let mut submissions = use_signal(|| 0);
    let mut copies = use_signal(Vec::<CopyRequest>::new);
    let requests = copies.read().len();
    let request_text = copies
        .read()
        .last()
        .map(|request| request.value().to_owned())
        .unwrap_or_default();
    rsx! {
        div { class: "demo-columns",
            form { class: "settings-card", novalidate: true,
                onsubmit: move |event| { event.prevent_default(); submissions += 1; },
                h3 { "Application-owned fields" }
                p { class: "muted", "Labels, hints and errors stay connected without adopting a form state library." }
                form::Field {
                    id: "manual-name", label: "Note title", description: "A short, descriptive title.",
                    errors: if invalid() { vec!["Choose a different title.".to_owned()] } else { Vec::new() },
                    required: true, disabled: disabled(), read_only: read_only(),
                    form::TextInput { value: name(), oninput: move |event: FormEvent| name.set(event.value()) }
                }
                form::Field { id: "manual-notes", label: "Notes", description: "Keep your own draft and validation rules.",
                    disabled: disabled(), read_only: read_only(),
                    form::Textarea { value: notes(), rows: "3", oninput: move |event: FormEvent| notes.set(event.value()) }
                }
                div { class: "toolbar",
                    ActionButton { pending: pending(), pending_label: "Saving…", disabled: disabled(),
                        onclick: move |_| { pending.set(true); saved.set(false); }, "Save note"
                    }
                    CopyButton { value: name(), label: "Copy title", show_label: true, disabled: disabled(),
                        on_copy: move |request| copies.write().push(request)
                    }
                }
                p { class: "muted", "Copy this selectable text manually if clipboard access is unavailable:" }
                code { "{name}" }
            }
            section { class: "state-card",
                h3 { "Try each state" }
                p { class: "muted", "Operations wait here until you complete them. Change the title during a copy to verify that an older result cannot mark new text as copied." }
                div { class: "toolbar",
                    label { input { r#type: "checkbox", checked: read_only(), oninput: move |event| read_only.set(event.checked()) } "Read-only fields" }
                    label { input { r#type: "checkbox", checked: disabled(), oninput: move |event| disabled.set(event.checked()) } "Disable fields and actions" }
                    label { input { r#type: "checkbox", checked: invalid(), oninput: move |event| invalid.set(event.checked()) } "Show title error" }
                }
                div { class: "toolbar",
                    Button { r#type: "button", variant: ButtonVariant::Outline, disabled: !pending(),
                        onclick: move |_| { pending.set(false); saved.set(true); }, "Finish saving"
                    }
                    Button { r#type: "button", variant: ButtonVariant::Outline, disabled: requests == 0,
                        onclick: move |_| { let request = copies.write().remove(0); request.complete(Ok(())); }, "Complete oldest copy"
                    }
                    Button { r#type: "button", variant: ButtonVariant::Outline, disabled: requests == 0,
                        onclick: move |_| { let request = copies.write().remove(0); request.complete(Err("Clipboard permission denied".to_owned())); }, "Fail oldest copy"
                    }
                }
                Button { r#type: "button", variant: ButtonVariant::Outline,
                    onclick: move |_| confirm_open.set(true), "Review deletion"
                }
                dxcomp::AlertDialog { open: confirm_open(), on_open_change: move |open| confirm_open.set(open),
                    class: "manual-alert", "data-testid": "manual-alert",
                    dxcomp::AlertDialogTitle { "Delete this note?" }
                    dxcomp::AlertDialogDescription { "This example only demonstrates a styled confirmation. No note will be deleted." }
                    dxcomp::AlertDialogActions {
                        dxcomp::AlertDialogCancel { "Keep note" }
                        dxcomp::AlertDialogAction { on_click: move |_| confirm_open.set(false), "Delete note" }
                    }
                }
                dl {
                    dt { "Outstanding copies" } dd { id: "manual-copy-count", "{requests}" }
                    dt { "Latest requested value" } dd { id: "manual-copy-value", "{request_text}" }
                    dt { "Unexpected form submissions" } dd { id: "manual-submissions", "{submissions}" }
                }
                p { role: "status", if saved() { "Note saved." } }
            }
        }
    }
}
