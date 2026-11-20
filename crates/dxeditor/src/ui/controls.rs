//! Shared building blocks of the editor's floating surfaces: icon buttons with
//! hover tooltips, menu rows, collapsible menu groups and colour swatches.
use super::icons::Icon;
use crate::AttributeMap;
use dioxus::prelude::*;

/// Whether the browser runs on an Apple platform, where shortcuts use ⌘.
fn apple() -> bool {
    web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .is_some_and(|platform| {
            platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
        })
}

/// A shortcut written as `Control+Shift+S`, in the form for `aria-keyshortcuts`
/// and the form displayed to the user on this platform.
pub(super) fn shortcut(keys: &str) -> (String, String) {
    if keys.is_empty() {
        return (String::new(), String::new());
    }
    let arrows = |keys: String| {
        keys.replace("ArrowUp", "↑")
            .replace("ArrowDown", "↓")
            .replace("ArrowLeft", "←")
            .replace("ArrowRight", "→")
    };
    if apple() {
        let aria = keys.replace("Control+", "Meta+");
        let display = aria
            .replace("Meta+", "⌘")
            .replace("Shift+", "⇧")
            .replace("Alt+", "⌥");
        (aria, arrows(display))
    } else {
        (keys.to_string(), arrows(keys.replace("Control+", "Ctrl+")))
    }
}

/// An icon-only toolbar button. Hovering or focusing it reveals its name and
/// shortcut in a tooltip; `expanded` marks a button that opens a menu.
#[component]
pub(super) fn IconButton(
    label: String,
    icon: String,
    onclick: EventHandler<()>,
    #[props(default)] shortcut: String,
    #[props(default)] pressed: Option<bool>,
    #[props(default)] expanded: Option<bool>,
    #[props(default)] swatch: Option<String>,
) -> Element {
    let (aria_keys, display_keys) = self::shortcut(&shortcut);
    let tooltip = label.trim_end_matches('…').to_string();
    let menu = expanded.is_some();
    rsx! {
        button {
            r#type: "button",
            class: if menu { "dxeditor-engine__button dxeditor-bubble__button dxeditor-bubble__button--menu" } else { "dxeditor-engine__button dxeditor-bubble__button" },
            aria_label: "{label}",
            aria_pressed: pressed.map(|pressed| pressed.to_string()),
            aria_expanded: expanded.map(|expanded| expanded.to_string()),
            aria_haspopup: menu.then_some("menu"),
            aria_keyshortcuts: (!aria_keys.is_empty()).then_some(aria_keys),
            "data-swatch": swatch,
            onmousedown: move |event| event.prevent_default(),
            onclick: move |_| onclick.call(()),
            Icon { name: icon }
            if menu {
                Icon { name: "chevron" }
            }
            span { class: "dxeditor-tooltip", aria_hidden: "true",
                "{tooltip}"
                if !display_keys.is_empty() {
                    kbd { "{display_keys}" }
                }
            }
        }
    }
}

/// A full-width menu entry with an icon, a label and an optional shortcut.
/// `checked` makes it a checkbox item (a radio item with `radio`) that shows a
/// check mark when selected.
#[component]
pub(super) fn MenuRow(
    label: String,
    icon: String,
    onclick: EventHandler<()>,
    #[props(default)] shortcut: String,
    #[props(default)] checked: Option<bool>,
    #[props(default)] radio: bool,
    #[props(default)] danger: bool,
) -> Element {
    let role = match checked {
        None => "menuitem",
        Some(_) if radio => "menuitemradio",
        Some(_) => "menuitemcheckbox",
    };
    let (aria_keys, display_keys) = self::shortcut(&shortcut);
    rsx! {
        button {
            r#type: "button",
            role,
            aria_label: "{label}",
            class: if danger { "dxeditor-engine__button dxeditor-menu__row dxeditor-menu__row--danger" } else { "dxeditor-engine__button dxeditor-menu__row" },
            aria_checked: checked.map(|checked| checked.to_string()),
            aria_keyshortcuts: (!aria_keys.is_empty()).then_some(aria_keys),
            onmousedown: move |event| event.prevent_default(),
            onclick: move |_| onclick.call(()),
            Icon { name: icon }
            span { "{label}" }
            if !display_keys.is_empty() {
                kbd { aria_hidden: "true", "{display_keys}" }
            }
            if checked.is_some() {
                Icon { name: "check" }
            }
        }
    }
}

