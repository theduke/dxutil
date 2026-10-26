#![cfg(all(feature = "web", target_arch = "wasm32"))]
use dxeditor::dom::{DomHost, Host, geometry, mapping, selection_sync};
use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineInput, EngineOptions, EngineOutput,
    InlineContainerRef, Profile, TextPoint, TextRange,
    commands::{Axis, TableCommand},
    model,
};
use dxeditor::{ComponentMark, ComponentNode, EditorCatalog};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_test::*;
use web_sys::{
    ClipboardEvent, ClipboardEventInit, DataTransfer, Element, Event, InputEvent, InputEventInit,
};
wasm_bindgen_test_configure!(run_in_browser);

struct Fixture {
    host: Host,
    wrapper: Element,
    changes: Rc<RefCell<Vec<EngineOutput>>>,
}
impl Fixture {
    fn new(blocks: Vec<ComponentNode>) -> Self {
        Self::profile(
            blocks,
            EditorCatalog::block_document(),
            Profile::BlockDocument,
        )
    }
    fn profile(blocks: Vec<ComponentNode>, catalog: EditorCatalog, profile: Profile) -> Self {
        Self::profile_with_drop(blocks, catalog, profile, false)
    }
    fn profile_with_drop(
        blocks: Vec<ComponentNode>,
        catalog: EditorCatalog,
        profile: Profile,
        native_file_drop: bool,
    ) -> Self {
        let document = web_sys::window().unwrap().document().unwrap();
        let wrapper = document.create_element("div").unwrap();
        wrapper.set_class_name("dxeditor");
        wrapper
            .set_attribute("data-dxeditor-session", "wasm-test")
            .unwrap();
        document.body().unwrap().append_child(&wrapper).unwrap();
        let element = document.create_element("div").unwrap();
        wrapper.append_child(&element).unwrap();
        if native_file_drop {
            element
                .set_attribute("data-native-file-drop", "true")
                .unwrap();
        }
        let engine = Rc::new(RefCell::new(
            EditorEngine::new(
                model::document(blocks),
                &catalog,
                profile,
                EngineOptions::default(),
            )
            .unwrap(),
        ));
        let changes = Rc::new(RefCell::new(Vec::new()));
        let output = changes.clone();
        let host = DomHost::mount(
            element,
            engine,
            Rc::new(move |items| output.borrow_mut().extend(items)),
        )
        .unwrap();
        Self {
            host,
            wrapper,
            changes,
        }
    }
    fn root(&self) -> Element {
        self.host.borrow().root.clone()
    }
    fn engine(&self) -> Rc<RefCell<EditorEngine>> {
        self.host.borrow().engine.clone()
    }
    fn first_uid(&self) -> String {
        model::children(self.engine().borrow().document(), None).unwrap()[0]
            .id
            .as_ref()
            .unwrap()
            .0
            .clone()
    }
    fn set_caret(&self, container: InlineContainerRef, offset: usize) {
        DomHost::run(
            &self.host,
            Command::SetSelection(EditorSelection::caret(container, offset)),
        );
    }
    fn text(&self) -> String {
        dxeditor::engine::clipboard::blocks_to_plain_text(
            model::children(self.engine().borrow().document(), None).unwrap(),
        )
    }
    fn change_count(&self) -> usize {
        self.changes
            .borrow()
            .iter()
            .filter(|output| matches!(output, EngineOutput::Changed { .. }))
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.host.borrow_mut().destroy();
        self.wrapper.remove();
    }
}
fn paragraph(text: &str) -> ComponentNode {
    model::block_node(ComponentNode::container(
        "paragraph",
        None,
        if text.is_empty() {
            Vec::new()
        } else {
            vec![ComponentNode::text(text)]
        },
    ))
}
fn input(root: &Element, kind: &str, text: Option<&str>, cancelable: bool) -> InputEvent {
    let options = InputEventInit::new();
    options.set_bubbles(true);
    options.set_cancelable(cancelable);
    options.set_input_type(kind);
    options.set_data(text);
    let event = InputEvent::new_with_event_init_dict("beforeinput", &options).unwrap();
    root.dispatch_event(&event).unwrap();
    event
}
fn clipboard(root: &Element, kind: &str, data: &DataTransfer) -> ClipboardEvent {
    let options = ClipboardEventInit::new();
    options.set_bubbles(true);
    options.set_cancelable(true);
    options.set_clipboard_data(Some(data));
    let event = ClipboardEvent::new_with_event_init_dict(kind, &options).unwrap();
    root.dispatch_event(&event).unwrap();
    event
}
async fn tick() {
    futures_timer::Delay::new(std::time::Duration::from_millis(10)).await;
}

#[wasm_bindgen_test]
fn every_block_kind_renders_semantic_dom() {
    let mut blocks = Vec::new();
    for kind in [
        "paragraph",
        "heading",
        "bulleted_list_item",
        "numbered_list_item",
        "todo",
        "toggle",
        "quote",
        "callout",
        "code_block",
        "divider",
        "image",
        "file",
        "bookmark",
        "entity_embed",
        "graph",
        "unsupported",
        "details_break",
    ] {
        let mut node = ComponentNode::new(kind);
        if kind == "heading" {
            node.attrs.insert("level".into(), json!(3));
        }
        if kind == "todo" {
            node.attrs.insert("checked".into(), json!(true));
        }
        if kind == "bookmark" {
            node.attrs
                .insert("url".into(), json!("https://example.com"));
        }
        if kind == "entity_embed" {
            node.attrs.insert("entity".into(), json!("one"));
        }
        if kind == "unsupported" {
            node.attrs.insert("raw".into(), json!({"kind":"unknown"}));
        }
        blocks.push(model::block_node(node));
    }
    let fixture = Fixture::new(blocks);
    let root = fixture.root();
    assert_eq!(root.get_attribute("role").as_deref(), Some("textbox"));
    assert_eq!(
        root.get_attribute("aria-multiline").as_deref(),
        Some("true")
    );
    assert!(root.query_selector("h3").unwrap().is_some());
    assert!(
        root.query_selector("input[type=checkbox]:checked")
            .unwrap()
            .is_some()
    );
    assert!(
        root.query_selector("button[aria-expanded]")
            .unwrap()
            .is_some()
    );
    assert!(root.query_selector("blockquote").unwrap().is_some());
    assert!(root.query_selector("pre > code").unwrap().is_some());
    assert!(
        root.query_selector("figure > figcaption")
            .unwrap()
            .is_some()
    );
    assert!(
        root.query_selector(".dxeditor-island[contenteditable=false]")
            .unwrap()
            .is_some()
    );
    assert!(root.query_selector("p > br").unwrap().is_some());
}

