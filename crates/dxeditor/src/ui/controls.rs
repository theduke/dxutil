//! Shared building blocks of the editor's floating surfaces: icon buttons with
//! hover tooltips, menu rows, collapsible menu groups and colour swatches.
use super::icons::Icon;
use crate::AttributeMap;
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

/// The menu control under the pointer of `event`.
fn pointer_target(event: &MouseEvent) -> Option<web_sys::HtmlElement> {
    use dioxus::web::WebEventExt;
    event
        .data()
        .try_as_web_event()?
        .target()?
        .dyn_into::<web_sys::Element>()
        .ok()?
        .closest("button,summary")
        .ok()??
        .dyn_into::<web_sys::HtmlElement>()
        .ok()
}

/// Moves focus to the menu control under the pointer, so the pointer and the
/// keyboard share one highlighted row. Menus manage focus themselves, so this
/// never takes focus from the document. Entering a row of the top-level menu
/// closes its open submenus; entering a group's title opens it.
fn follow_pointer(event: MouseEvent) {
    let Some(item) = pointer_target(&event) else {
        return;
    };
    if let Ok(Some(menu)) = item.closest("[role=menu]")
        && item
            .closest(".dxeditor-menu__submenu")
            .ok()
            .flatten()
            .is_none()
    {
        close_flyouts(&menu, &item);
        let own = item
            .parent_element()
            .filter(|parent| parent.tag_name() == "DETAILS");
        if let Some(group) = own
            && flyout(&group)
            && !group.has_attribute("open")
        {
            let _ = group.set_attribute("open", "");
            fit_submenu(&group);
        }
    }
    let focused = item
        .owner_document()
        .and_then(|document| document.active_element())
        .is_some_and(|active| item.is_same_node(Some(&active)));
    if !focused {
        let _ = item.focus();
    }
}

/// Whether a group's rows open as a flyout beside its menu rather than
/// expanding in place (on narrow or touch screens).
fn flyout(group: &web_sys::Element) -> bool {
    group
        .query_selector(":scope > .dxeditor-menu__submenu")
        .ok()
        .flatten()
        .and_then(|submenu| web_sys::window()?.get_computed_style(&submenu).ok()?)
        .and_then(|style| style.get_property_value("position").ok())
        .is_some_and(|position| position == "absolute")
}

/// Opens a menu whose groups may fly out: when its rows do not fit the room
/// it was given, flyouts could not follow it as it scrolls, so its groups
/// expand in place and the menu scrolls instead. Focuses its first row.
pub(super) fn mount_menu(event: MountedEvent) {
    use dioxus::web::WebEventExt;
    if let Some(menu) = event.data().try_as_web_event()
        && menu.scroll_height() > menu.client_height() + 1
    {
        let _ = menu.set_attribute("data-inline-groups", "");
    }
    super::focus_first(event);
}

/// Whether `element` is a row of a group shown as a flyout.
pub(super) fn in_flyout(element: &web_sys::Element) -> bool {
    element
        .closest("details")
        .ok()
        .flatten()
        .is_some_and(|group| flyout(&group))
        && element.tag_name() != "SUMMARY"
}

/// Closes the open flyouts of `menu` other than the one titled by `focused`.
pub(super) fn close_flyouts(menu: &web_sys::Element, focused: &web_sys::Element) {
    let own = focused.parent_element();
    if let Ok(open) = menu.query_selector_all("details[open]") {
        for index in 0..open.length() {
            if let Some(group) = open
                .item(index)
                .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
                && own.as_ref() != Some(&group)
                && flyout(&group)
            {
                let _ = group.remove_attribute("open");
            }
        }
    }
}

/// Keeps an open flyout inside the viewport: it flips to the menu's other
/// side when there is no room beside it, and moves up when it would run past
/// the bottom edge.
pub(super) fn fit_submenu(group: &web_sys::Element) {
    let Some(submenu) = group
        .query_selector(":scope > .dxeditor-menu__submenu")
        .ok()
        .flatten()
        .and_then(|submenu| submenu.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let style = submenu.style();
    let _ = style.remove_property("top");
    let _ = submenu.remove_attribute("data-side");
    if !group.has_attribute("open") || !flyout(group) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let width = window
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .unwrap_or(0.0);
    let height = window
        .inner_height()
        .ok()
        .and_then(|height| height.as_f64())
        .unwrap_or(0.0);
    let rect = submenu.get_bounding_client_rect();
    if rect.right() > width - 8. {
        let _ = submenu.set_attribute("data-side", "start");
    }
    let overflow = rect.bottom() - (height - 8.);
    if overflow > 0. {
        let shift = overflow.min((rect.top() - 8.).max(0.));
        let _ = style.set_property("top", &format!("{}px", -4. - shift));
    }
}

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
    #[props(default)] text: Option<String>,
) -> Element {
    let (aria_keys, display_keys) = self::shortcut(&shortcut);
    let tooltip = label.trim_end_matches('…').to_string();
    let menu = expanded.is_some();
    let class = match (menu, text.is_some()) {
        (true, true) => {
            "dxeditor-engine__button dxeditor-bubble__button dxeditor-bubble__button--menu dxeditor-bubble__button--text"
        }
        (true, false) => {
            "dxeditor-engine__button dxeditor-bubble__button dxeditor-bubble__button--menu"
        }
        _ => "dxeditor-engine__button dxeditor-bubble__button",
    };
    rsx! {
        button {
            r#type: "button",
            class,
            aria_label: "{label}",
            aria_pressed: pressed.map(|pressed| pressed.to_string()),
            aria_expanded: expanded.map(|expanded| expanded.to_string()),
            aria_haspopup: menu.then_some("menu"),
            aria_keyshortcuts: (!aria_keys.is_empty()).then_some(aria_keys),
            "data-swatch": swatch,
            onmousedown: move |event| event.prevent_default(),
            onclick: move |_| onclick.call(()),
            Icon { name: icon }
            if let Some(text) = &text {
                span { class: "dxeditor-bubble__text", "{text}" }
            }
            if menu {
                Icon { name: "chevron_down" }
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
            onmousemove: follow_pointer,
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

/// A group of menu rows behind a titled row. On wide screens the rows open
/// as a flyout beside the menu; on narrow or touch screens they expand in
/// place. Groups sharing a `name` open one at a time.
#[component]
pub(super) fn MenuGroup(label: String, icon: String, name: String, children: Element) -> Element {
    rsx! {
        details {
            "name": "{name}",
            ontoggle: move |event| {
                use dioxus::web::WebEventExt;
                if let Some(group) = event
                    .data()
                    .try_as_web_event()
                    .and_then(|event| event.target())
                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                {
                    fit_submenu(&group);
                }
            },
            summary {
                class: "dxeditor-menu__row",
                aria_haspopup: "menu",
                onmousedown: move |event| event.prevent_default(),
                onmousemove: follow_pointer,
                // A flyout opened by hovering its title stays open when the
                // title is then clicked.
                onclick: move |event| {
                    if let Some(item) = pointer_target(&event)
                        && let Some(group) = item.parent_element()
                        && group.has_attribute("open")
                        && flyout(&group)
                    {
                        event.prevent_default();
                    }
                },
                Icon { name: icon }
                span { "{label}" }
                Icon { name: "chevron" }
            }
            div { class: "dxeditor-menu__submenu", role: "group", aria_label: "{label}", {children} }
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
