use dioxus::prelude::*;
use dxeditor::{BlockNode, EditorCatalog, EditorDocument, InlineNode, Mark};

#[test]
fn dioxus_smoke_renders_marked_spans() {
    let document = EditorDocument::new(vec![BlockNode::paragraph(
        "block-1",
        vec![
            InlineNode::text("text-1", "Bold").with_mark(Mark::new("bold")),
            InlineNode::text("text-2", " and "),
            InlineNode::text("text-3", "code").with_mark(Mark::new("code")),
            InlineNode::text("text-4", " link").with_mark(Mark::link("https://example.com")),
        ],
    )]);

    let html = dioxus_ssr::render_element(rsx! {
        dxeditor::DocumentView {
            document,
            catalog: EditorCatalog::default(),
        }
    });

    assert!(html.contains("<strong>"));
    assert!(html.contains("<code"));
    assert!(html.contains("<a href=\"https://example.com\""));
}

#[test]
fn document_view_does_not_render_unsafe_link_destinations() {
    let document = EditorDocument::new(vec![BlockNode::paragraph(
        "block-1",
        vec![InlineNode::text("text-1", "unsafe").with_mark(Mark::link("javascript:alert(1)"))],
    )]);

    let html = dioxus_ssr::render_element(rsx! {
        dxeditor::DocumentView {
            document,
            catalog: EditorCatalog::default(),
        }
    });

    assert!(html.contains("unsafe"));
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("<a href="));
}

#[test]
fn native_markdown_editor_exposes_an_accessible_textarea() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            dxeditor::MarkdownEditor { value: "hello".to_string(), on_change: |_| {} }
        }
    });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("textarea"));
    assert!(html.contains("aria-label=\"Document editor\""));
    assert!(html.contains("hello"));
}
