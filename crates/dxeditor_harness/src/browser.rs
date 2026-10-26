use dioxus::prelude::*;
use dxeditor::{
    ComponentDocumentV2, ComponentNode, EditorCatalog, EntityLinkCandidate, EntityLinkExtension,
    EntityLinkPreview, EntityLinkProvider, IslandRegistry, SuggestionItem, SuggestionProvider,
    SuggestionQueryContext, SuggestionRegistry,
    engine::{identity::new_uid, model},
    profiles::composer::*,
};
use futures::future::LocalBoxFuture;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

fn content(kind: &str, text: &str) -> ComponentNode {
    let mut node = ComponentNode::new(kind);
    if !text.is_empty() {
        node.content.push(ComponentNode::text(text));
    }
    match kind {
        "heading" => {
            node.attrs.insert("level".into(), json!(2));
        }
        "todo" => {
            node.attrs.insert("checked".into(), json!(false));
        }
        "entity_embed" => {
            node.attrs.insert("entity".into(), json!("entity-1"));
        }
        _ => {}
    }
    model::block_node(node)
}
fn table() -> ComponentNode {
    let mut node = ComponentNode::new("table");
    node.attrs = serde_json::from_value(json!({"header_row":true,"header_column":false})).unwrap();
    node.content = (0..2)
        .map(|row| {
            ComponentNode::container(
                "table_row",
                Some(dxeditor::NodeId::new(new_uid())),
                (0..2)
                    .map(|col| {
                        ComponentNode::container(
                            if row == 0 {
                                "table_header_cell"
                            } else {
                                "table_cell"
                            },
                            None,
                            vec![ComponentNode::text(format!("{row},{col}"))],
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    model::block_node(node)
}
fn image() -> ComponentNode {
    let mut node = ComponentNode::new("image");
    node.attrs = serde_json::from_value(
        json!({"media":{"type":"url","url":"https://example.com/image.png"},"alt":"Sample image"}),
    )
    .unwrap();
    model::block_node(node)
}
fn document(route: &str, fixture: &str, count: usize) -> ComponentDocumentV2 {
    let blocks = if route == "/perf" {
        (0..count)
            .map(|index| {
                if index < 100 && index % 2 == 1 {
                    content("entity_embed", "")
                } else {
                    content("paragraph", &format!("Block {index}"))
                }
            })
            .collect()
    } else if route == "/islands" {
        vec![content("graph", ""), content("paragraph", "After island")]
    } else {
        match fixture {
            "table" => vec![table(), content("paragraph", "After table")],
            "image" => vec![image(), content("paragraph", "After image")],
            "quote" => vec![content("quote", "quote")],
            "todo" => vec![content("todo", "Task")],
            "task-children" => {
                let mut block = content("todo", "Task with children");
                block.content.push(ComponentNode::container(
                    "block_children",
                    None,
                    vec![content("paragraph", "Promoted child")],
                ));
                vec![block]
            }
            "toggle" => {
                let mut block = content("toggle", "Summary tail");
                block.content.push(ComponentNode::container(
                    "block_children",
                    None,
                    vec![content("paragraph", "Nested child")],
                ));
                vec![block, content("paragraph", "After toggle")]
            }
            "all" => {
                let mut blocks: Vec<_> = [
                    "paragraph",
                    "heading",
                    "bulleted_list_item",
                    "numbered_list_item",
                    "todo",
                    "toggle",
                    "quote",
                    "callout",
                    "code_block",
                ]
                .into_iter()
                .map(|kind| content(kind, kind))
                .collect();
                blocks.extend([
                    table(),
                    image(),
                    content("entity_embed", ""),
                    content("paragraph", "Last"),
                ]);
                blocks
            }
            "mention" | "deleted-mention" | "no-provider" => {
                let mut block = content("paragraph", "Hello ");
                let mut mention = ComponentNode::new("mention");
                mention.attrs =
                    serde_json::from_value(json!({"entity_id":if fixture == "deleted-mention" {"deleted"} else {"entity-1"},"label":"Ada"})).unwrap();
                block.content[0].content.push(mention);
                vec![block]
            }
            _ => vec![content("paragraph", "")],
        }
    };
    model::document(blocks)
}
struct Entities {
    on_open: EventHandler<String>,
}
impl EntityLinkProvider for Entities {
    fn search(&self, query: String) -> LocalBoxFuture<'static, Vec<EntityLinkCandidate>> {
        Box::pin(async move {
            ["Ada", "Alan", "Grace"]
                .into_iter()
                .filter(|label| label.to_lowercase().contains(&query.to_lowercase()))
                .map(|label| EntityLinkCandidate {
                    id: format!("entity-{}", if label == "Ada" { "1" } else { label }),
                    label: label.into(),
                    detail: Some("Harness entity".into()),
                })
                .collect()
        })
    }
    fn open(&self, entity_id: String) {
        self.on_open.call(entity_id);
    }
    fn preview(&self, entity_id: String) -> LocalBoxFuture<'static, Option<EntityLinkPreview>> {
        Box::pin(async move {
            if entity_id == "deleted" {
                None
            } else {
                Some(EntityLinkPreview {
                    id: entity_id,
                    label: "Ada".into(),
                    detail: Some("Harness preview".into()),
                    fields: vec![],
                })
            }
        })
    }
}
struct Commands;
impl SuggestionProvider for Commands {
    fn trigger(&self) -> char {
        '/'
    }
    fn query(&self, ctx: SuggestionQueryContext) -> LocalBoxFuture<'static, Vec<SuggestionItem>> {
        Box::pin(async move {
            [
            ("chip", "Attach Ada", json!({"chip":{"kind":"entity","label":"Ada","payload":{"id":"entity-1"}},"group":"People"})),
            ("help", "Help", json!({"text":"/help ","at_start":true,"group":"Commands"})),
            ("disabled", "Disabled hint", json!({"disabled":true,"group":"Commands"})),
        ].into_iter().filter(|(_,label,_)| label.to_lowercase().contains(&ctx.query.to_lowercase())).map(|(id,label,data)| SuggestionItem {id:id.into(),label:label.into(),description:None,icon:None,data,priority:0}).collect()
        })
    }
}

#[component]
pub fn App() -> Element {
    let (route, fixture, count) = use_hook(|| {
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            if let Some(root) = document.document_element() {
                let _ = root.set_attribute("lang", "en");
            }
        }

        let location = web_sys::window().unwrap().location();
        let params =
            web_sys::UrlSearchParams::new_with_str(&location.search().unwrap_or_default()).unwrap();
        (
            location.pathname().unwrap_or_else(|_| "/blocks".into()),
            params.get("fixture").unwrap_or_default(),
            params
                .get("blocks")
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(2000)
                .min(20_000),
        )
    });
    let mut current = use_signal(|| document(&route, &fixture, count));
    let mut handle = use_signal(|| None::<dxeditor::ComponentEditorHandle>);
    let mut composer_handle = use_signal(|| None::<ComposerHandle>);
    let started = use_hook(|| web_sys::window().unwrap().performance().unwrap().now());
    let mut mount_ms = use_signal(|| 0f64);
    let mut revision = use_signal(|| 0u64);
    let mut changes = use_signal(|| 0u64);
    let mut read_only = use_signal(|| false);
    let mut events = use_signal(Vec::<Value>::new);
    let mut notes = use_signal(|| {
        "# Notes\n\nA **bold** paragraph.\n\n- [x] done\n  - child\n\n```rust edition=2024\nfn main() {}\n```\n\n<section>opaque</section>".to_string()
    });
    let mut composer = use_signal(ComposerValue::default);
    let last = use_hook(|| Rc::new(RefCell::new(None::<ComponentDocumentV2>)));
    let last_snapshot = last.clone();
    let mut snapshot = use_signal(|| Value::Null);
    let route_for_hook = route.clone();
    use_effect(move || {
        let state = json!({"route":route_for_hook,"document":if route_for_hook == "/perf" {Value::Null} else {serde_json::to_value(&*current.read()).unwrap()},"revision":revision(),"changes":changes(),"events":events(),"notes":notes(),"composer":composer(),"ready":handle.read().is_some() || composer_handle.read().is_some(),"mountMs":mount_ms()});
        snapshot.set(state.clone());
        let _ = document::eval(&format!("window.__dxeditorHarness = {state};"));
    });
    let mut islands = IslandRegistry::default();
    islands.register("entity_embed", Rc::new(|context| rsx! { Island {context} }));
    islands.register("graph", Rc::new(|context| rsx! { Island {context} }));
    let catalog = if route == "/graph-node" {
        EditorCatalog::graph_node()
    } else {
        EditorCatalog::block_document()
    };
    let entity_links = if fixture == "no-provider" {
        None
    } else {
        Some(EntityLinkExtension::new(Rc::new(Entities {
            on_open: EventHandler::new(move |entity_id| {
                events.write().push(json!({"opened":entity_id}))
            }),
        })))
    };
    let mut suggestions = SuggestionRegistry::default();
    suggestions.register(Rc::new(Commands));
    let is_perf = route == "/perf";
    let snapshot_text = snapshot().to_string();
    rsx! {
        style { "body {{ margin:0; padding:24px; font:16px system-ui; color:#17202a; background:white; }} main {{ max-width:1000px; margin:auto; }} nav, .harness-controls {{ display:flex; flex-wrap:wrap; gap:8px; margin-block:16px; }} button {{ min-height:32px; }} .dxeditor__textarea {{ width:100%; min-height:240px; }} [data-testid=snapshot] {{ display:none; }}" }
        main {
            h1 { "Rust editor harness" }
            nav { aria_label: "Profiles", for path in ["blocks", "graph-node", "notes", "composer", "islands", "perf"] { a { href:"/{path}", "{path}" } } }
            div { class:"harness-controls", aria_label:"Harness controls",
                button { onclick:move |_| if let Some(handle)=handle.read().clone(){handle.undo()}, "Undo" }
                button { onclick:move |_| if let Some(handle)=handle.read().clone(){handle.redo()}, "Redo" }
                button { onclick:move |_| read_only.set(!read_only()), "Toggle read only" }
                button { onclick:move |_| revision += 1, "Acknowledge" }
                button { onclick:move |_| if let Some(handle)=handle.read().clone(){handle.replace_document(model::document(vec![content("paragraph","Replaced")]),dxeditor::HistoryPolicy::Reset);}, "Reset document" }
                button { onclick:move |_| if let Some(handle)=handle.read().clone(){handle.replace_document(model::document(vec![content("paragraph","Preserved")]),dxeditor::HistoryPolicy::Preserve);}, "Preserve document" }
                button { onclick:move |_| if let Some(handle)=handle.read().clone(){handle.insert_text("host text".into());}, "Insert host text" }
                button { onclick:move |_| {
                    let request = events.peek().iter().rev().find(|event| event["kind"] == "entityCreateRequest").and_then(|event| event["request_id"].as_u64());
                    if let (Some(handle), Some(request_id)) = (handle.read().clone(), request) { handle.entity_create_result(request_id, Some(EntityLinkCandidate {id:"created-1".into(),label:"Created entity".into(),detail:None})); }
                }, "Complete entity creation" }
                button { onclick:move |_| {
                    let request = events.peek().iter().rev().find(|event| event["kind"] == "entityPickerRequest").and_then(|event| event["request_id"].as_u64());
                    if let (Some(handle), Some(request_id)) = (handle.read().clone(), request) { handle.entity_picker_result(request_id, Some(EntityLinkCandidate {id:"picked-1".into(),label:"Picked entity".into(),detail:None})); }
                }, "Complete entity picker" }

                button { onclick:move |_| if let Some(handle)=composer_handle.read().clone(){handle.insert_chip(ComposerChip {kind:"entity".into(),label:"Ada".into(),payload:json!({"id":"entity-1"})});}, "Insert chip" }
                button { onclick:move |_| {
                    let handle = handle.read().clone(); let last = last_snapshot.clone();
                    spawn(async move { if let Some(handle)=handle { if let Some(document)=handle.snapshot_document().await { *last.borrow_mut()=Some(document); } } });
                }, "Snapshot" }
            }
            section { aria_label:"Editor under test",
                if route == "/notes" {
                    dxeditor::MarkdownEditor {value:notes(),readonly:read_only(),entity_links:entity_links.clone(),on_change:move |value| {notes.set(value);changes += 1;}}
                } else if route == "/composer" {
                    ComposerEditor {value:composer(),on_change:move |value|{composer.set(value);changes += 1;},triggers:vec!['/'],suggestions,on_ready:move |value|composer_handle.set(Some(value)),on_event:move |event|events.write().push(json!(format!("{event:?}"))),placeholder:"Message",disabled:read_only()}
                } else {
                    dxeditor::ComponentEditor {
                        document:current(),catalog,external_revision:Some(revision()),read_only:read_only(),aria_label:"Document editor",islands,entity_links,host_entity_picker:fixture=="host-picker",
                        on_ready:move |value|{mount_ms.set(web_sys::window().unwrap().performance().unwrap().now()-started);handle.set(Some(value));},
                        on_change:move |change:dxeditor::ComponentChange| {
                            if is_perf {
                                *last.borrow_mut()=Some(change.document);
                            } else {
                                changes += 1;
                                current.set(change.document);
                            }
                        },
                        on_event:move |event:dxeditor::EngineEvent| events.write().push(serde_json::to_value(event).unwrap()),
                        on_invalid_change:move |problem|events.write().push(json!({"invalid":problem})),
                    }
                }
            }
            pre { "data-testid":"snapshot", "{snapshot_text}" }
        }
    }
}

#[derive(Props, Clone)]
struct IslandProps {
    context: dxeditor::IslandContext,
}
impl PartialEq for IslandProps {
    fn eq(&self, other: &Self) -> bool {
        self.context.uid == other.context.uid
            && self.context.attrs == other.context.attrs
            && self.context.read_only == other.context.read_only
    }
}
#[component]
fn Island(props: IslandProps) -> Element {
    let context = props.context;
    let mut dialog = use_signal(|| false);
    let mut value = use_signal(|| "island input".to_string());
    let title = context
        .attrs
        .read()
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Island")
        .to_string();
    let update = context.clone();
    rsx! {
        div { "data-testid":"island", h2 {"{title}"}
            input { aria_label:"Island input",value:value(),oninput:move |event|value.set(event.value()) }
            button {onclick:move |_| {let mut attrs=update.attrs.peek().clone();attrs.insert("title".into(),json!("Updated island"));update.update_attrs(attrs);},"Update island"}
            button {onclick:move |_| dialog.set(true),"Open island dialog"}
            if dialog() {dialog {open:true,aria_label:"Island dialog",button {onclick:move |_|dialog.set(false),"Close dialog"}}}
        }
    }
}