#[wasm_bindgen_test]
fn utf16_mapping_surrogates_marks_and_inline_atoms_round_trip() {
    let mut marked = ComponentNode::text("a🌍e\u{301}");
    marked.marks.push(ComponentMark::new("bold"));
    let mut mention = ComponentNode::new("mention");
    mention.attrs.insert("entity_id".into(), json!("one"));
    mention.attrs.insert("label".into(), json!("One"));
    let fixture = Fixture::new(vec![model::block_node(ComponentNode::container(
        "paragraph",
        None,
        vec![
            marked,
            mention,
            ComponentNode::text("👨\u{200d}👩\u{200d}👧"),
        ],
    ))]);
    let container = InlineContainerRef::Block(fixture.first_uid());
    for offset in 0..=10 {
        let point = TextPoint::new(container.clone(), offset);
        let (node, dom_offset) = mapping::to_dom(&fixture.root(), &point).unwrap();
        assert_eq!(
            mapping::from_dom(&fixture.root(), &node, dom_offset),
            Some(point)
        );
    }
    assert_eq!(mapping::units_to_utf16("a🌍b", 2), 3);
    assert_eq!(mapping::utf16_to_units("a🌍b", 2), 1);
    assert_eq!(mapping::utf16_to_units("a🌍b", 3), 2);
}

#[wasm_bindgen_test]
fn zero_width_anchors_do_not_change_model_positions() {
    let chip =
        dxeditor::profiles::composer::chip_node(&dxeditor::profiles::composer::ComposerChip {
            kind: "entity".into(),
            label: "A long label".into(),
            payload: json!("one"),
        });
    let fixture = Fixture::profile(
        vec![model::block_node(ComponentNode::container(
            "paragraph",
            None,
            vec![chip.clone(), chip],
        ))],
        EditorCatalog::composer(),
        Profile::Composer,
    );
    let container = InlineContainerRef::Block(fixture.first_uid());
    for offset in 0..=2 {
        let point = TextPoint::new(container.clone(), offset);
        let (node, dom_offset) = mapping::to_dom(&fixture.root(), &point).unwrap();
        assert_eq!(
            mapping::from_dom(&fixture.root(), &node, dom_offset),
            Some(point)
        );
    }
    let paragraph = fixture.root().query_selector("p").unwrap().unwrap();
    assert_eq!(mapping::read_text(&paragraph), "\u{fffc}\u{fffc}");
}

#[wasm_bindgen_test]
fn selection_write_read_and_echo_do_not_emit_changes() {
    let fixture = Fixture::new(vec![paragraph("hello 🌍")]);
    let container = InlineContainerRef::Block(fixture.first_uid());
    let selection = EditorSelection::Text(TextRange {
        anchor: TextPoint::new(container.clone(), 2),
        head: TextPoint::new(container, 7),
    });
    DomHost::run(&fixture.host, Command::SetSelection(selection.clone()));
    assert_eq!(selection_sync::read(&fixture.root()), Some(selection));
    fixture
        .root()
        .owner_document()
        .unwrap()
        .dispatch_event(&Event::new("selectionchange").unwrap())
        .unwrap();
    assert_eq!(fixture.change_count(), 0);
    assert!(!fixture.engine().borrow().can_undo());
}

