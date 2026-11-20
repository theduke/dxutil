use super::controls::IconButton;
use super::*;

/// The image toolbar's approximate width.
const TOOLBAR_WIDTH: f64 = 520.;
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
    // The toolbar overlays a wide image's top corner. Beside a narrow image
    // it uses the free column space next to it; only when there is none does
    // it uses the top edge of the image.
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
    let width = TOOLBAR_WIDTH / sx;
    let gap = 8. / sx;
    let beside = ui.geometry.block_line.zip(ui.geometry.block).and_then(|(image, column)| {
        let right = image.x + image.width;
        let left = if image.width >= width + 2. * gap {
            Some(right - gap - width)
        } else if column.x + column.width - right >= width + gap {
            Some(right + gap)
        } else if image.x - column.x >= width + gap {
            Some(image.x - gap - width)
        } else {
            None
        }?;
        Some(format!(
            "position:absolute;left:{left}px;top:{}px;z-index:10;pointer-events:auto;transform:scale({},{});transform-origin:top left",
            image.y + 8. / sy,
            1. / sx,
            1. / sy
        ))
    });
    let placement = beside.unwrap_or_else(|| {
        let bounds = ui.geometry.block.unwrap_or_default();
        let top = ui.geometry.block_line.unwrap_or(bounds).y;
        format!(
            "position:absolute;left:{}px;top:{}px;max-width:{}px;z-index:10;pointer-events:auto;transform:scale({},{});transform-origin:top left",
            bounds.x,
            top + 4. / sy,
            bounds.width * sx,
            1. / sx,
            1. / sy
        )
    });
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
    let notes = ui.profile == crate::engine::Profile::MarkdownNotes;
    let run = move |handle: &EngineHandle, command: Command| {
        let handle = handle.clone();
        move |_| handle.run(command.clone())
    };
    let set = move |handle: &EngineHandle, uid: &str, attrs: serde_json::Value| {
        run(
            handle,
            Command::Image {
                uid: uid.to_string(),
                attrs: attr(attrs),
            },
        )
    };
    let alignment = image
        .attrs
        .get("alignment")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("start")
        .to_owned();
    let width = image.attrs.get("width").and_then(serde_json::Value::as_i64);
    let replace = if notes {
        Command::OpenMenu("image-url".into())
    } else {
        Command::Request(EngineEvent::FilePickerRequest {
            uid: replace_uid,
            accept: "image/*".into(),
            placeholder: false,
        })
    };
    rsx! {
        div {
            class: "dxeditor-engine__surface dxeditor-engine__bubble dxeditor-image-menu",
            role: "toolbar",
            aria_label: "Image controls",
            style: "{placement}",
            onmounted: init_roving,
            onfocusin: roving_focus,
            onkeydown: move |event| menu_keys(event, keys.clone()),
            if !notes {
                for (value , label , icon) in [("start", "Align left", "align_left"), ("center", "Align centre", "align_center"), ("end", "Align right", "align_right")] {
                    IconButton {
                        label,
                        icon,
                        pressed: Some(alignment == value),
                        onclick: set(&handle, &uid, serde_json::json!({ "alignment": value })),
                    }
                }
                span { role: "separator", class: "dxeditor-bubble__separator" }
                for preset in [25, 50, 75, 100] {
                    button {
                        r#type: "button",
                        class: "dxeditor-engine__button dxeditor-image-menu__width",
                        aria_label: "{preset}%",
                        aria_pressed: (width == Some(preset)).to_string(),
                        onmousedown: move |event| event.prevent_default(),
                        onclick: {
                            let set_width = set(&handle, &uid, serde_json::json!({ "width": preset }));
                            move |_| set_width(())
                        },
                        "{preset}%"
                    }
                }
                span { role: "separator", class: "dxeditor-bubble__separator" }
            }
            input {
                r#type: "text",
                class: "dxeditor-image-menu__alt",
                aria_label: "Image alt text",
                placeholder: "Alt text",
                value: "{alt}",
                oninput: move |e| alt.set(e.value()),
                onkeydown: move |event: KeyboardEvent| {
                    use dioxus::web::WebEventExt;
                    if event.data().try_as_web_event().is_some_and(|web| web.is_composing() || web.key_code() == 229) {
                        return;
                    }
                    if matches!(event.key(), Key::Enter | Key::Tab) {
                        if event.key() == Key::Enter {
                            event.prevent_default();
                            event.stop_propagation();
                        }
                        alt_keys.run(Command::Image {
                            uid: alt_key_uid.clone(),
                            attrs: attr(serde_json::json!({ "alt": alt() })),
                        });
                        if event.key() == Key::Enter {
                            alt_keys.run(Command::Focus);
                        }
                    }
                },
                onchange: move |_| alt_handle.run(Command::Image {
                    uid: alt_uid.clone(),
                    attrs: attr(serde_json::json!({ "alt": alt() })),
                }),
            }
            span { role: "separator", class: "dxeditor-bubble__separator" }
            if notes {
                IconButton {
                    label: "Edit image URL and title",
                    icon: "link",
                    onclick: run(&handle, Command::OpenMenu("image-url".into())),
                }
            }
            IconButton { label: "Replace image", icon: "image", onclick: run(&handle, replace) }
            if failed {
                IconButton {
                    label: "Retry upload",
                    icon: "retry",
                    onclick: run(&handle, Command::Request(EngineEvent::UploadRetry { uid: uid.clone() })),
                }
            }
            if missing_media && !notes {
                IconButton {
                    label: "Upload again",
                    icon: "upload",
                    onclick: run(&handle, Command::Request(EngineEvent::FilePickerRequest { uid: uid.clone(), accept: "image/*".into(), placeholder: false })),
                }
            }
            IconButton {
                label: "Download image",
                icon: "download",
                onclick: move |_| image_link(&download_uid, true),
            }
            IconButton {
                label: "Open image",
                icon: "open",
                onclick: move |_| image_link(&open_uid, false),
            }
            IconButton {
                label: "Remove image",
                icon: "delete",
                onclick: run(&handle, Command::RemoveNode(uid.clone())),
            }
        }
    }
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
