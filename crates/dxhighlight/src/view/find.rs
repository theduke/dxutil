use super::{CodeFind, CodeView};
use crate::{CodeSelection, FindQuery, FindSummary, LineTarget, MAX_FIND_MATCHES};
use dioxus::prelude::*;

/// Find controls for a [`CodeView`]: query input, case toggle, `n of m`,
/// previous/next and close. Enter steps forward, Shift+Enter backward and
/// Escape closes. The input takes focus on mount and whenever `focus`
/// changes. Styles come from the neighbouring viewer's stylesheet.
#[component]
pub fn FindBar(
    query: FindQuery,
    summary: FindSummary,
    active: usize,
    on_query: EventHandler<FindQuery>,
    on_step: EventHandler<isize>,
    on_close: EventHandler<()>,
    #[props(default)] scope_note: Option<String>,
    #[props(default)] focus: usize,
) -> Element {
    let mut input = use_signal(|| None::<std::rc::Rc<MountedData>>);
    use_effect(use_reactive((&focus,), move |_| {
        if let Some(input) = input() {
            spawn(async move {
                let _ = input.set_focus(true).await;
            });
        }
    }));
    let status = match summary {
        _ if query.text.is_empty() => String::new(),
        FindSummary { count: 0, .. } => "No results".into(),
        FindSummary { count, capped } => {
            let total = if capped {
                format!("{MAX_FIND_MATCHES}+")
            } else {
                count.to_string()
            };
            format!("{} of {total}", active % count + 1)
        }
    };
    let case_query = query.clone();
    rsx! {
        div { class: "dxhighlight-find", role: "search",
            input {
                class: "dxhighlight-find-input",
                r#type: "search",
                aria_label: "Find in text",
                placeholder: "Find",
                value: "{query.text}",
                onmounted: move |event| async move {
                    let _ = event.set_focus(true).await;
                    input.set(Some(event.data()));
                },
                oninput: move |event| on_query.call(FindQuery { text: event.value(), case_sensitive: query.case_sensitive }),
                onkeydown: move |event| match event.key() {
                    Key::Enter => {
                        event.prevent_default();
                        on_step.call(if event.modifiers().shift() { -1 } else { 1 });
                    }
                    Key::Escape => {
                        event.prevent_default();
                        on_close.call(());
                    }
                    _ => {}
                },
            }
            button {
                r#type: "button",
                title: "Match case. Otherwise only ASCII letters match in either case.",
                aria_label: "Match case",
                aria_pressed: if case_query.case_sensitive { "true" } else { "false" },
                onclick: move |_| on_query.call(FindQuery { case_sensitive: !case_query.case_sensitive, ..case_query.clone() }),
                "Aa"
            }
            span { class: "dxhighlight-find-status", role: "status", "{status}" }
            button { r#type: "button", aria_label: "Previous match", disabled: summary.count == 0, onclick: move |_| on_step.call(-1), "↑" }
            button { r#type: "button", aria_label: "Next match", disabled: summary.count == 0, onclick: move |_| on_step.call(1), "↓" }
            button { r#type: "button", aria_label: "Close find", onclick: move |_| on_close.call(()), "×" }
            if let Some(note) = scope_note {
                span { class: "dxhighlight-find-note", "{note}" }
            }
        }
    }
}

/// [`CodeView`] with its own find state. Ctrl/Cmd+F inside the viewer, or the
/// Find button, opens the bar and focuses its input. `scope_note` tells users
/// what the search covers, for example when only a prefix is loaded.
#[component]
pub fn SearchableCodeView(
    value: String,
    path: Option<String>,
    #[props(default)] goto: Option<LineTarget>,
    #[props(default)] on_selection: Option<EventHandler<CodeSelection>>,
    #[props(default = true)] line_numbers: bool,
    #[props(default)] scope_note: Option<String>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut query = use_signal(FindQuery::default);
    let mut active = use_signal(|| 0usize);
    let mut summary = use_signal(FindSummary::default);
    let mut focus = use_signal(|| 0usize);
    let find = open().then(|| CodeFind {
        query: query(),
        active: active(),
    });
    rsx! {
        div {
            class: "dxhighlight-searchable",
            onkeydown: move |event| {
                if (event.modifiers().ctrl() || event.modifiers().meta())
                    && event.key() == Key::Character("f".into())
                {
                    event.prevent_default();
                    open.set(true);
                    focus += 1;
                }
            },
            if open() {
                FindBar {
                    query: query(),
                    summary: summary(),
                    active: active(),
                    scope_note,
                    focus: focus(),
                    on_query: move |next| {
                        query.set(next);
                        active.set(0);
                    },
                    on_step: move |step: isize| {
                        let count = summary.peek().count;
                        if count > 0 {
                            let current = *active.peek() % count;
                            active.set((current + count).wrapping_add_signed(step) % count);
                        }
                    },
                    on_close: move |_| open.set(false),
                }
            } else {
                button {
                    class: "dxhighlight-find-open",
                    r#type: "button",
                    title: "Find (Ctrl+F)",
                    onclick: move |_| open.set(true),
                    "Find"
                }
            }
            CodeView {
                value,
                path,
                goto,
                on_selection,
                line_numbers,
                find,
                on_find: move |next| summary.set(next),
            }
        }
    }
}