#[wasm_bindgen_test]
fn beforeinput_text_enter_delete_history_and_linebreak() {
    let fixture = Fixture::new(vec![paragraph("")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    assert!(input(&fixture.root(), "insertText", Some("hello🌍"), true).default_prevented());
    assert_eq!(fixture.text(), "hello🌍");
    input(&fixture.root(), "deleteContentBackward", None, true);
    assert_eq!(fixture.text(), "hello");
    input(&fixture.root(), "insertParagraph", None, true);
    input(&fixture.root(), "insertText", Some("next"), true);
    assert_eq!(fixture.text(), "hello\nnext");
    input(&fixture.root(), "insertLineBreak", None, true);
    assert_eq!(fixture.text(), "hello\nnext\n");
    input(&fixture.root(), "historyUndo", None, true);
    // Adjacent typing and hard-break input share the 500ms typing group.
    assert_eq!(fixture.text(), "hello\n");
    input(&fixture.root(), "historyRedo", None, true);
    assert_eq!(fixture.text(), "hello\nnext\n");
}

#[wasm_bindgen_test]
fn read_only_blocks_beforeinput_and_applies_live_aria() {
    let fixture = Fixture::new(vec![paragraph("unchanged")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 3);
    let output = fixture.engine().borrow_mut().set_read_only(true);
    DomHost::publish(&fixture.host, output);
    fixture.host.borrow_mut().set_aria_label("Read only editor");
    assert!(input(&fixture.root(), "insertText", Some("oops"), true).default_prevented());
    assert_eq!(fixture.text(), "unchanged");
    assert_eq!(
        fixture.root().get_attribute("contenteditable").as_deref(),
        Some("false")
    );
    assert_eq!(
        fixture.root().get_attribute("aria-label").as_deref(),
        Some("Read only editor")
    );
    assert_eq!(fixture.change_count(), 0);
}

#[wasm_bindgen_test]
async fn composition_reads_back_once_without_touching_active_dom() {
    let fixture = Fixture::new(vec![paragraph("a")]);
    let container = InlineContainerRef::Block(fixture.first_uid());
    fixture.set_caret(container.clone(), 1);
    let paragraph = fixture.root().query_selector("p").unwrap().unwrap();
    let original = paragraph.first_child().unwrap();
    fixture
        .root()
        .dispatch_event(&Event::new("compositionstart").unwrap())
        .unwrap();
    original.set_node_value(Some("a日本語"));
    let event = input(
        &fixture.root(),
        "insertCompositionText",
        Some("日本語"),
        true,
    );
    assert!(!event.default_prevented());
    assert_eq!(fixture.text(), "a");
    assert_eq!(paragraph.first_child(), Some(original.clone()));
    fixture
        .root()
        .dispatch_event(&Event::new("compositionend").unwrap())
        .unwrap();
    tick().await;
    assert_eq!(fixture.text(), "a日本語");
    assert_eq!(fixture.change_count(), 1);
    DomHost::run(&fixture.host, Command::Undo);
    assert_eq!(fixture.text(), "a");
}

#[wasm_bindgen_test]
async fn mutation_observer_recovers_text_and_reverts_foreign_structure() {
    let fixture = Fixture::new(vec![paragraph("old")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    let paragraph = fixture.root().query_selector("p").unwrap().unwrap();
    paragraph
        .first_child()
        .unwrap()
        .set_node_value(Some("corrected"));
    tick().await;
    assert_eq!(fixture.text(), "corrected");
    let script = paragraph
        .owner_document()
        .unwrap()
        .create_element("script")
        .unwrap();
    script.set_text_content(Some("/* unexpected inert structure */"));
    paragraph.append_child(&script).unwrap();
    tick().await;
    assert!(fixture.root().query_selector("script").unwrap().is_none());
    assert_eq!(fixture.text(), "corrected");
}

#[wasm_bindgen_test]
fn clipboard_copy_cut_html_and_tsv_paste() {
    let fixture = Fixture::new(vec![paragraph("copy me")]);
    let container = InlineContainerRef::Block(fixture.first_uid());
    DomHost::run(
        &fixture.host,
        Command::SetSelection(EditorSelection::Text(TextRange {
            anchor: TextPoint::new(container.clone(), 0),
            head: TextPoint::new(container, 4),
        })),
    );
    let data = DataTransfer::new().unwrap();
    assert!(clipboard(&fixture.root(), "copy", &data).default_prevented());
    assert_eq!(data.get_data("text/plain").unwrap(), "copy");
    assert!(
        data.get_data("text/html")
            .unwrap()
            .contains("data-semantic-slice")
    );
    assert!(
        !data
            .get_data(dxeditor::engine::clipboard::INTERNAL_CLIPBOARD_MIME)
            .unwrap()
            .is_empty()
    );
    clipboard(&fixture.root(), "cut", &data);
    assert_eq!(fixture.text(), " me");
    let html = DataTransfer::new().unwrap();
    html.set_data(
        "text/html",
        "<p><strong>bold</strong></p><script>bad</script>",
    )
    .unwrap();
    clipboard(&fixture.root(), "paste", &html);
    assert_eq!(fixture.text(), "bold me");
    assert!(fixture.root().query_selector("strong").unwrap().is_some());
    assert!(fixture.root().query_selector("script").unwrap().is_none());
}

#[wasm_bindgen_test]
fn tables_render_cells_selection_and_tsv_growth() {
    let fixture = Fixture::new(vec![paragraph("")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    DomHost::run(
        &fixture.host,
        Command::Table(TableCommand::Insert {
            rows: 2,
            columns: 2,
            header_row: true,
        }),
    );
    assert_eq!(fixture.root().query_selector_all("th").unwrap().length(), 2);
    let uid = model::block_order(fixture.engine().borrow().document())[0].clone();
    DomHost::run(
        &fixture.host,
        Command::SetSelection(EditorSelection::Cells {
            table: uid.clone(),
            anchor: (0, 0),
            head: (1, 1),
        }),
    );
    assert_eq!(
        fixture
            .root()
            .query_selector_all("td[data-selected],th[data-selected]")
            .unwrap()
            .length(),
        4
    );
    let data = DataTransfer::new().unwrap();
    data.set_data("text/plain", "A\tB\tC").unwrap();
    clipboard(&fixture.root(), "paste", &data);
    assert_eq!(fixture.root().query_selector_all("tr").unwrap().length(), 2);
    assert_eq!(fixture.root().query_selector_all("th").unwrap().length(), 3);
    DomHost::run(
        &fixture.host,
        Command::Table(TableCommand::ToggleHeader(Axis::Column)),
    );
    assert_eq!(fixture.root().query_selector_all("th").unwrap().length(), 4);
}

#[wasm_bindgen_test]
fn adopted_island_survives_attribute_updates_moves_and_undo() {
    let fixture = Fixture::new(vec![
        model::block_node(ComponentNode::new("graph")),
        paragraph("after"),
    ]);
    let uid = fixture.first_uid();
    let document = fixture.root().owner_document().unwrap();
    let parking = document.create_element("div").unwrap();
    parking.set_id("dxeditor-islands-wasm-test");
    fixture.wrapper.append_child(&parking).unwrap();
    let island = document.create_element("div").unwrap();
    island.set_class_name("dxeditor-island-root");
    island.set_id(&format!("dxeditor-island-wasm-test-{uid}"));
    let input = document.create_element("input").unwrap();
    island.append_child(&input).unwrap();
    parking.append_child(&island).unwrap();
    fixture.host.borrow_mut().island_ready(&uid);
    let adopted_parent = island.parent_element().unwrap();
    assert!(fixture.root().contains(Some(&island)));
    DomHost::run(
        &fixture.host,
        Command::UpdateNodeAttrs {
            uid: uid.clone(),
            attrs: serde_json::from_value(json!({"title":"Changed"})).unwrap(),
            add_to_history: true,
        },
    );
    assert_eq!(island.parent_element(), Some(adopted_parent));
    DomHost::run(&fixture.host, Command::RemoveNode(uid.clone()));
    assert_eq!(island.parent_element(), Some(parking));
    DomHost::run(&fixture.host, Command::Undo);
    assert!(fixture.root().contains(Some(&island)));
    assert_eq!(island.first_element_child(), Some(input));
}

#[wasm_bindgen_test]
fn geometry_accounts_for_scaled_wrapper_and_focus() {
    let fixture = Fixture::new(vec![paragraph("geometry")]);
    fixture
        .wrapper
        .set_attribute(
            "style",
            "width:400px;min-height:100px;transform:scale(.5);transform-origin:top left",
        )
        .unwrap();
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 3);
    DomHost::run(&fixture.host, Command::Focus);
    assert_eq!(
        fixture.root().owner_document().unwrap().active_element(),
        Some(fixture.root())
    );
    let geometry = geometry::measure(&fixture.root(), &fixture.engine().borrow());
    assert!((geometry.scale_x - 0.5).abs() < 0.01);
    assert!((geometry.scale_y - 0.5).abs() < 0.01);
    assert!(geometry.caret.is_some());
}

#[wasm_bindgen_test]
fn destroyed_host_removes_listeners_and_ignores_stale_dispatches() {
    let fixture = Fixture::new(vec![paragraph("old")]);
    let root = fixture.root();
    fixture.host.borrow_mut().destroy();
    input(&root, "insertText", Some("new"), true);
    DomHost::dispatch(
        &fixture.host,
        EngineInput::InsertText { text: "new".into() },
    );
    assert_eq!(fixture.text(), "old");
    assert_eq!(fixture.change_count(), 0);
}

#[wasm_bindgen_test]
fn composer_backspace_selects_chip_before_removing_and_emits_once() {
    use dxeditor::profiles::composer::{
        ComposerChip, ComposerEvent, ComposerSegment, ComposerValue, composer_to_blocks,
    };
    let value = ComposerValue {
        segments: vec![
            ComposerSegment::Text("hi ".into()),
            ComposerSegment::Chip(ComposerChip {
                kind: "entity".into(),
                label: "Long label".into(),
                payload: json!({"id":"one"}),
            }),
        ],
    };
    let blocks = model::children(&composer_to_blocks(&value), None)
        .unwrap()
        .clone();
    let fixture = Fixture::profile(blocks, EditorCatalog::composer(), Profile::Composer);
    let events = Rc::new(RefCell::new(Vec::new()));
    let capture = events.clone();
    fixture
        .host
        .borrow_mut()
        .set_composer_event_callback(Rc::new(move |event| capture.borrow_mut().push(event)));
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 4);
    input(&fixture.root(), "deleteContentBackward", None, true);
    assert_eq!(
        fixture
            .root()
            .query_selector_all("[data-composer-chip]")
            .unwrap()
            .length(),
        1
    );
    assert_eq!(fixture.change_count(), 0);
    assert!(!fixture.engine().borrow().selection().is_collapsed());
    input(&fixture.root(), "deleteContentBackward", None, true);
    assert_eq!(
        fixture
            .root()
            .query_selector_all("[data-composer-chip]")
            .unwrap()
            .length(),
        0
    );
    assert_eq!(fixture.change_count(), 1);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, ComposerEvent::ChipRemoved(_)))
            .count(),
        1
    );
}

#[wasm_bindgen_test]
async fn composition_in_list_caption_table_and_composer_containers() {
    for kind in ["bulleted_list_item", "image", "table", "composer"] {
        let fixture = if kind == "composer" {
            Fixture::profile(
                vec![paragraph("start")],
                EditorCatalog::composer(),
                Profile::Composer,
            )
        } else if kind == "table" {
            let fixture = Fixture::new(vec![paragraph("")]);
            DomHost::run(
                &fixture.host,
                Command::Table(TableCommand::Insert {
                    rows: 1,
                    columns: 1,
                    header_row: false,
                }),
            );
            fixture
        } else {
            Fixture::new(vec![model::block_node(ComponentNode::container(
                kind,
                None,
                vec![ComponentNode::text("start")],
            ))])
        };
        let uid = fixture.first_uid();
        let container = if kind == "image" {
            InlineContainerRef::Caption(uid)
        } else if kind == "table" {
            let engine = fixture.engine();
            let engine = engine.borrow();
            let row = model::content(engine.document(), &uid).unwrap().content[0]
                .id
                .as_ref()
                .unwrap()
                .0
                .clone();
            InlineContainerRef::Cell { row, col: 0 }
        } else {
            InlineContainerRef::Block(uid)
        };
        fixture.set_caret(container.clone(), if kind == "table" { 0 } else { 5 });
        let (node, _) =
            mapping::to_dom(&fixture.root(), &TextPoint::new(container.clone(), 0)).unwrap();
        let (element, _) = mapping::container(&node, &fixture.root()).unwrap();
        let initial_changes = fixture.change_count();
        fixture
            .root()
            .dispatch_event(&Event::new("compositionstart").unwrap())
            .unwrap();
        element.set_text_content(Some("入力"));
        fixture
            .root()
            .dispatch_event(&Event::new("compositionend").unwrap())
            .unwrap();
        tick().await;
        let engine = fixture.engine();
        let engine = engine.borrow();
        assert_eq!(
            dxeditor::engine::text::plain(model::inline(engine.document(), &container).unwrap()),
            "入力",
            "{kind}"
        );
        assert_eq!(fixture.change_count(), initial_changes + 1, "{kind}");
    }
}

#[wasm_bindgen_test]
fn collapsed_children_update_without_document_changes() {
    let mut toggle = model::block_node(ComponentNode::container(
        "toggle",
        None,
        vec![ComponentNode::text("parent")],
    ));
    let uid = toggle.id.as_ref().unwrap().0.clone();
    toggle.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![paragraph("child")],
    ));
    let fixture = Fixture::new(vec![toggle]);
    DomHost::run(&fixture.host, Command::ToggleCollapsed(uid.clone()));
    assert!(
        fixture
            .root()
            .query_selector("[data-children]")
            .unwrap()
            .unwrap()
            .has_attribute("hidden")
    );
    assert_eq!(fixture.change_count(), 0);
    DomHost::run(&fixture.host, Command::SetCollapsed(Vec::new()));
    assert!(
        !fixture
            .root()
            .query_selector("[data-children]")
            .unwrap()
            .unwrap()
            .has_attribute("hidden")
    );
}

