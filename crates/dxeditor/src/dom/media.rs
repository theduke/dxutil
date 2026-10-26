//! Image URL resolution and transient upload UI. Only safe HTTP(S) media URLs
//! and previews created by this page can become image sources.
use crate::engine::{EditorEngine, EngineOutput, RenderPlan, model};
use std::{collections::BTreeMap, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlProgressElement};

#[derive(Default)]
pub struct Media {
    next: u64,
    pending: BTreeMap<u64, (String, f64)>,
    // Failed and timed-out lookups are cached too, preventing a request storm.
    resolved: BTreeMap<String, Option<String>>,
    previews: BTreeMap<String, String>,
}
impl Media {
    pub fn refresh(
        &mut self,
        root: &Element,
        engine: &EditorEngine,
        dirty: Option<&[String]>,
    ) -> Vec<EngineOutput> {
        let mut output = Vec::new();
        let now = web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0., |performance| performance.now());
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, (_, started))| now - *started >= 10_000.)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some((key, _)) = self.pending.remove(&id) {
                self.resolved.insert(key, None);
            }
        }
        let images: Vec<_> = if let Some(ids) = dirty {
            ids.iter()
                .filter_map(|uid| {
                    engine
                        .content(uid)
                        .filter(|node| node.kind.0 == "image")
                        .map(|node| (uid.clone(), node))
                })
                .collect()
        } else {
            fn collect<'a>(
                node: &'a crate::ComponentNode,
                output: &mut Vec<(String, &'a crate::ComponentNode)>,
            ) {
                if node.kind.0 == "block" {
                    if let (Some(uid), Some(content)) = (&node.id, node.content.first()) {
                        if content.kind.0 == "image" {
                            output.push((uid.0.clone(), content));
                        }
                    }
                }
                for child in &node.content {
                    collect(child, output);
                }
            }
            let mut images = Vec::new();
            collect(&engine.document().root, &mut images);
            images
        };
        self.previews.retain(|uid, url| {
            let keep = engine.index.paths.contains_key(uid);
            if !keep {
                let _ = web_sys::Url::revoke_object_url(url);
            }
            keep
        });
        for (uid, node) in images {
            let Ok(Some(figure)) = root.query_selector(&format!("[data-uid='{uid}'] figure"))
            else {
                continue;
            };
            let Ok(Some(image)) = figure.query_selector("img") else {
                continue;
            };
            let media = node
                .attrs
                .get("media")
                .and_then(serde_json::Value::as_object);
            let key = media.map(|media| serde_json::to_string(media).unwrap_or_default());
            if figure.get_attribute("data-media-key").as_ref() != key.as_ref() {
                let _ = figure.remove_attribute("data-image-error");
                if let Some(key) = &key {
                    let _ = figure.set_attribute("data-media-key", key);
                } else {
                    let _ = figure.remove_attribute("data-media-key");
                }
            }
            let upload = engine
                .transient(&uid)
                .and_then(|attrs| attrs.get("upload"))
                .and_then(serde_json::Value::as_object);
            let state = upload
                .and_then(|upload| upload.get("status"))
                .and_then(serde_json::Value::as_str);
            if let Some(state) = state {
                let _ = figure.set_attribute("data-upload-state", state);
            } else {
                let _ = figure.remove_attribute("data-upload-state");
            }
            let direct = media
                .filter(|media| {
                    media.get("type").and_then(serde_json::Value::as_str) == Some("url")
                })
                .and_then(|media| media.get("url"))
                .and_then(serde_json::Value::as_str)
                .filter(|url| crate::is_safe_media_url(url));
            if media.is_some_and(|media| {
                media.get("type").and_then(serde_json::Value::as_str) == Some("url")
            }) && direct.is_none()
            {
                if let Some(key) = &key {
                    self.resolved.insert(key.clone(), None);
                }
            }
            let resolved = key.as_ref().and_then(|key| self.resolved.get(key));
            let preview = if state == Some("uploading") {
                upload
                    .and_then(|upload| upload.get("preview_url"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|url| safe_preview(url))
            } else {
                None
            };
            if self.previews.get(&uid).map(String::as_str) != preview {
                if let Some(previous) = self.previews.remove(&uid) {
                    let _ = web_sys::Url::revoke_object_url(&previous);
                }
                if let Some(preview) = preview {
                    self.previews.insert(uid.clone(), preview.into());
                }
            }
            let source = preview
                .or(direct)
                .or_else(|| resolved.and_then(|value| value.as_deref()));
            if let Some(source) = source {
                if image.get_attribute("src").as_deref() != Some(source) {
                    let _ = image.set_attribute("src", source);
                }
                let _ = image.remove_attribute("hidden");
            } else {
                let _ = image.remove_attribute("src");
                let _ = image.set_attribute("hidden", "");
            }
            let _ = image.set_attribute("draggable", "false");
            if let Ok(Some(status)) = figure.query_selector(".dx-doc-image-upload") {
                status.set_text_content(None);
                let text = match state {
                    Some("uploading") => upload
                        .and_then(|upload| upload.get("name"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Uploading image"),
                    Some("failed") => upload
                        .and_then(|upload| upload.get("error"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Upload failed"),
                    Some("reading") => "Reading file…",
                    _ if media.is_none() => "Upload interrupted",
                    _ if figure.has_attribute("data-image-error")
                        || resolved.is_some_and(Option::is_none) =>
                    {
                        "Image unavailable"
                    }
                    _ if source.is_none() => "Loading image…",
                    _ => "",
                };
                status.set_text_content(Some(text));
                if state == Some("uploading") {
                    if let Some(document) = status.owner_document() {
                        if let Some(progress) = document
                            .create_element("progress")
                            .ok()
                            .and_then(|element| element.dyn_into::<HtmlProgressElement>().ok())
                        {
                            progress.set_max(1.);
                            progress.set_value(
                                upload
                                    .and_then(|upload| upload.get("progress"))
                                    .and_then(serde_json::Value::as_f64)
                                    .unwrap_or(0.)
                                    .clamp(0., 1.),
                            );
                            let _ = progress.set_attribute("aria-label", "Upload progress");
                            let _ = status.append_child(&progress);
                        }
                    }
                }
                if text.is_empty() {
                    let _ = status.set_attribute("hidden", "");
                } else {
                    let _ = status.remove_attribute("hidden");
                }
            }
            if let (Some(media), Some(key)) = (media, key) {
                if direct.is_none()
                    && !self.resolved.contains_key(&key)
                    && !self.pending.values().any(|(pending, _)| pending == &key)
                {
                    self.next = self.next.wrapping_add(1);
                    self.pending.insert(self.next, (key, now));
                    output.push(EngineOutput::Request(crate::EngineEvent::MediaUrlQuery {
                        request_id: self.next,
                        media: media.clone(),
                    }));
                }
            }
        }
        output
    }
    pub fn result(&mut self, id: u64, url: Option<String>) {
        if let Some((key, _)) = self.pending.remove(&id) {
            self.resolved
                .insert(key, url.filter(|url| crate::is_safe_media_url(url)));
        }
    }
}
fn safe_preview(url: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .is_some_and(|origin| url.starts_with(&format!("blob:{origin}/")))
}

/// A response may never arrive. A weak timer causes pending media to enter its
/// terminal unavailable state after 10 seconds, without retaining the editor.
pub(crate) fn schedule_timeout(host: &super::Host) {
    let weak = Rc::downgrade(host);
    wasm_bindgen_futures::spawn_local(async move {
        futures_timer::Delay::new(std::time::Duration::from_millis(10_010)).await;
        let Some(host) = weak.upgrade() else {
            return;
        };
        let dirty = {
            let Ok(current) = host.try_borrow() else {
                return;
            };
            if current.media.pending.is_empty() {
                return;
            }
            let engine = current.engine.borrow();
            model::block_order(engine.document())
                .into_iter()
                .filter(|uid| {
                    engine
                        .content(uid)
                        .is_some_and(|node| node.kind.0 == "image")
                })
                .collect()
        };
        super::DomHost::publish(
            &host,
            vec![EngineOutput::Render(RenderPlan {
                dirty,
                structure_changed: false,
                full: false,
            })],
        );
    });
}

/// Image load events do not bubble, so use capture on the root. Publishing the
/// dirty figure draws its status and drains the observer's own mutation records.
pub(crate) fn install(host: &super::Host) -> Result<(), wasm_bindgen::JsValue> {
    let root = host.borrow().root.clone();
    for (name, failed) in [("load", false), ("error", true)] {
        super::DomHost::listen(host, root.as_ref(), name, true, move |host, event| {
            let Some(image) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .filter(|element| element.tag_name() == "IMG")
            else {
                return;
            };
            let Some(figure) = image.closest("figure.dx-doc-image").ok().flatten() else {
                return;
            };
            if failed {
                let _ = figure.set_attribute("data-image-error", "");
            } else {
                let _ = figure.remove_attribute("data-image-error");
            }
            if let Some(uid) = figure
                .closest(".dx-block[data-uid]")
                .ok()
                .flatten()
                .and_then(|block| block.get_attribute("data-uid"))
            {
                super::DomHost::publish(
                    host,
                    vec![EngineOutput::Render(RenderPlan {
                        dirty: vec![uid],
                        structure_changed: false,
                        full: false,
                    })],
                );
            }
        })?;
    }
    Ok(())
}

impl Drop for Media {
    fn drop(&mut self) {
        for url in self.previews.values() {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}