/// A collapsible group of menu rows. Groups sharing a `name` open one at a time.
#[component]
pub(super) fn MenuGroup(label: String, icon: String, name: String, children: Element) -> Element {
    rsx! {
        details { "name": "{name}",
            summary { class: "dxeditor-menu__row", onmousedown: move |event| event.prevent_default(),
                Icon { name: icon }
                span { "{label}" }
                Icon { name: "chevron" }
            }
            {children}
        }
    }
}

#[component]
pub(super) fn MenuSeparator() -> Element {
    rsx! { div { class: "dxeditor-menu__separator", role: "separator" } }
}

/// A titled row of colour swatches, led by one that clears the colour.
/// `filled` shows the colours as backgrounds rather than as text colours.
#[component]
pub(super) fn SwatchRow(
    label: String,
    filled: bool,
    current: Option<String>,
    onpick: EventHandler<Option<String>>,
) -> Element {
    let colors = std::iter::once(None).chain(
        crate::profiles::block_document::COLOR_TOKENS
            .into_iter()
            .map(|color| Some(color.to_string())),
    );
    rsx! {
        div { class: "dxeditor-bubble__heading", role: "presentation", "{label}" }
        div { class: "dxeditor-bubble__swatches", role: "group", aria_label: "{label}",
            for color in colors {
                {
                    let name = color.clone().unwrap_or_else(|| "default".into());
                    let pressed = current == color;
                    rsx! {
                        button {
                            r#type: "button",
                            class: "dxeditor-engine__button dxeditor-bubble__swatch",
                            aria_label: "{label} {name}",
                            title: "{name}",
                            aria_pressed: pressed.to_string(),
                            "data-kind": filled.then_some("highlight"),
                            "data-swatch": color.clone(),
                            onmousedown: move |event| event.prevent_default(),
                            onclick: move |_| onpick.call(color.clone()),
                            "A"
                        }
                    }
                }
            }
        }
    }
}

/// A block type a text block can be turned into.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TurnInto {
    pub kind: String,
    pub attrs: AttributeMap,
    pub label: String,
    pub icon: String,
    pub shortcut: &'static str,
}

impl TurnInto {
    /// Whether a block of `kind` with `attrs` is of this type.
    pub fn matches(&self, kind: Option<&str>, attrs: &AttributeMap) -> bool {
        kind == Some(self.kind.as_str())
            && self
                .attrs
                .get("level")
                .is_none_or(|level| attrs.get("level").unwrap_or(&1.into()) == level)
    }
}

/// The text block types offered by the catalog, with headings per level.
pub(super) fn turn_into_options(catalog: &crate::EditorCatalog) -> Vec<TurnInto> {
    let mut options = Vec::new();
    for spec in catalog
        .component_specs()
        .specs()
        .filter(|spec| crate::engine::model::is_text_kind(&spec.id.0))
    {
        let kind = spec.id.0.clone();
        let label = spec
            .insert
            .as_ref()
            .map_or_else(|| spec.label.clone(), |insert| insert.label.clone());
        if kind == "heading" {
            for level in 1..=3u8 {
                let mut attrs = AttributeMap::new();
                attrs.insert("level".into(), level.into());
                options.push(TurnInto {
                    kind: kind.clone(),
                    attrs,
                    label: format!("Heading {level}"),
                    icon: format!("h{level}"),
                    shortcut: ["Control+Alt+1", "Control+Alt+2", "Control+Alt+3"]
                        [usize::from(level - 1)],
                });
            }
            continue;
        }
        let mut attrs = AttributeMap::new();
        if kind == "todo" {
            attrs.insert("checked".into(), false.into());
        }
        let shortcut = match kind.as_str() {
            "paragraph" => "Control+Alt+0",
            "bulleted_list_item" => "Control+Shift+8",
            "numbered_list_item" => "Control+Shift+7",
            "todo" => "Control+Shift+9",
            "quote" => "Control+Shift+B",
            "code_block" => "Control+Alt+C",
            _ => "",
        };
        options.push(TurnInto {
            icon: kind.clone(),
            kind,
            attrs,
            label,
            shortcut,
        });
    }
    options.sort_by_key(|option| crate::engine::block_type_rank(&option.kind));
    options
}