#[wasm_bindgen_test]
fn composer_structured_paste_leaves_text_caret_after_inserted_value() {
    use dxeditor::profiles::composer::{
        ComposerChip, ComposerSegment, ComposerValue, blocks_to_composer,
    };
    let fixture = Fixture::profile(
        vec![paragraph("")],
        EditorCatalog::composer(),
        Profile::Composer,
    );
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    let value = ComposerValue {
        segments: vec![
            ComposerSegment::Chip(ComposerChip {
                kind: "entity".into(),
                label: "Person".into(),
                payload: json!("one"),
            }),
            ComposerSegment::Text("\nnext".into()),
        ],
    };
    let data = DataTransfer::new().unwrap();
    data.set_data(
        dxeditor::engine::clipboard::COMPOSER_CLIPBOARD_MIME,
        &dxeditor::engine::clipboard::encode_composer_clipboard(&value).unwrap(),
    )
    .unwrap();
    clipboard(&fixture.root(), "paste", &data);
    assert!(matches!(
        fixture.engine().borrow().selection(),
        EditorSelection::Text(_)
    ));
    input(&fixture.root(), "insertText", Some("!"), true);
    let mut expected = value;
    if let Some(ComposerSegment::Text(text)) = expected.segments.last_mut() {
        text.push('!');
    }
    assert_eq!(
        blocks_to_composer(fixture.engine().borrow().document()),
        expected
    );
}

