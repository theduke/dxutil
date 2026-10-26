use super::position::{self, Side, Size};
use crate::engine::{
    Command, EditorSelection, EditorUiState, InlineContainerRef,
    commands::{Axis, TableCommand},
    ui_state::Rect,
};
use crate::{AttributeMap, ComponentMark, EditorCatalog, EngineEvent};
use dioxus::prelude::*;
use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
use wasm_bindgen::JsCast;
#[path = "block_controls.rs"]
mod block_controls;
#[path = "bubble.rs"]
mod bubble;
#[path = "entity_preview.rs"]
mod entity_preview;
#[path = "icons.rs"]
mod icons;
#[path = "image_menu.rs"]
mod image_menu;
#[path = "slash.rs"]
mod slash;
#[path = "table_menu.rs"]
mod table_menu;

#[derive(Clone)]
pub struct EngineHandle(Rc<dyn Fn(Command)>);
impl EngineHandle {
    pub fn new(run: Rc<dyn Fn(Command)>) -> Self {
        Self(run)
    }
    pub fn run(&self, command: Command) {
        (self.0)(command);
    }
    pub fn close(&self) {
        self.run(Command::CloseMenu);
    }
    pub fn picker(&self, mode: &str) {
        self.run(Command::Request(EngineEvent::EntityPickerRequest {
            request_id: request_id(),
            mode: mode.into(),
        }));
    }
    pub fn create(&self, ui: &EditorUiState, trigger: &str, query: String) {
        self.run(Command::Request(EngineEvent::EntityCreateRequest {
            request_id: request_id(),
            query,
            trigger: trigger.into(),
            uids: ui.selection.uid.iter().cloned().collect(),
            checked: ui
                .selection
                .attrs
                .get("checked")
                .and_then(serde_json::Value::as_bool),
        }));
    }
}
impl PartialEq for EngineHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
static REQUEST: AtomicU64 = AtomicU64::new(1 << 48);
fn request_id() -> u64 {
    REQUEST.fetch_add(1, Ordering::Relaxed)
}
fn attr(value: serde_json::Value) -> AttributeMap {
    value.as_object().cloned().unwrap_or_default()
}
fn style(ui: &EditorUiState, anchor: Option<Rect>, size: Size, side: Side) -> String {
    let anchor = anchor
        .or(ui.geometry.caret)
        .or(ui.geometry.block)
        .unwrap_or_default();
    let viewport = ui.geometry.viewport;
    let viewport = if viewport.width > 0.0 {
        viewport
    } else {
        Rect {
            x: 0.,
            y: 0.,
            width: 800.,
            height: 600.,
        }
    };
    let sx = if ui.geometry.scale_x > 0.0 {
        ui.geometry.scale_x
    } else {
        1.
    };
    let sy = if ui.geometry.scale_y > 0.0 {
        ui.geometry.scale_y
    } else {
        1.
    };
    let placed = position::place(
        position::Rect {
            x: anchor.x,
            y: anchor.y,
            width: anchor.width,
            height: anchor.height,
        },
        Size {
            width: size.width / sx,
            height: size.height / sy,
        },
        position::Rect {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width,
            height: viewport.height,
        },
        side,
        8. / sy,
    );
    format!(
        "position:absolute;left:{}px;top:{}px;max-height:{}px;z-index:10;pointer-events:auto;transform:scale({},{});transform-origin:top left",
        placed.x,
        placed.y,
        (placed.max_height * sy).max(80.),
        1. / sx,
        1. / sy
    )
}
fn suggestion_style(ui: &EditorUiState) -> String {
    let sx = if ui.geometry.scale_x > 0.0 {
        ui.geometry.scale_x
    } else {
        1.
    };
    let sy = if ui.geometry.scale_y > 0.0 {
        ui.geometry.scale_y
    } else {
        1.
    };
    let viewport = ui.geometry.viewport;
    if viewport.width * sx > 0.0 && viewport.width * sx <= 640. {
        let height = (viewport.height * sy * 0.55).min(420.);
        format!(
            "position:absolute!important;left:{}px!important;top:{}px!important;bottom:auto!important;width:{}px!important;max-height:{}px!important;box-sizing:border-box!important;min-width:0!important;max-width:none!important;z-index:10;pointer-events:auto;transform:scale({},{});transform-origin:top left",
            viewport.x,
            viewport.y + viewport.height - height / sy,
            viewport.width * sx,
            height,
            1. / sx,
            1. / sy
        )
    } else {
        style(
            ui,
            ui.geometry.caret,
            Size {
                width: 320.,
                height: 360.,
            },
            Side::Bottom,
        )
    }
}
fn menu_keys(event: KeyboardEvent, handle: EngineHandle) {
    let key = event.key().to_string();
    if key == "Escape" {
        event.prevent_default();
        event.stop_propagation();
        handle.close();
        return;
    }
    if !matches!(
        key.as_str(),
        "ArrowDown" | "ArrowUp" | "ArrowLeft" | "ArrowRight" | "Home" | "End"
    ) {
        return;
    }
    use dioxus::web::WebEventExt;
    let Some(web) = event.data().try_as_web_event() else {
        return;
    };
    let Some(target) = web
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    if target
        .matches("input,textarea,[contenteditable=true]")
        .unwrap_or(false)
    {
        return;
    }
    let Ok(Some(menu)) = target.closest("[role=menu],[role=toolbar]") else {
        return;
    };
    let Ok(buttons) = menu.query_selector_all("button:not([disabled]),input,summary") else {
        return;
    };
    let mut choices = Vec::new();
    for i in 0..buttons.length() {
        if let Some(element) = buttons
            .item(i)
            .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        {
            if element.get_client_rects().length() > 0 {
                choices.push(element);
            }
        }
    }
    if choices.is_empty() {
        return;
    }
    let current = choices
        .iter()
        .position(|e| e.is_same_node(Some(&target)))
        .unwrap_or(0);
    let next = match key.as_str() {
        "Home" => 0,
        "End" => choices.len() - 1,
        "ArrowUp" | "ArrowLeft" => (current + choices.len() - 1) % choices.len(),
        _ => (current + 1) % choices.len(),
    };
    for (index, element) in choices.iter().enumerate() {
        element.set_tab_index(if index == next { 0 } else { -1 });
    }
    let _ = choices[next].focus();
    event.prevent_default();
    event.stop_propagation();
}
fn roving(element: &web_sys::Element) {
    if let Ok(items) = element.query_selector_all("button:not([disabled]),input,summary") {
        let active = element
            .owner_document()
            .and_then(|document| document.active_element());
        let choices: Vec<_> = (0..items.length())
            .filter_map(|index| {
                items
                    .item(index)
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
            })
            .filter(|item| item.get_client_rects().length() > 0)
            .collect();
        let selected = choices
            .iter()
            .position(|item| {
                active
                    .as_ref()
                    .is_some_and(|active| item.is_same_node(Some(active)))
            })
            .unwrap_or(0);
        for (index, item) in choices.iter().enumerate() {
            item.set_tab_index(if index == selected { 0 } else { -1 });
        }
    }
}
fn init_roving(event: MountedEvent) {
    use dioxus::web::WebEventExt;
    if let Some(element) = event.data().try_as_web_event() {
        roving(&element);
    }
}
fn roving_focus(event: FocusEvent) {
    use dioxus::web::WebEventExt;
    if let Some(element) = event
        .data()
        .try_as_web_event()
        .and_then(|event| event.target())
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|target| target.closest("[role=toolbar],[role=menu]").ok().flatten())
    {
        roving(&element);
    }
}
fn focus_input(event: MountedEvent) {
    use dioxus::web::WebEventExt;
    if let Some(element) = event.data().try_as_web_event() {
        if let Some(input) = element.dyn_ref::<web_sys::HtmlElement>() {
            let _ = input.focus();
        }
    }
}
fn focus_first(event: MountedEvent) {
    use dioxus::web::WebEventExt;
    if let Some(element) = event.data().try_as_web_event() {
        roving(&element);
        if let Ok(Some(button)) = element.query_selector("[tabindex='0']") {
            if let Some(button) = button.dyn_ref::<web_sys::HtmlElement>() {
                let _ = button.focus();
            }
        }
    }
}
#[component]
fn ActionButton(
    label: String,
    handle: EngineHandle,
    command: Command,
    #[props(default)] pressed: bool,
    #[props(default)] shortcut: String,
    #[props(default)] destructive: bool,
) -> Element {
    let mac = web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .is_some_and(|platform| {
            platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
        });
    let shortcut = if mac {
        shortcut.replace("Control+", "Meta+")
    } else {
        shortcut
    };
    let shortcut_display = if mac {
        shortcut
            .replace("Meta+", "⌘")
            .replace("Shift+", "⇧")
            .replace("Alt+", "⌥")
    } else {
        shortcut.replace("Control+", "Ctrl+")
    };
    let title = if shortcut.is_empty() {
        label.clone()
    } else {
        format!("{label} ({shortcut_display})")
    };
    let text = match label.as_str() {
        "Bold" => "B",
        "Italic" => "I",
        "Underline" => "U",
        "Strike" => "S",
        "Code" => "</>",
        other => other,
    };
    rsx! {button{r#type:"button",class:if destructive{"dxeditor-engine__button dxeditor-table-destructive"}else{"dxeditor-engine__button"},title:"{title}",aria_label:"{label}",aria_pressed:pressed.to_string(),aria_keyshortcuts:"{shortcut}",onmousedown:move|e|e.prevent_default(),onclick:move |_|handle.run(command.clone()),"{text}"}}
}
#[component]
fn MenuAction(
    label: String,
    handle: EngineHandle,
    command: Command,
    #[props(default)] checked: Option<bool>,
    #[props(default)] destructive: bool,
) -> Element {
    rsx! {button{r#type:"button",role:if checked.is_some(){"menuitemcheckbox"}else{"menuitem"},aria_checked:checked.map(|c|c.to_string()),class:if destructive{"dxeditor-engine__button dxeditor-table-destructive"}else{"dxeditor-engine__button"},onmousedown:move|e|e.prevent_default(),onclick:move |_|{handle.run(command.clone());handle.close();},"{label}"}}
}
#[component]
fn Colors(
    handle: EngineHandle,
    #[props(default)] block: bool,
    #[props(default)] background: bool,
) -> Element {
    let mut colors = vec![None];
    colors.extend(
        crate::profiles::block_document::COLOR_TOKENS
            .into_iter()
            .map(|s| Some(s.to_string())),
    );
    rsx! {div{class:"dxeditor-color-swatches",role:"group",aria_label:if background{"Highlight color"}else{"Text color"},for color in colors{ {let key=if background{"background_color"}else{"text_color"};let mark_kind=if background{"highlight"}else{"text_color"};let label=color.as_ref().map(|s|format!("{s} {}",if background{"background"}else{"text"})).unwrap_or_else(||"Default color".into());let command=if block{Command::SetColor{key:key.into(),color:color.clone()}}else if let Some(color)=&color{let mut mark=ComponentMark::new(mark_kind);mark.attrs.insert("color".into(),serde_json::json!(color));Command::SetMark(mark)}else{Command::UnsetMark(mark_kind.into())};let persist=color.clone();let handle=handle.clone();rsx!{button{r#type:"button",class:"dxeditor-engine__button",aria_label:"{label}",title:"{label}","data-color":color.clone().unwrap_or_default(),onmousedown:move|e|e.prevent_default(),onclick:move |_|{if let Some(color)=&persist{if let Some(storage)=web_sys::window().and_then(|w|w.local_storage().ok().flatten()){let _=storage.set_item(if background{"dxeditor-last-highlight"}else{"dxeditor-last-textColor"},color);}}handle.run(command.clone());},"●"}}} }}}
}
#[component]
pub fn EditorSurfaces(
    state: ReadSignal<EditorUiState>,
    handle: EngineHandle,
    catalog: EditorCatalog,
) -> Element {
    let ui = state();
    if ui.composing {
        return rsx! {};
    }
    if ui.read_only {
        return rsx! {style{dangerous_inner_html:SURFACE_STYLE}entity_preview::EntityPreview{ui,handle}};
    }
    rsx! {
        style{dangerous_inner_html:SURFACE_STYLE}
        if let Some(menu)=&ui.open_menu {
            match menu.as_str(){"image-url"=>rsx!{image_menu::ImageUrlPopover{ui:ui.clone(),handle:handle.clone()}},"bookmark"=>rsx!{bubble::BookmarkPopover{ui:ui.clone(),handle:handle.clone()}},"link"=>rsx!{bubble::LinkPopover{ui:ui.clone(),handle:handle.clone()}},"block"=>rsx!{block_controls::BlockMenu{ui:ui.clone(),handle:handle.clone(),catalog:catalog.clone()}},"table"|"table-row"|"table-column"=>rsx!{table_menu::TableMenu{ui:ui.clone(),handle:handle.clone()}},"image"=>rsx!{image_menu::ImageMenu{ui:ui.clone(),handle:handle.clone()}},_=>rsx!{}}
        } else if ui.suggestion.as_ref().is_some_and(|suggestion|suggestion.manual){slash::Suggestions{ui:ui.clone(),handle:handle.clone()}}
        else if ui.image.is_some(){image_menu::ImageMenu{ui:ui.clone(),handle:handle.clone()}}
        else if ui.table.is_some(){table_menu::TableEdges{ui:ui.clone(),handle:handle.clone()}}
        else if ui.focused&&!ui.selection.collapsed&&matches!(ui.selection.selection,EditorSelection::Text(_)){bubble::Bubble{ui:ui.clone(),handle:handle.clone(),catalog:catalog.clone()}}
        else if ui.suggestion.is_some(){slash::Suggestions{ui:ui.clone(),handle:handle.clone()}}
        if ui.entity_preview.is_some(){entity_preview::EntityPreview{ui:ui.clone(),handle:handle.clone()}}
        if ui.open_menu.is_none()&&ui.table.is_none()&&ui.image.is_none()&&ui.suggestion.is_none()&&!(ui.focused&&!ui.selection.collapsed&&matches!(ui.selection.selection,EditorSelection::Text(_)))&&ui.selection.kind.as_deref()!=Some("chip"){block_controls::BlockControls{ui:ui,handle,catalog}}
    }
}
const SURFACE_STYLE: &str = r#"
.dxeditor-engine__surface{display:flex;gap:4px;padding:6px;border:1px solid var(--dx-border,#cfd8e3);border-radius:8px;background:var(--dx-surface,white);box-shadow:0 4px 16px #0002;color:var(--dx-text,#17202a);box-sizing:border-box;overflow:auto;max-width:calc(100vw - 16px)}
.dxeditor-engine__button{min-width:28px;min-height:28px;border:0;border-radius:4px;padding:5px 8px;color:inherit;background:transparent;font:inherit;cursor:pointer;text-align:left}
.dxeditor-engine__button:hover,.dxeditor-engine__button[aria-pressed=true],.dxeditor-engine__button[aria-selected=true]{background:var(--dx-surface-muted,#edf2f5)}
.dxeditor-engine__surface button:focus-visible,.dxeditor-engine__surface input:focus-visible{outline:2px solid var(--dx-accent,#176b87);outline-offset:1px}
.dxeditor-engine__surface[role=menu]{flex-direction:column;min-width:200px}.dxeditor-engine__surface input{min-height:30px;max-width:100%;box-sizing:border-box;border:1px solid var(--dx-border,#cfd8e3);border-radius:4px;padding:4px 8px;font:inherit}
.dxeditor-color-swatches [data-color=gray]{color:#737b86}.dxeditor-color-swatches [data-color=brown]{color:#986b4c}.dxeditor-color-swatches [data-color=orange]{color:#c06a22}.dxeditor-color-swatches [data-color=yellow]{color:#a17a12}.dxeditor-color-swatches [data-color=green]{color:#38865d}.dxeditor-color-swatches [data-color=blue]{color:#387ab7}.dxeditor-color-swatches [data-color=purple]{color:#8462b5}.dxeditor-color-swatches [data-color=pink]{color:#b34f8b}.dxeditor-color-swatches [data-color=red]{color:#be4f4f}
.dxeditor-color-swatches{display:flex;flex-wrap:wrap;max-width:240px}.dxeditor-bubble__separator{width:1px;background:var(--dx-border,#cfd8e3);margin:3px}.dxeditor-engine__slash,.dxeditor-engine__mentions{flex-direction:column;width:320px;max-height:360px}.dxeditor-engine__slash [role=option],.dxeditor-engine__mentions [role=option]{display:flex;align-items:center;gap:8px;width:100%;flex-shrink:0;min-height:44px}.dxeditor-slash-copy{display:flex;flex-direction:column;flex:1}.dxeditor-slash-description{font-size:12px;color:var(--dx-text-muted,#617084)}.dxeditor-menu-group{padding:6px;font-size:12px;color:var(--dx-text-muted,#617084)}.dxeditor-table-destructive{color:var(--semantic-color-danger,#b42318)}
.dxeditor-gutter{display:flex;gap:2px;position:absolute;z-index:4;pointer-events:auto}.dxeditor-gutter button{min-width:32px;min-height:32px;padding:4px}.dxeditor-table-edges{position:absolute;pointer-events:none;z-index:5}.dxeditor-table-edges button{pointer-events:auto;position:absolute}.dxeditor-image-menu{flex-wrap:wrap;max-width:340px}.dxeditor-image-menu label{display:flex;flex-direction:column;font-size:12px}.dxeditor-image-menu input{width:220px}
@media(max-width:640px){.dxeditor-engine__slash,.dxeditor-engine__mentions{border-radius:12px 12px 0 0;padding:12px 12px max(12px,env(safe-area-inset-bottom))}.dxeditor-engine__button{min-height:32px}.dxeditor-gutter{flex-direction:column}.dxeditor-engine__bubble{flex-wrap:wrap;max-width:calc(100vw - 16px)}}
@media(prefers-reduced-motion:reduce){.dxeditor-engine__surface{scroll-behavior:auto}}
"#;
