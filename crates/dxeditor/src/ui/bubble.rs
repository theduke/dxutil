use super::*;
#[component]
pub(super) fn Bubble(ui: EditorUiState, handle: EngineHandle, catalog: EditorCatalog) -> Element {
    let placement = style(
        &ui,
        ui.geometry.selection,
        Size {
            width: 480.,
            height: 44.,
        },
        Side::Top,
    );
    let marks = [
        ("bold", "Bold", "Control+B"),
        ("italic", "Italic", "Control+I"),
        ("underline", "Underline", "Control+U"),
        ("strike", "Strike", "Control+Shift+S"),
        ("code", "Code", "Control+E"),
    ];
    let keys = handle.clone();
    let create_handle = handle.clone();
    let create_ui = ui.clone();
    rsx! {div{class:"dxeditor-engine__surface dxeditor-engine__bubble",role:"toolbar",onmounted:init_roving,onfocusin:roving_focus,aria_label:"Text formatting",style:"{placement}",onkeydown:move|event|menu_keys(event,keys.clone()),
        details{summary{aria_label:"Turn into","Turn into"}div{role:"menu",aria_label:"Turn into",for spec in catalog.component_specs().specs().filter(|s|crate::engine::model::is_text_kind(&s.id.0)){ {let h=handle.clone();let kind=spec.id.0.clone();let label=spec.label.clone();rsx!{MenuAction{label,handle:h,command:Command::TurnInto{kind,attrs:AttributeMap::new()},checked:Some(ui.selection.kind.as_ref()==Some(&spec.id.0))}}} }} }
        span{role:"separator",class:"dxeditor-bubble__separator"}
        for (kind,label,shortcut) in marks.into_iter().filter(|(kind,_,_)|catalog.component_specs().spec_by_id(kind).is_some()){
            ActionButton{label:label.to_string(),shortcut:shortcut.to_string(),handle:handle.clone(),command:Command::ToggleMark(ComponentMark::new(kind)),pressed:ui.selection.active_marks.iter().any(|m|m.kind.0==kind)}
        }
        if catalog.component_specs().spec_by_id("highlight").is_some(){details{summary{aria_label:"Colour","Colour"}div{role:"menu",aria_label:"Colors",Colors{handle:handle.clone()}Colors{handle:handle.clone(),background:true}}}}
        span{role:"separator",class:"dxeditor-bubble__separator"}
        ActionButton{label:"Link".to_string(),shortcut:"Control+K".to_string(),handle:handle.clone(),command:Command::OpenMenu("link".into()),pressed:ui.selection.link.is_some()}
        if catalog.component_specs().spec_by_id("entity_link").is_some(){button{r#type:"button",class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onclick:{let h=handle.clone();move |_|h.picker("link")},"Link to entity…"}}
        if catalog.component_specs().spec_by_id("entity_embed").is_some(){button{r#type:"button",class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onclick:move |_|create_handle.create(&create_ui,"selection",String::new()),"Create from selection…"}}
    }}
}
#[component]
pub(super) fn LinkPopover(ui: EditorUiState, handle: EngineHandle) -> Element {
    let mut value = use_signal(|| ui.selection.link.clone().unwrap_or_default());
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
    let placement = style(
        &ui,
        ui.geometry.selection,
        Size {
            width: 320.,
            height: if ui.profile == crate::engine::Profile::MarkdownNotes {
                140.
            } else {
                100.
            },
        },
        Side::Bottom,
    );
    let keys = handle.clone();
    let apply_handle = handle.clone();
    let enter_handle = handle.clone();
    rsx! {div{class:"dxeditor-engine__surface dxeditor-engine__link-popover",role:"dialog",aria_label:"Edit link",style:"{placement};flex-wrap:wrap",onkeydown:move|event|{if event.key().to_string()=="Enter"{event.prevent_default();apply_link(&enter_handle,&value(),&title(),&mut error);}else{menu_keys(event,keys.clone());}},
        input{r#type:"url",autofocus:true,onmounted:focus_input,placeholder:"Paste or type a link",aria_label:"Link URL",value:"{value}",oninput:move|event|value.set(event.value())}
        if ui.profile==crate::engine::Profile::MarkdownNotes{input{r#type:"text",aria_label:"Link title",placeholder:"Title (optional)",value:"{title}",oninput:move|event|title.set(event.value())}}
        button{r#type:"button",class:"dxeditor-engine__button",aria_label:"Apply link",onmousedown:move|e|e.prevent_default(),onclick:move |_|apply_link(&apply_handle,&value(),&title(),&mut error),"Apply"}
        if ui.selection.link.is_some(){button{r#type:"button",class:"dxeditor-engine__button",aria_label:"Remove link",onmousedown:move|e|e.prevent_default(),onclick:move |_|{handle.run(Command::UnsetMark("link".into()));handle.close();},"Remove"}}
        button{r#type:"button",class:"dxeditor-engine__button",aria_label:"Open link",onmousedown:move|event|event.prevent_default(),onclick:move |_|{if crate::is_safe_url(&value()){if let Some(window)=web_sys::window(){let _=window.open_with_url_and_target_and_features(&value(),"_blank","noopener");}}},"Open"}
        if !error().is_empty(){span{role:"alert","{error}"}}
    }}
}
fn apply_link(handle: &EngineHandle, value: &str, title: &str, error: &mut Signal<String>) {
    if !crate::is_safe_url(value) || value.trim().is_empty() {
        error.set("Enter a valid URL".into());
        return;
    }
    let mut mark = ComponentMark::new("link");
    mark.attrs
        .insert("href".into(), serde_json::json!(value.trim()));
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
            width: 320.,
            height: 90.,
        },
        Side::Bottom,
    );
    let apply = move |handle: &EngineHandle, value: String, error: &mut Signal<String>| {
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
        handle.close();
    };
    rsx! {div{class:"dxeditor-engine__surface",role:"dialog",aria_label:"Insert web bookmark",style:"{placement}",onkeydown:move|event|{if event.key().to_string()=="Enter"{event.prevent_default();apply(&submit,value(),&mut error);}else{menu_keys(event,keys.clone());}},
        input{r#type:"url",aria_label:"Bookmark URL",placeholder:"Paste a web address",autofocus:true,onmounted:focus_input,value:"{value}",oninput:move|event|value.set(event.value())}
        button{r#type:"button",class:"dxeditor-engine__button",onmousedown:move|event|event.prevent_default(),onclick:move |_|apply(&handle,value(),&mut error),"Insert"}
        if !error().is_empty(){span{role:"alert","{error}"}}
    }}
}