fn drag(target: &Element, kind: &str, data: &DataTransfer, x: i32, y: i32) -> web_sys::DragEvent {
    let options = web_sys::DragEventInit::new();
    options.set_bubbles(true);
    options.set_cancelable(true);
    options.set_data_transfer(Some(data));
    options.set_client_x(x);
    options.set_client_y(y);
    let event = web_sys::DragEvent::new_with_event_init_dict(kind, &options).unwrap();
    target.dispatch_event(&event).unwrap();
    event
}
fn key(root: &Element, key: &str, ctrl: bool, shift: bool) {
    let options = web_sys::KeyboardEventInit::new();
    options.set_bubbles(true);
    options.set_cancelable(true);
    options.set_key(key);
    options.set_ctrl_key(ctrl);
    options.set_shift_key(shift);
    root.dispatch_event(
        &web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &options).unwrap(),
    )
    .unwrap();
}
fn file_data() -> DataTransfer {
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str("test bytes"));
    let options = web_sys::FilePropertyBag::new();
    options.set_type("image/png");
    let file =
        web_sys::File::new_with_str_sequence_and_options(&parts, "photo.png", &options).unwrap();
    let data = DataTransfer::new().unwrap();
    let _ = data.items().add_with_file(&file).unwrap();
    data
}

#[wasm_bindgen_test]
fn copied_cell_rectangle_keeps_tsv_shape_and_cut_clears_only_cells() {
    let fixture = Fixture::new(vec![paragraph("")]);
    DomHost::run(
        &fixture.host,
        Command::Table(TableCommand::Insert {
            rows: 3,
            columns: 3,
            header_row: true,
        }),
    );
    let uid = fixture.first_uid();
    DomHost::run(
        &fixture.host,
        Command::Table(TableCommand::PasteGrid(vec![
            vec!["h1".into(), "h2".into(), "h3".into()],
            vec!["a".into(), "b".into(), "c".into()],
            vec!["d".into(), "e".into(), "f".into()],
        ])),
    );
    DomHost::run(
        &fixture.host,
        Command::SetSelection(EditorSelection::Cells {
            table: uid.clone(),
            anchor: (1, 1),
            head: (2, 2),
        }),
    );
    let data = DataTransfer::new().unwrap();
    clipboard(&fixture.root(), "copy", &data);
    assert_eq!(data.get_data("text/plain").unwrap(), "b\tc\ne\tf");
    let fingerprint = fixture.engine().borrow().schema().fingerprint.clone();
    let slice = dxeditor::engine::clipboard::decode_internal_clipboard(
        &data
            .get_data(dxeditor::engine::clipboard::INTERNAL_CLIPBOARD_MIME)
            .unwrap(),
        &fingerprint,
    )
    .unwrap();
    assert_eq!(slice.content[0].content[0].attrs["header_row"], false);
    let before_cut = fixture.change_count();
    clipboard(&fixture.root(), "cut", &data);
    let engine = fixture.engine();
    let engine = engine.borrow();
    let table = model::content(engine.document(), &uid).unwrap();
    assert_eq!(table.content.len(), 3);
    assert_eq!(
        dxeditor::engine::text::plain(&table.content[1].content[0].content),
        "a"
    );
    assert!(table.content[1].content[1].content.is_empty());
    assert_eq!(fixture.change_count(), before_cut + 1);
}

#[wasm_bindgen_test]
fn native_file_drop_bubbles_without_inserting_placeholders() {
    let fixture = Fixture::profile_with_drop(
        vec![paragraph("original")],
        EditorCatalog::block_document(),
        Profile::BlockDocument,
        true,
    );
    let count = Rc::new(RefCell::new(0));
    let capture = count.clone();
    let listener = Closure::wrap(
        Box::new(move |_: Event| *capture.borrow_mut() += 1) as Box<dyn FnMut(Event)>
    );
    fixture
        .wrapper
        .add_event_listener_with_callback("drop", listener.as_ref().unchecked_ref())
        .unwrap();
    let event = drag(&fixture.root(), "drop", &file_data(), 1, 1);
    assert!(event.default_prevented());
    assert_eq!(*count.borrow(), 1);
    assert_eq!(fixture.text(), "original");
    assert_eq!(fixture.change_count(), 0);
    fixture
        .wrapper
        .remove_event_listener_with_callback("drop", listener.as_ref().unchecked_ref())
        .unwrap();
}

