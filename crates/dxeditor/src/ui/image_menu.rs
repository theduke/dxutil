use super::*;
#[component]
pub(super) fn ImageMenu(ui: EditorUiState, handle: EngineHandle) -> Element {
    let Some(image) = ui.image.clone() else {
        return rsx! {};
    };
    let mut alt = use_signal(|| {
        image
            .attrs
            .get("alt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let uid = image.uid.clone();
    let placement = style(
        &ui,
        ui.geometry.block,
        Size {
            width: 320.,
            height: 150.,
        },
        Side::Top,
    );
    let keys = handle.clone();
    let alt_handle = handle.clone();
    let alt_uid = uid.clone();
    let alt_keys = handle.clone();
    let alt_key_uid = uid.clone();
    let replace_uid = uid.clone();
    let download_uid = uid.clone();
    let open_uid = uid.clone();
    let upload = image.transient.get("upload");
    let failed = upload
        .and_then(|u| u.get("status"))
        .and_then(serde_json::Value::as_str)
        == Some("failed");
    let missing_media = image
        .attrs
        .get("media")
        .is_none_or(serde_json::Value::is_null);
    rsx! {div{class:"dxeditor-engine__surface dxeditor-image-menu",role:"toolbar",onmounted:init_roving,onfocusin:roving_focus,aria_label:"Image controls",style:"{placement}",onkeydown:move|event|menu_keys(event,keys.clone()),
        if ui.profile!=crate::engine::Profile::MarkdownNotes {for alignment in ["start","center","end"]{ActionButton{label:format!("Align {alignment}"),handle:handle.clone(),command:Command::Image{uid:uid.clone(),attrs:attr(serde_json::json!({"alignment":alignment}))},pressed:image.attrs.get("alignment").and_then(serde_json::Value::as_str)==Some(alignment)}}
        for width in [25,50,75,100]{ActionButton{label:format!("{width}%"),handle:handle.clone(),command:Command::Image{uid:uid.clone(),attrs:attr(serde_json::json!({"width":width}))},pressed:image.attrs.get("width").and_then(serde_json::Value::as_i64)==Some(width)}}}
        label{"Alt text" input{r#type:"text",aria_label:"Image alt text",value:"{alt}",oninput:move|e|alt.set(e.value()),onkeydown:move|event:KeyboardEvent|{use dioxus::web::WebEventExt;if event.data().try_as_web_event().is_some_and(|web|web.is_composing()||web.key_code()==229){return;}if matches!(event.key().to_string().as_str(),"Tab"|"Enter"){event.prevent_default();event.stop_propagation();alt_keys.run(Command::Image{uid:alt_key_uid.clone(),attrs:attr(serde_json::json!({"alt":alt()}))});alt_keys.run(Command::Focus);}},onchange:move |_|alt_handle.run(Command::Image{uid:alt_uid.clone(),attrs:attr(serde_json::json!({"alt":alt()}))})}}
        if ui.profile==crate::engine::Profile::MarkdownNotes {ActionButton{label:"Edit image URL and title".to_string(),handle:handle.clone(),command:Command::OpenMenu("image-url".into())}ActionButton{label:"Replace image".to_string(),handle:handle.clone(),command:Command::OpenMenu("image-url".into())}}
        else{ActionButton{label:"Replace image".to_string(),handle:handle.clone(),command:Command::Request(EngineEvent::FilePickerRequest{uid:replace_uid,accept:"image/*".into()})}}
        button{r#type:"button",class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onclick:move |_|image_link(&download_uid,true),"Download image"}
        button{r#type:"button",class:"dxeditor-engine__button",onmousedown:move|e|e.prevent_default(),onclick:move |_|image_link(&open_uid,false),"Open image"}
        if failed{ActionButton{label:"Retry upload".to_string(),handle:handle.clone(),command:Command::Request(EngineEvent::UploadRetry{uid:uid.clone()})}}
        if missing_media&&ui.profile!=crate::engine::Profile::MarkdownNotes{ActionButton{label:"Upload again".to_string(),handle:handle.clone(),command:Command::Request(EngineEvent::FilePickerRequest{uid:uid.clone(),accept:"image/*".into()})}}
        ActionButton{label:"Remove image".to_string(),handle,command:Command::RemoveNode(uid),destructive:true}
    }}
}
fn image_link(uid: &str, download: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Ok(Some(image)) = document.query_selector(&format!("[data-uid=\"{uid}\"] img")) else {
        return;
    };
    let Some(url) = image
        .get_attribute("src")
        .filter(|url| crate::is_safe_media_url(url))
    else {
        return;
    };
    if download {
        if let Ok(anchor) = document.create_element("a") {
            let _ = anchor.set_attribute("href", &url);
            let _ = anchor.set_attribute(
                "download",
                &image
                    .get_attribute("alt")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "image".into()),
            );
            if let Ok(anchor) = anchor.dyn_into::<web_sys::HtmlElement>() {
                anchor.click();
            }
        }
    } else {
        let _ = window.open_with_url_and_target_and_features(&url, "_blank", "noopener");
    }
}
#[component]
pub(super) fn ImageUrlPopover(ui: EditorUiState, handle: EngineHandle) -> Element {
    let mut url = use_signal(|| {
        ui.image
            .as_ref()
            .and_then(|image| image.attrs.get("media"))
            .and_then(|media| media.get("url"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let mut alt = use_signal(|| {
        ui.image
            .as_ref()
            .and_then(|image| image.attrs.get("alt"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let mut title = use_signal(|| {
        ui.image
            .as_ref()
            .and_then(|image| image.attrs.get("title"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let mut error = use_signal(String::new);
    let keys = handle.clone();
    let submit = handle.clone();
    let uid = ui.image.as_ref().map(|image| image.uid.clone());
    let enter_uid = uid.clone();
    let placement = style(
        &ui,
        ui.geometry.block.or(ui.geometry.caret),
        Size {
            width: 320.,
            height: 180.,
        },
        Side::Bottom,
    );
    rsx! {div{class:"dxeditor-engine__surface dxeditor-image-menu",role:"dialog",aria_label:"Image properties",style:"{placement}",onkeydown:move|event|{if event.key().to_string()=="Enter"{event.prevent_default();apply_url(&submit,enter_uid.clone(),url(),alt(),title(),&mut error);}else{menu_keys(event,keys.clone());}},
        label{"Image URL" input{r#type:"url",aria_label:"Image URL",autofocus:true,onmounted:focus_input,value:"{url}",oninput:move|event|url.set(event.value())}}
        label{"Alt text" input{r#type:"text",aria_label:"Image alt text",value:"{alt}",oninput:move|event|alt.set(event.value())}}
        label{"Title" input{r#type:"text",aria_label:"Image title",value:"{title}",oninput:move|event|title.set(event.value())}}
        button{r#type:"button",class:"dxeditor-engine__button",aria_label:"Apply image",onmousedown:move|event|event.prevent_default(),onclick:move |_|apply_url(&handle,uid.clone(),url(),alt(),title(),&mut error),"Apply image"}
        if !error().is_empty(){span{role:"alert","{error}"}}
    }}
}
fn apply_url(
    handle: &EngineHandle,
    uid: Option<String>,
    url: String,
    alt: String,
    title: String,
    error: &mut Signal<String>,
) {
    if !crate::is_safe_media_url(&url) || url.trim().is_empty() {
        error.set("Enter a valid image URL".into());
        return;
    }
    let attrs =
        attr(serde_json::json!({"media":{"type":"url","url":url.trim()},"alt":alt,"title":title}));
    if let Some(uid) = uid {
        handle.run(Command::Image { uid, attrs });
    } else {
        let mut node = crate::ComponentNode::new("image");
        node.attrs = attrs;
        handle.run(Command::InsertBlocks {
            at: crate::InsertAt::Selection,
            nodes: vec![crate::engine::model::block_node(node)],
        });
    }
    handle.close();
}
