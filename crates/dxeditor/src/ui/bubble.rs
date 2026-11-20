use super::controls::{IconButton, MenuRow, SwatchRow, turn_into_options};
use super::*;

/// The bubble's dropdown menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BubbleMenu {
    TurnInto,
    Colour,
}

const BUBBLE: Size = Size {
    width: 420.,
    height: 40.,
};
/// The tallest dropdown: the full list of block types.
const DROPDOWN_HEIGHT: f64 = 380.;

/// The selection toolbar: block type, marks, colours and links.
#[component]
pub(super) fn Bubble(ui: EditorUiState, handle: EngineHandle, catalog: EditorCatalog) -> Element {
    let mut open = use_signal(|| None::<BubbleMenu>);
    let (placement, placed) = place_surface(&ui, ui.geometry.selection, BUBBLE, Side::Top);
    // Open dropdowns towards whichever side of the toolbar has more room.
    let viewport = ui.geometry.viewport;
    let scale = if ui.geometry.scale_y > 0.0 {
        ui.geometry.scale_y
    } else {
        1.
    };
    let above = (placed.y - viewport.y) * scale;
    let below = (viewport.y + viewport.height - placed.y) * scale - BUBBLE.height;
    let upward = below < DROPDOWN_HEIGHT && above > below;
    let menu_height = (if upward { above } else { below } - 16.).max(120.);
    let specs = catalog.component_specs();
    let profile = ui.profile;
    let has = |kind: &str| {
        specs.spec_by_id(kind).is_some()
            && (profile.allows_mark(kind)
                || !matches!(kind, "text_color" | "highlight" | "underline"))
    };
    let marks = [
        ("bold", "Bold", "Control+B"),
        ("italic", "Italic", "Control+I"),
        ("underline", "Underline", "Control+U"),
        ("strike", "Strikethrough", "Control+Shift+S"),
        ("code", "Inline code", "Control+E"),
    ];
    let options = turn_into_options(&catalog);
    let current = options
        .iter()
        .find(|option| option.matches(ui.selection.kind.as_deref(), &ui.selection.attrs))
        .cloned();
    let mark_color = |kind: &str| {
        ui.selection
            .active_marks
            .iter()
            .find(|mark| mark.kind.0 == kind)
            .and_then(|mark| mark.attrs.get("color"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let text_color = mark_color("text_color");
    let highlight = mark_color("highlight");
    let mut toggle = move |menu: BubbleMenu| {
        open.set(if open() == Some(menu) {
            None
        } else {
            Some(menu)
        });
    };
    let keys = handle.clone();
    let escape_focus = handle.clone();
    let menu_attrs = (
        if upward { "top" } else { "bottom" },
        format!("max-height:{menu_height}px"),
    );
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__bubble",
            role: "toolbar",
            aria_label: "Text formatting",
            style: "{placement}",
            onmounted: init_roving,
            onfocusin: roving_focus,
            onkeydown: move |event| {
                if open().is_some() && event.key() == Key::Escape {
                    event.prevent_default();
                    event.stop_propagation();
                    open.set(None);
                    escape_focus.run(Command::Focus);
                } else {
                    menu_keys(event, keys.clone());
                }
            },
            if !options.is_empty() && ui.profile.has_blocks() {
                span { class: "dxeditor-bubble__group",
                    IconButton {
                        label: "Turn into",
                        icon: current.as_ref().map_or_else(|| "paragraph".to_string(), |current| current.icon.clone()),
                        expanded: Some(open() == Some(BubbleMenu::TurnInto)),
                        onclick: move |_| toggle(BubbleMenu::TurnInto),
                    }
                    if open() == Some(BubbleMenu::TurnInto) {
                        div {
                            class: "dxeditor-bubble__menu",
                            role: "menu",
                            aria_label: "Turn into",
                            onmounted: focus_first,
                            "data-side": menu_attrs.0,
                            style: "{menu_attrs.1}",
                            for option in options.iter().cloned() {
                                MenuRow {
                                    label: option.label.clone(),
                                    icon: option.icon.clone(),
                                    shortcut: option.shortcut,
                                    checked: Some(current.as_ref() == Some(&option)),
                                    radio: true,
                                    onclick: {
                                        let handle = handle.clone();
                                        move |_| {
                                            open.set(None);
                                            handle.run(Command::TurnInto {
                                                kind: option.kind.clone(),
                                                attrs: option.attrs.clone(),
                                            });
                                            handle.run(Command::Focus);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                span { role: "separator", class: "dxeditor-bubble__separator" }
            }
            for (kind , label , shortcut) in marks.into_iter().filter(|(kind, _, _)| has(kind)) {
                IconButton {
                    label,
                    icon: kind,
                    shortcut,
                    pressed: Some(ui.selection.active_marks.iter().any(|mark| mark.kind.0 == kind)),
                    onclick: {
                        let handle = handle.clone();
                        move |_| handle.run(Command::ToggleMark(ComponentMark::new(kind)))
                    },
                }
            }
            if has("text_color") || has("highlight") {
                span { class: "dxeditor-bubble__group",
                    IconButton {
                        label: "Colour",
                        icon: "colour",
                        swatch: text_color.clone(),
                        expanded: Some(open() == Some(BubbleMenu::Colour)),
                        onclick: move |_| toggle(BubbleMenu::Colour),
                    }
                    if open() == Some(BubbleMenu::Colour) {
                        div {
                            class: "dxeditor-bubble__menu",
                            role: "menu",
                            aria_label: "Colour",
                            onmounted: focus_first,
                            "data-side": menu_attrs.0,
                            "data-align": "end",
                            style: "{menu_attrs.1}",
                            for (kind , label , filled , current) in [("text_color", "Text", false, text_color.clone()), ("highlight", "Highlight", true, highlight.clone())] {
                                if has(kind) {
                                    SwatchRow {
                                        label,
                                        filled,
                                        current,
                                        onpick: {
                                            let handle = handle.clone();
                                            move |color: Option<String>| {
                                                open.set(None);
                                                handle.run(color_mark_command(kind, color));
                                                handle.run(Command::Focus);
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if has("link") || has("entity_link") || has("entity_embed") {
                span { role: "separator", class: "dxeditor-bubble__separator" }
            }
            if has("link") {
                IconButton {
                    label: "Link",
                    icon: "link",
                    shortcut: "Control+K",
                    pressed: Some(ui.selection.link.is_some()),
                    onclick: {
                        let handle = handle.clone();
                        move |_| handle.run(Command::OpenMenu("link".into()))
                    },
                }
            }
            if has("entity_link") {
                IconButton {
                    label: "Link to entity…",
                    icon: "entity_embed",
                    onclick: {
                        let handle = handle.clone();
                        move |_| handle.picker("link")
                    },
                }
            }
            if has("entity_embed") {
                IconButton {
                    label: "Create from selection…",
                    icon: "create_entity",
                    onclick: {
                        let handle = handle.clone();
                        let ui = ui.clone();
                        move |_| handle.create(&ui, "selection", String::new())
                    },
                }
            }
        }
    }
}

/// Sets a colour mark on the selection, or clears it for the default colour,
/// remembering the last colour used.
fn color_mark_command(kind: &str, color: Option<String>) -> Command {
    let Some(color) = color else {
        return Command::UnsetMark(kind.into());
    };
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let key = if kind == "highlight" {
            "dxeditor-last-highlight"
        } else {
            "dxeditor-last-textColor"
        };
        let _ = storage.set_item(key, &color);
    }
    let mut mark = ComponentMark::new(kind);
    mark.attrs.insert("color".into(), serde_json::json!(color));
    Command::SetMark(mark)
}

/// Edits the link of the selected text. It takes the toolbar's place until
/// the link is applied or dismissed.
#[component]
pub(super) fn LinkPopover(ui: EditorUiState, handle: EngineHandle) -> Element {
    let existing = ui.selection.link.clone();
    let mut value = use_signal(|| existing.clone().unwrap_or_default());
    let mut title = use_signal(|| {
        ui.selection
            .active_marks
            .iter()
            .find(|mark| mark.kind.0 == "link")
            .and_then(|mark| mark.attrs.get("title"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let mut error = use_signal(String::new);
    let notes = ui.profile == crate::engine::Profile::MarkdownNotes;
    let placement = style(
        &ui,
        ui.geometry.selection,
        Size {
            width: 340.,
            height: if notes { 80. } else { 40. },
        },
        Side::Bottom,
    );
    let keys = handle.clone();
    let enter = handle.clone();
    let apply = handle.clone();
    let remove = handle.clone();
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__link-popover",
            role: "dialog",
            aria_label: "Edit link",
            style: "{placement}",
            onkeydown: move |event| {
                if event.key() == Key::Enter {
                    event.prevent_default();
                    apply_link(&enter, &value(), &title(), &mut error);
                } else {
                    menu_keys(event, keys.clone());
                }
            },
            input {
                r#type: "url",
                autofocus: true,
                onmounted: focus_input,
                placeholder: "Paste or type a link",
                aria_label: "Link URL",
                aria_invalid: (!error().is_empty()).to_string(),
                value: "{value}",
                oninput: move |event| {
                    error.set(String::new());
                    value.set(event.value());
                },
            }
            IconButton {
                label: "Apply link",
                icon: "check",
                onclick: move |_| apply_link(&apply, &value(), &title(), &mut error),
            }
            if crate::is_safe_url(&crate::normalize_link_url(&value())) && !value().trim().is_empty() {
                IconButton {
                    label: "Open link",
                    icon: "open",
                    onclick: move |_| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.open_with_url_and_target_and_features(&crate::normalize_link_url(&value()), "_blank", "noopener");
                        }
                    },
                }
            }
            if existing.is_some() {
                IconButton {
                    label: "Remove link",
                    icon: "unlink",
                    onclick: move |_| {
                        remove.run(Command::UnsetMark("link".into()));
                        remove.close();
                    },
                }
            }
            if notes {
                input {
                    r#type: "text",
                    class: "dxeditor-link-title",
                    aria_label: "Link title",
                    placeholder: "Title (optional)",
                    value: "{title}",
                    oninput: move |event| title.set(event.value()),
                }
            }
            span { role: "alert", "{error}" }
        }
    }
}
/// Shows the link under a collapsed caret, with actions to open, edit or
/// remove it.
#[component]
pub(super) fn LinkPreview(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(href) = ui.selection.link.clone() else {
        return rsx! {};
    };
    let placement = style(
        &ui,
        ui.geometry.caret,
        Size {
            width: 340.,
            height: 40.,
        },
        Side::Top,
    );
    let safe = crate::is_safe_url(&href);
    let keys = handle.clone();
    let open = href.clone();
    let edit = handle.clone();
    let remove = handle.clone();
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__bubble dxeditor-engine__link-preview",
            role: "toolbar",
            aria_label: "Link",
            style: "{placement}",
            onmounted: init_roving,
            onfocusin: roving_focus,
            onkeydown: move |event| menu_keys(event, keys.clone()),
            span { class: "dxeditor-link-preview__url", title: "{href}", "{href}" }
            span { role: "separator", class: "dxeditor-bubble__separator" }
            if safe {
                IconButton {
                    label: "Open link",
                    icon: "open",
                    onclick: move |_| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.open_with_url_and_target_and_features(&open, "_blank", "noopener");
                        }
                    },
                }
            }
            IconButton {
                label: "Edit link",
                icon: "link",
                shortcut: "Control+K",
                onclick: move |_| edit.run(Command::OpenMenu("link".into())),
            }
            IconButton {
                label: "Remove link",
                icon: "unlink",
                onclick: move |_| remove.run(Command::UnsetMark("link".into())),
            }
        }
    }
}
fn apply_link(handle: &EngineHandle, value: &str, title: &str, error: &mut Signal<String>) {
    let value = crate::normalize_link_url(value);
    if !crate::is_safe_url(&value) || value.is_empty() {
        error.set("Enter a valid URL".into());
        return;
    }
    let mut mark = ComponentMark::new("link");
    mark.attrs.insert("href".into(), serde_json::json!(value));
    if !title.is_empty() {
        mark.attrs.insert("title".into(), serde_json::json!(title));
    }
    handle.run(Command::SetMark(mark));
    handle.close();
}
#[component]
pub(super) fn BookmarkPopover(ui: EditorUiState, handle: EngineHandle) -> Element {
    let mut value = use_signal(String::new);
    let mut error = use_signal(String::new);
    let keys = handle.clone();
    let submit = handle.clone();
    let placement = style(
        &ui,
        ui.geometry.caret,
        Size {
            width: 340.,
            height: 40.,
        },
        Side::Bottom,
    );
    let apply = move |handle: &EngineHandle, value: String, error: &mut Signal<String>| {
        let value = crate::normalize_link_url(&value);
        if !crate::is_safe_url(&value) || !value.contains("://") {
            error.set("Enter a valid URL".into());
            return;
        }
        let mut node = crate::ComponentNode::new("bookmark");
        node.attrs.insert("url".into(), serde_json::json!(value));
        handle.run(Command::InsertBlocks {
            at: crate::InsertAt::Selection,
            nodes: vec![crate::engine::model::block_node(node)],
        });
    };
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__link-popover",
            role: "dialog",
            aria_label: "Insert web bookmark",
            style: "{placement}",
            onkeydown: move |event| {
                if event.key() == Key::Enter {
                    event.prevent_default();
                    apply(&submit, value(), &mut error);
                } else {
                    menu_keys(event, keys.clone());
                }
            },
            input {
                r#type: "url",
                aria_label: "Bookmark URL",
                aria_invalid: (!error().is_empty()).to_string(),
                placeholder: "Paste a web address",
                autofocus: true,
                onmounted: focus_input,
                value: "{value}",
                oninput: move |event| {
                    error.set(String::new());
                    value.set(event.value());
                },
            }
            IconButton {
                label: "Insert bookmark",
                icon: "check",
                onclick: move |_| apply(&handle, value(), &mut error),
            }
            span { role: "alert", "{error}" }
        }
    }
}