#[wasm_bindgen_test]
async fn pasted_file_placeholder_uid_matches_host_upload_and_preview_is_transient() {
    let fixture = Fixture::new(vec![paragraph("")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    clipboard(&fixture.root(), "paste", &file_data());
    for _ in 0..100 {
        if fixture.changes.borrow().iter().any(|output| {
            matches!(
                output,
                EngineOutput::Request(dxeditor::EngineEvent::FilesPasted { .. })
            )
        }) {
            break;
        }
        tick().await;
    }
    let uid = fixture.first_uid();
    let engine = fixture.engine();
    let engine = engine.borrow();
    assert_eq!(
        model::content(engine.document(), &uid).unwrap().kind.0,
        "image"
    );
    assert!(
        engine.transient(&uid).unwrap()["upload"]["preview_url"]
            .as_str()
            .unwrap()
            .starts_with("blob:")
    );
    assert!(
        !serde_json::to_string(engine.document())
            .unwrap()
            .contains("preview_url")
    );
    assert!(fixture.changes.borrow().iter().any(|output| matches!(output, EngineOutput::Request(dxeditor::EngineEvent::FilesPasted { items }) if items.len() == 1 && items[0].uid == uid && items[0].mime == "image/png")), "uid {uid}, events {:?}", fixture.changes.borrow());
    let progress = fixture
        .root()
        .query_selector("progress[aria-label='Upload progress']")
        .unwrap()
        .unwrap();
    assert_eq!(
        progress
            .dyn_ref::<web_sys::HtmlProgressElement>()
            .unwrap()
            .max(),
        1.
    );
}

#[wasm_bindgen_test]
fn image_upload_progress_errors_and_safe_resolution_are_view_only() {
    let mut node = ComponentNode::new("image");
    node.attrs
        .insert("media".into(), json!({"type":"storage","id":"one"}));
    let fixture = Fixture::new(vec![model::block_node(node)]);
    let uid = fixture.first_uid();
    let request = fixture
        .changes
        .borrow()
        .iter()
        .find_map(|output| {
            if let EngineOutput::Request(dxeditor::EngineEvent::MediaUrlQuery {
                request_id, ..
            }) = output
            {
                Some(*request_id)
            } else {
                None
            }
        })
        .unwrap();
    fixture
        .host
        .borrow_mut()
        .media_url_result(request, Some("javascript:unsafe".into()));
    DomHost::publish(&fixture.host, Vec::new());
    assert_eq!(
        fixture
            .root()
            .query_selector(".dx-doc-image-upload")
            .unwrap()
            .unwrap()
            .text_content()
            .as_deref(),
        Some("Image unavailable")
    );
    assert!(
        fixture
            .root()
            .query_selector("img")
            .unwrap()
            .unwrap()
            .get_attribute("src")
            .is_none()
    );
    DomHost::run(&fixture.host, Command::SetTransientAttrs { uid: uid.clone(), attrs: serde_json::from_value(json!({"upload":{"status":"uploading","name":"Photo upload","progress":0.5,"preview_url":"blob:https://evil.example/preview"}})).unwrap() });
    let progress = fixture.root().query_selector("progress").unwrap().unwrap();
    assert_eq!(
        progress
            .dyn_ref::<web_sys::HtmlProgressElement>()
            .unwrap()
            .value(),
        0.5
    );
    assert!(
        fixture
            .root()
            .query_selector("img")
            .unwrap()
            .unwrap()
            .get_attribute("src")
            .is_none()
    );
    DomHost::run(
        &fixture.host,
        Command::SetTransientAttrs {
            uid,
            attrs: serde_json::from_value(
                json!({"upload":{"status":"failed","error":"Storage unavailable"}}),
            )
            .unwrap(),
        },
    );
    assert_eq!(
        fixture
            .root()
            .query_selector(".dx-doc-image-upload")
            .unwrap()
            .unwrap()
            .text_content()
            .as_deref(),
        Some("Storage unavailable")
    );
    assert_eq!(fixture.change_count(), 0);
    assert!(!fixture.engine().borrow().can_undo());
}

#[wasm_bindgen_test]
fn table_row_drag_preserves_identity_and_undoes_once() {
    let fixture = Fixture::new(vec![paragraph("")]);
    DomHost::run(
        &fixture.host,
        Command::Table(TableCommand::Insert {
            rows: 3,
            columns: 1,
            header_row: false,
        }),
    );
    let uid = fixture.first_uid();
    let before = model::content(fixture.engine().borrow().document(), &uid)
        .unwrap()
        .content
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    let handle = fixture
        .wrapper
        .owner_document()
        .unwrap()
        .create_element("button")
        .unwrap();
    handle.set_attribute("data-table-row-drag", "0").unwrap();
    handle.set_attribute("data-table-uid", &uid).unwrap();
    fixture.wrapper.append_child(&handle).unwrap();
    let data = DataTransfer::new().unwrap();
    drag(&handle, "dragstart", &data, 0, 0);
    let last = fixture
        .root()
        .query_selector_all("tr")
        .unwrap()
        .item(2)
        .unwrap()
        .dyn_into::<Element>()
        .unwrap();
    let rect = last.get_bounding_client_rect();
    let baseline = fixture.change_count();
    drag(
        &last,
        "dragover",
        &data,
        rect.left() as i32 + 1,
        rect.bottom() as i32,
    );
    drag(
        &last,
        "drop",
        &data,
        rect.left() as i32 + 1,
        rect.bottom() as i32,
    );
    let after = model::content(fixture.engine().borrow().document(), &uid)
        .unwrap()
        .content
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        after,
        [before[1].clone(), before[2].clone(), before[0].clone()]
    );
    assert_eq!(fixture.change_count(), baseline + 1);
    DomHost::run(&fixture.host, Command::Undo);
    assert_eq!(
        model::content(fixture.engine().borrow().document(), &uid)
            .unwrap()
            .content
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
        before
    );
}

#[wasm_bindgen_test]
fn composer_suggestion_enter_and_plain_paste_override() {
    use dxeditor::profiles::composer::{ComposerChip, ComposerEvent};
    let fixture = Fixture::profile(
        vec![paragraph("")],
        EditorCatalog::composer(),
        Profile::Composer,
    );
    let engine = fixture.engine();
    engine.borrow_mut().set_suggestion_triggers(vec!['/']);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 0);
    input(&fixture.root(), "insertText", Some("/Attach"), true);
    let generation = engine.borrow().suggestion().generation;
    let item = dxeditor::SuggestionItem {
        id: "ada".into(),
        label: "Attach Ada".into(),
        description: None,
        icon: None,
        priority: 0,
        data: json!({"chip":ComposerChip { kind:"entity".into(), label:"Ada".into(), payload:json!("one") }}),
    };
    let output = engine
        .borrow_mut()
        .set_suggestion_items(generation, vec![item]);
    DomHost::publish(&fixture.host, output);
    key(&fixture.root(), "Enter", false, false);
    assert_eq!(
        fixture
            .root()
            .query_selector_all("[data-composer-chip]")
            .unwrap()
            .length(),
        1
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let capture = events.clone();
    fixture
        .host
        .borrow_mut()
        .set_composer_event_callback(Rc::new(move |event| capture.borrow_mut().push(event)));
    fixture.host.borrow_mut().set_max_inline_paste_chars(3);
    key(&fixture.root(), "V", true, true);
    let data = DataTransfer::new().unwrap();
    data.set_data("text/plain", "long plain text").unwrap();
    data.set_data(
        dxeditor::engine::clipboard::COMPOSER_CLIPBOARD_MIME,
        "{\"segments\":[{\"Chip\":{\"kind\":\"entity\",\"label\":\"Other\",\"payload\":null}}]}",
    )
    .unwrap();
    clipboard(&fixture.root(), "paste", &data);
    assert!(fixture.text().ends_with("long plain text"));
    assert_eq!(
        fixture
            .root()
            .query_selector_all("[data-composer-chip]")
            .unwrap()
            .length(),
        1
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, ComposerEvent::PastedText { .. }))
    );
}

#[wasm_bindgen_test]
fn repeated_typing_targets_toggle_text_and_image_caption() {
    for kind in ["toggle", "image"] {
        let fixture = Fixture::new(vec![model::block_node(ComponentNode::new(kind))]);
        let uid = fixture.first_uid();
        let container = if kind == "image" {
            InlineContainerRef::Caption(uid)
        } else {
            InlineContainerRef::Block(uid)
        };
        fixture.set_caret(container, 0);
        for text in ["a", "b", "c"] {
            input(&fixture.root(), "insertText", Some(text), true);
        }
        let selector = if kind == "image" {
            "figcaption"
        } else {
            ".dx-block-control-text"
        };
        assert_eq!(
            fixture
                .root()
                .query_selector(selector)
                .unwrap()
                .unwrap()
                .text_content()
                .as_deref(),
            Some("abc"),
            "{kind}"
        );
        if kind == "toggle" {
            assert_eq!(
                fixture
                    .root()
                    .query_selector("button[data-toggle]")
                    .unwrap()
                    .unwrap()
                    .text_content()
                    .as_deref(),
                Some("▾")
            );
        } else {
            assert_eq!(
                fixture
                    .root()
                    .query_selector(".dx-doc-image-upload")
                    .unwrap()
                    .unwrap()
                    .text_content()
                    .as_deref(),
                Some("Upload interrupted")
            );
        }
    }
}

