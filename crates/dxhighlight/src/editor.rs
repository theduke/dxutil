use crate::{CodeSelection, LineTarget, view::CSS};
use dioxus::prelude::*;

/// A plain textarea for source text, styled like [`crate::CodeView`]. Text is
/// kept verbatim apart from the browser's LF normalization. With `dom`, `goto`
/// moves the caret and scrolls, and `on_selection` reports the selected lines;
/// without it both are inert. Ctrl/Cmd+S calls `on_save`.
#[component]
pub fn TextEditor(
    value: String,
    on_input: EventHandler<String>,
    #[props(default)] on_save: Option<EventHandler<()>>,
    #[props(default)] goto: Option<LineTarget>,
    #[props(default)] on_selection: Option<EventHandler<CodeSelection>>,
    #[props(default)] read_only: bool,
    #[props(default)] wrap: bool,
    #[props(default)] label: Option<String>,
) -> Element {
    let id = use_hook(crate::view::next_id);
    let goto_id = id.clone();
    use_effect(use_reactive((&goto,), move |(goto,)| {
        if let Some(target) = goto {
            dom::goto(&goto_id, &target);
        }
    }));
    let selection_id = id.clone();
    rsx! {
        style { "{CSS}" }
        textarea {
            id,
            class: "dxhighlight-editor",
            aria_label: label.unwrap_or_else(|| "Text".into()),
            spellcheck: false,
            wrap: if wrap { "soft" } else { "off" },
            readonly: read_only,
            value,
            oninput: move |event| on_input.call(event.value()),
            onselect: move |_| {
                if let Some(handler) = on_selection {
                    dom::selection(&selection_id, handler);
                }
            },
            onkeydown: move |event| {
                if let Some(on_save) = on_save
                    && (event.modifiers().ctrl() || event.modifiers().meta())
                    && event.key() == Key::Character("s".into())
                {
                    event.prevent_default();
                    on_save.call(());
                }
            },
        }
    }
}

#[cfg(feature = "dom")]
mod dom {
    use crate::{CodeSelection, LineTarget};
    use dioxus::prelude::*;

    pub(super) fn goto(id: &str, target: &LineTarget) {
        let eval = document::eval(include_str!("editor/goto.js"));
        let _ = eval.send(serde_json::json!({ "id": id, "target": target }));
    }

    pub(super) fn selection(id: &str, handler: EventHandler<CodeSelection>) {
        let id = id.to_string();
        spawn(async move {
            let mut eval = document::eval(include_str!("editor/selection.js"));
            let _ = eval.send(id);
            if let Ok(Some(selection)) = eval.recv::<Option<CodeSelection>>().await {
                handler.call(selection);
            }
        });
    }
}

#[cfg(not(feature = "dom"))]
mod dom {
    use crate::{CodeSelection, LineTarget};
    use dioxus::prelude::EventHandler;

    pub(super) fn goto(_: &str, _: &LineTarget) {}
    pub(super) fn selection(_: &str, _: EventHandler<CodeSelection>) {}
}
