use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};
use dxeditor::profiles::composer::{
    ComposerEditor, ComposerEvent, ComposerSegment, ComposerValue, SubmitKey,
};

const NOTES: &str = "# A little room to think\n\nBuild something useful, one component at a time.\n\nExplore **rich text** and Markdown with [Dioxus](https://dioxuslabs.com).\n\n## Today\n\n- Explore the components\n- Try the keyboard shortcuts\n- Make it your own\n\n> Good tools leave room for your ideas.\n";
const SECOND_NOTES: &str = "# Release checklist\n\n1. Review the changes\n2. Run the tests\n3. Share what you built\n\nA small `inline code` example.\n";

#[component]
pub fn Markdown() -> Element {
    let mut value = use_signal(|| NOTES.to_string());
    let mut changes = use_signal(|| 0u64);
    rsx! {
        div { class: "controls-strip",
            Button { variant: ButtonVariant::Outline, onclick: move |_| value.set(SECOND_NOTES.into()), "Load checklist" }
            span { class: "muted", "{changes} changes · {value().chars().count()} characters" }
        }
        div { class: "editor-surface", "data-testid": "markdown-editor",
            dxeditor::MarkdownEditor { value: value(), on_change: move |text| { value.set(text); changes += 1; } }
        }
        details { class: "output-details", summary { "Live Markdown output" } pre { "{value}" } }
    }
}

#[component]
pub fn Composer() -> Element {
    let mut value = use_signal(ComposerValue::default);
    let mut sent = use_signal(Vec::<String>::new);
    let mut last_event = use_signal(|| "No events yet".to_string());
    let segments = serde_json::to_string_pretty(&value()).unwrap_or_default();
    rsx! {
        div { class: "composer-card",
            span { class: "eyebrow", "NEW MESSAGE" }
            ComposerEditor {
                value: value(), on_change: move |next| value.set(next),
                triggers: vec![], suggestions: Default::default(),
                placeholder: "What are you building?", aria_label: "Compose a message",
                submit_on: SubmitKey::ModEnter,
                on_event: move |event| {
                    if matches!(event, ComposerEvent::Submit { .. }) {
                        let text: String = value.peek().segments.iter().map(|segment| match segment {
                            ComposerSegment::Text(text) => text.clone(),
                            ComposerSegment::Chip(chip) => format!("[{}]", chip.label),
                        }).collect();
                        if !text.trim().is_empty() { sent.write().push(text); value.set(ComposerValue::default()); }
                    }
                    last_event.set(format!("{event:?}"));
                },
            }
            p { class: "muted", "Ctrl/Cmd+Enter to send · Shift+Enter for a new line" }
        }
        p { class: "state-line", role: "status", "Last event: {last_event}" }
        div { class: "message-list", "aria-label": "Sent messages",
            if sent().is_empty() { p { class: "muted", "Your sent messages will appear here." } }
            for (index, message) in sent().iter().enumerate() {
                article { class: "message", span { class: "eyebrow", "MESSAGE {index + 1}" } p { "{message}" } }
            }
        }
        details { class: "output-details", summary { "Structured composer value" } pre { "{segments}" } }
    }
}