#[wasm_bindgen_test]
fn parking_outer_island_keeps_nested_editor_islands_adopted_inside_it() {
    let fixture = Fixture::new(vec![model::block_node(ComponentNode::new("graph"))]);
    let uid = fixture.first_uid();
    let document = fixture.wrapper.owner_document().unwrap();
    let outer_parking = document.create_element("div").unwrap();
    outer_parking.set_id("dxeditor-islands-wasm-test");
    fixture.wrapper.append_child(&outer_parking).unwrap();
    let outer = document.create_element("div").unwrap();
    outer.set_class_name("dxeditor-island-root");
    outer.set_id(&format!("dxeditor-island-wasm-test-{uid}"));
    outer_parking.append_child(&outer).unwrap();
    let nested = document.create_element("div").unwrap();
    nested.set_class_name("dxeditor-island-root");
    nested.set_id("dxeditor-island-nested-session-uid");
    nested
        .set_attribute("data-island-parking", "dxeditor-islands-nested-session")
        .unwrap();
    outer.append_child(&nested).unwrap();
    let nested_parking = document.create_element("div").unwrap();
    nested_parking.set_id("dxeditor-islands-nested-session");
    outer.append_child(&nested_parking).unwrap();
    fixture.host.borrow_mut().island_ready(&uid);
    DomHost::run(&fixture.host, Command::RemoveNode(uid));
    assert_eq!(outer.parent_element(), Some(outer_parking));
    assert_eq!(nested.parent_element(), Some(outer));
}

#[wasm_bindgen_test]
fn focus_reconciles_after_listener_skips_a_borrowed_engine() {
    let fixture = Fixture::new(vec![paragraph("focus")]);
    let engine = fixture.engine();
    let borrow = engine.borrow();
    fixture
        .root()
        .dyn_ref::<web_sys::HtmlElement>()
        .unwrap()
        .focus()
        .unwrap();
    drop(borrow);
    DomHost::publish(&fixture.host, Vec::new());
    assert!(fixture.engine().borrow().ui_state().focused);
}

#[wasm_bindgen_test]
async fn island_dom_mutations_leave_editor_block_nodes_in_place() {
    let fixture = Fixture::new(vec![
        model::block_node(ComponentNode::new("graph")),
        paragraph("writing"),
    ]);
    let block = fixture.root().query_selector(".dx-block").unwrap().unwrap();
    let island = fixture
        .root()
        .query_selector(".dxeditor-island")
        .unwrap()
        .unwrap();
    island.set_text_content(Some("island rendered new content"));
    tick().await;
    assert_eq!(
        fixture.root().query_selector(".dx-block").unwrap(),
        Some(block)
    );
    assert_eq!(fixture.change_count(), 0);
}

#[wasm_bindgen_test]
fn block_drag_ghost_is_bounded_and_removed_on_drag_end() {
    let fixture = Fixture::new(vec![paragraph("Drag this block")]);
    let document = fixture.wrapper.owner_document().unwrap();
    let handle = document.create_element("button").unwrap();
    handle
        .set_attribute("data-dxeditor-drag", &fixture.first_uid())
        .unwrap();
    fixture.wrapper.append_child(&handle).unwrap();
    let data = DataTransfer::new().unwrap();
    drag(&handle, "dragstart", &data, 30, 30);
    let ghost = document
        .query_selector(".dx-block-drag-ghost")
        .unwrap()
        .unwrap();
    assert!(
        ghost
            .get_attribute("style")
            .unwrap()
            .contains("max-height:120px")
    );
    assert_eq!(ghost.get_attribute("aria-hidden").as_deref(), Some("true"));
    assert!(ghost.text_content().unwrap().contains("Drag this block"));
    drag(&handle, "dragend", &data, 30, 30);
    assert!(
        document
            .query_selector(".dx-block-drag-ghost")
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.change_count(), 0);
}

#[wasm_bindgen_test]
fn end_caret_stays_parent_anchored_through_typing() {
    let fixture = Fixture::new(vec![paragraph("hello")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 5);
    let selection = web_sys::window().unwrap().get_selection().unwrap().unwrap();
    let parent = selection.anchor_node().unwrap();
    assert_eq!(parent.node_type(), web_sys::Node::ELEMENT_NODE);
    let offset = selection.anchor_offset();
    input(&fixture.root(), "insertText", Some("🌍"), true);
    assert_eq!(selection.anchor_node().as_ref(), Some(&parent));
    assert_eq!(selection.anchor_offset(), offset);
    assert_eq!(fixture.text(), "hello🌍");
    assert_eq!(
        fixture
            .engine()
            .borrow()
            .selection()
            .text_range()
            .unwrap()
            .head
            .offset,
        6
    );
}

#[wasm_bindgen_test]
fn parent_anchored_caret_geometry_uses_text_position_without_moving_selection() {
    let fixture = Fixture::new(vec![paragraph("/image")]);
    fixture
        .wrapper
        .set_attribute("style", "margin:80px 40px;padding:30px;width:400px")
        .unwrap();
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 6);
    let document = fixture.root().owner_document().unwrap();
    let selection = document.get_selection().unwrap().unwrap();
    let parent = selection.anchor_node().unwrap();
    let offset = selection.anchor_offset();
    assert_eq!(parent.node_type(), web_sys::Node::ELEMENT_NODE);
    let engine = fixture.engine();
    let engine = engine.borrow();
    let point = &engine.selection().text_range().unwrap().head;
    let (text, text_offset) = mapping::to_dom(&fixture.root(), point).unwrap();
    let range = document.create_range().unwrap();
    range.set_start(&text, text_offset).unwrap();
    range.collapse_with_to_start(true);
    let expected = range.get_bounding_client_rect();
    assert!(expected.height() > 0.);
    let wrapper = fixture.wrapper.get_bounding_client_rect();
    let caret = geometry::measure(&fixture.root(), &engine).caret.unwrap();
    assert!((caret.x - (expected.x() - wrapper.x())).abs() < 0.1);
    assert!((caret.y - (expected.y() - wrapper.y())).abs() < 0.1);
    assert!((caret.height - expected.height()).abs() < 0.1);
    assert_eq!(selection.anchor_node().as_ref(), Some(&parent));
    assert_eq!(selection.anchor_offset(), offset);
}

#[wasm_bindgen_test]
fn composition_reconciles_native_split_text_nodes_once() {
    let fixture = Fixture::new(vec![paragraph("before")]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 6);
    let root = fixture.root();
    root.dispatch_event(&web_sys::Event::new("compositionstart").unwrap())
        .unwrap();
    let paragraph = root
        .query_selector("[data-inline-container]")
        .unwrap()
        .unwrap();
    let document = paragraph.owner_document().unwrap();
    paragraph
        .append_child(&document.create_text_node("日本"))
        .unwrap();
    let selection = document.get_selection().unwrap().unwrap();
    selection
        .set_base_and_extent(&paragraph, 2, &paragraph, 2)
        .unwrap();
    root.dispatch_event(&web_sys::Event::new("compositionend").unwrap())
        .unwrap();
    assert_eq!(fixture.text(), "before日本");
    assert_eq!(
        root.query_selector("[data-inline-container]")
            .unwrap()
            .unwrap()
            .text_content()
            .as_deref(),
        Some("before日本")
    );
    assert_eq!(fixture.change_count(), 1);
    assert_eq!(
        fixture
            .engine()
            .borrow()
            .selection()
            .text_range()
            .unwrap()
            .head
            .offset,
        8
    );
}

#[wasm_bindgen_test]
fn disabled_composer_chip_remove_is_inert() {
    use dxeditor::profiles::composer::{
        ComposerChip, ComposerSegment, ComposerValue, composer_to_blocks,
    };
    let document = composer_to_blocks(&ComposerValue {
        segments: vec![ComposerSegment::Chip(ComposerChip {
            kind: "entity".into(),
            label: "Ada".into(),
            payload: json!({"id":"one"}),
        })],
    });
    let fixture = Fixture::profile(
        model::children(&document, None).unwrap().clone(),
        EditorCatalog::composer(),
        Profile::Composer,
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let captured = events.clone();
    fixture
        .host
        .borrow_mut()
        .set_composer_event_callback(Rc::new(move |event| captured.borrow_mut().push(event)));
    let output = fixture.engine().borrow_mut().set_read_only(true);
    DomHost::publish(&fixture.host, output);
    fixture
        .root()
        .query_selector("[data-chip-remove]")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::HtmlElement>()
        .unwrap()
        .click();
    assert_eq!(
        fixture
            .root()
            .query_selector_all("[data-composer-chip]")
            .unwrap()
            .length(),
        1
    );
    assert_eq!(fixture.change_count(), 0);
    assert!(events.borrow().is_empty());
}

#[wasm_bindgen_test]
fn code_text_fast_path_keeps_trailing_newline_anchor() {
    let mut code = ComponentNode::new("code_block");
    code.content = vec![ComponentNode::text("code")];
    let fixture = Fixture::new(vec![model::block_node(code)]);
    fixture.set_caret(InlineContainerRef::Block(fixture.first_uid()), 4);
    input(&fixture.root(), "insertParagraph", None, true);
    assert!(
        fixture
            .root()
            .query_selector("pre code > br")
            .unwrap()
            .is_some()
    );
    input(&fixture.root(), "insertText", Some("next"), true);
    assert!(
        fixture
            .root()
            .query_selector("pre code > br")
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.text(), "code\nnext");
}

#[wasm_bindgen_test]
fn contextual_toolbar_focus_stays_owned_until_leaving_editor() {
    let fixture = Fixture::new(vec![paragraph("Selected text")]);
    let document = fixture.wrapper.owner_document().unwrap();
    let overlay = document.create_element("div").unwrap();
    overlay.set_attribute("data-dxeditor-overlays", "").unwrap();
    let button = document
        .create_element("button")
        .unwrap()
        .dyn_into::<web_sys::HtmlElement>()
        .unwrap();
    overlay.append_child(&button).unwrap();
    fixture.wrapper.append_child(&overlay).unwrap();
    fixture
        .root()
        .dyn_ref::<web_sys::HtmlElement>()
        .unwrap()
        .focus()
        .unwrap();
    DomHost::run(&fixture.host, Command::OpenMenu("link".into()));
    let output = fixture
        .engine()
        .borrow_mut()
        .set_entity_preview("entity-1".into(), None, true);
    DomHost::publish(&fixture.host, output);
    button.focus().unwrap();
    assert!(fixture.engine().borrow().ui_state().focused);
    assert!(fixture.engine().borrow().ui_state().open_menu.is_some());
    assert!(
        fixture
            .engine()
            .borrow()
            .ui_state()
            .entity_preview
            .is_some()
    );
    let outside = document
        .create_element("input")
        .unwrap()
        .dyn_into::<web_sys::HtmlElement>()
        .unwrap();
    document.body().unwrap().append_child(&outside).unwrap();
    outside.focus().unwrap();
    assert!(!fixture.engine().borrow().ui_state().focused);
    assert!(fixture.engine().borrow().ui_state().open_menu.is_none());
    assert!(
        fixture
            .engine()
            .borrow()
            .ui_state()
            .entity_preview
            .is_none()
    );
    assert_eq!(document.active_element().as_ref(), Some(outside.as_ref()));
    outside.remove();
}

#[wasm_bindgen_test]
fn editor_inside_an_outer_island_owns_its_own_focus() {
    let fixture = Fixture::new(vec![paragraph("Nested editor")]);
    let document = fixture.wrapper.owner_document().unwrap();
    let island = document.create_element("div").unwrap();
    island.set_class_name("dxeditor-island");
    document.body().unwrap().append_child(&island).unwrap();
    island.append_child(&fixture.wrapper).unwrap();
    fixture
        .root()
        .dyn_ref::<web_sys::HtmlElement>()
        .unwrap()
        .focus()
        .unwrap();
    assert!(fixture.engine().borrow().ui_state().focused);
    fixture.wrapper.remove();
    island.remove();
}

#[wasm_bindgen_test]
fn composer_clear_restores_empty_placeholder_state_and_undo() {
    let fixture = Fixture::profile(
        vec![paragraph("clear me"), paragraph("second line")],
        EditorCatalog::composer(),
        Profile::Composer,
    );
    DomHost::run(&fixture.host, Command::SelectAll);
    DomHost::run(&fixture.host, Command::SelectAll);
    DomHost::run(&fixture.host, Command::InsertText(String::new()));
    assert_eq!(
        fixture.root().get_attribute("data-empty").as_deref(),
        Some("true")
    );
    assert_eq!(fixture.text(), "");
    assert_eq!(fixture.change_count(), 1);
    DomHost::run(&fixture.host, Command::Undo);
    assert_eq!(fixture.text(), "clear me\nsecond line");
}
