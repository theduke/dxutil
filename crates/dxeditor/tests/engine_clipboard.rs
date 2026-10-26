use dxeditor::engine::{
    clipboard::*,
    html::{HtmlNode, blocks_to_html_with_slice, wrap_block},
};
use dxeditor::{
    ComponentMark, ComponentNode,
    profiles::composer::{ComposerChip, ComposerSegment, ComposerValue},
};
use serde_json::{Value, json};

fn element(tag: &str, children: Vec<HtmlNode>) -> HtmlNode {
    HtmlNode::element(tag, children)
}
fn text(value: &str) -> HtmlNode {
    HtmlNode::text(value)
}
fn own(node: &ComponentNode) -> &ComponentNode {
    &node.content[0]
}

#[test]
fn internal_slice_binds_schema_and_strips_ids() {
    let mut nested = ComponentNode::new("graph");
    nested.attrs.insert(
        "nodes".into(),
        json!([{"id":"019a0ea0-0000-7000-8000-000000000001"}]),
    );
    let block = wrap_block(
        nested,
        vec![wrap_block(ComponentNode::new("paragraph"), Vec::new())],
    );
    let slice = ClipboardSlice {
        content: vec![block],
        open_start: 0,
        open_end: 0,
    };
    let encoded = encode_internal_clipboard(&slice, "catalog-one").unwrap();
    assert!(decode_internal_clipboard(&encoded, "catalog-two").is_none());
    let decoded = decode_internal_clipboard(&encoded, "catalog-one").unwrap();
    assert!(decoded.content[0].id.is_none());
    assert!(decoded.content[0].content[1].content[0].id.is_none());
    assert_eq!(
        decoded.content[0].content[0].attrs["nodes"],
        slice.content[0].content[0].attrs["nodes"]
    );
    let mut malformed: Value = serde_json::from_str(&encoded).unwrap();
    malformed["version"] = json!(2);
    assert!(decode_internal_clipboard(&malformed.to_string(), "catalog-one").is_none());
}

#[test]
fn clipboard_limits_are_global_and_urls_checked() {
    let text = ComponentNode::text("x");
    let slice = ClipboardSlice {
        content: vec![text; MAX_CLIPBOARD_NODES + 1],
        ..Default::default()
    };
    assert!(encode_internal_clipboard(&slice, "x").is_err());
    let mut text = ComponentNode::text("safe label");
    let mut mark = ComponentMark::new("link");
    mark.attrs
        .insert("href".into(), json!("javascript:alert(1)"));
    text.marks.push(mark);
    assert!(
        encode_internal_clipboard(
            &ClipboardSlice {
                content: vec![text],
                ..Default::default()
            },
            "x"
        )
        .is_err()
    );
    let mut node = ComponentNode::new("paragraph");
    for _ in 0..34 {
        node = ComponentNode::container("block", None, vec![node]);
    }
    assert!(
        encode_internal_clipboard(
            &ClipboardSlice {
                content: vec![node],
                ..Default::default()
            },
            "x"
        )
        .is_err()
    );
    let mut image = ComponentNode::new("image");
    image.attrs.insert(
        "media".into(),
        json!({"type":"url","url":"data:image/svg+xml,bad"}),
    );
    assert!(
        encode_internal_clipboard(
            &ClipboardSlice {
                content: vec![image],
                ..Default::default()
            },
            "x"
        )
        .is_err()
    );
}

#[test]
fn tsv_quotes_padding_line_endings_and_limits() {
    assert_eq!(
        parse_tsv_grid("\"a\tb\"\t\"c\"\"d\"\r\nx\ty\r\n").unwrap(),
        vec![vec!["a\tb", "c\"d"], vec!["x", "y"]]
    );
    assert_eq!(
        parse_tsv_grid("a\tb\nc").unwrap(),
        vec![vec!["a", "b"], vec!["c", ""]]
    );
    assert!(parse_tsv_grid("a\nb").is_none());
    assert!(parse_tsv_grid("\"a\tb").is_none());
    assert!(parse_tsv_grid(&vec!["x"; 101].join("\t")).is_none());
    assert!(parse_tsv_grid(&vec!["x\ty"; 101].join("\n")).is_none());
}

#[test]
fn plain_blocks_preserve_empty_lines_and_markdown_detection() {
    let nodes = plain_blocks("a\r\n\r\nb\r");
    assert_eq!(nodes.len(), 4);
    assert_eq!(blocks_to_plain_text(&nodes), "a\n\nb\n");
    assert!(nodes.iter().all(|node| node.id.is_some()));
    for markdown in [
        "# Heading\ntext",
        "1. first\n2. second",
        "```rust\nlet x = 1;",
        "hello\n> quote",
    ] {
        assert!(looks_like_markdown(markdown));
    }
    for plain in ["# One line", "hello\nworld", "####### no\nhello"] {
        assert!(!looks_like_markdown(plain));
    }
}

#[test]
fn semantic_html_preserves_marks_lists_tables_and_safe_media() {
    let source = element(
        "body",
        vec![
            element(
                "p",
                vec![
                    text("Hello "),
                    element("strong", vec![text("world")]),
                    element("script", vec![text("evil")]),
                ],
            ),
            element(
                "ol",
                vec![element(
                    "li",
                    vec![
                        text("One"),
                        element("ul", vec![element("li", vec![text("nested")])]),
                    ],
                )],
            )
            .with_attr("start", "4"),
            element(
                "table",
                vec![element(
                    "tbody",
                    vec![
                        element(
                            "tr",
                            vec![
                                element("th", vec![text("A")]),
                                element("th", vec![text("B")]),
                            ],
                        ),
                        element(
                            "tr",
                            vec![
                                element("td", vec![text("C")])
                                    .with_attr("style", "text-align: right; color: red"),
                            ],
                        ),
                    ],
                )],
            ),
            element("img", Vec::new())
                .with_attr("src", "https://example.com/i.png")
                .with_attr("alt", "<photo>"),
            element("img", Vec::new()).with_attr("src", "javascript:evil"),
        ],
    );
    let blocks = html_to_blocks(&source);
    assert_eq!(blocks.len(), 4);
    assert_eq!(own(&blocks[0]).content[1].marks[0].kind.0, "bold");
    assert_eq!(own(&blocks[1]).attrs["start"], 4);
    assert_eq!(
        blocks[1].content[1].content[0].content[0].kind.0,
        "bulleted_list_item"
    );
    assert_eq!(own(&blocks[2]).content[1].content.len(), 2);
    assert_eq!(
        own(&blocks[2]).content[1].content[0].attrs["alignment"],
        "right"
    );
    let html = blocks_to_html(&blocks);
    assert!(html.contains("<strong>world</strong>"));
    assert!(html.contains("<ol start=\"4\">"));
    assert!(html.contains("<th>A</th>"));
    assert!(html.contains("alt=\"&lt;photo&gt;\""));
    assert!(!html.contains("evil"));
    assert!(!html.contains("javascript"));
}

#[test]
fn html_links_mention_escape_and_inline_images() {
    let source = element(
        "p",
        vec![
            text("before"),
            element("img", Vec::new()).with_attr("src", "https://example.com/a.png"),
            element("a", vec![text("after")]).with_attr("href", "javascript:alert(1)"),
        ],
    );
    let blocks = html_to_blocks(&source);
    assert_eq!(blocks.len(), 3);
    assert!(own(&blocks[2]).content[0].marks.is_empty());
    let mention =
        element("a", vec![text("@<Label>")]).with_attr("data-semantic-mention", "entity-one");
    let blocks = html_to_blocks(&element("p", vec![mention]));
    assert_eq!(own(&blocks[0]).content[0].kind.0, "mention");
    let html = blocks_to_html(&blocks);
    assert!(html.contains("&lt;Label&gt;"));
    let secondary = blocks_to_html_with_slice(&blocks, "\"<script>&");
    assert!(secondary.contains("data-semantic-slice=\"&quot;&lt;script&gt;&amp;\""));
}

#[test]
fn url_policy_corpus_and_entity_origin_classification() {
    let cases: Value =
        serde_json::from_str(include_str!("../assets/url_policy_cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let url = case["value"].as_str().unwrap();
        assert_eq!(
            dxeditor::is_safe_url(url),
            case["hyperlink"].as_bool().unwrap(),
            "{url}"
        );
        assert_eq!(
            dxeditor::is_safe_media_url(url),
            case["media"].as_bool().unwrap(),
            "{url}"
        );
    }
    assert!(
        matches!(classify_url("semantic:entity:abc", None), Some(ClassifiedUrl::Entity { id, .. }) if id == "abc")
    );
    assert!(
        matches!(classify_url("/entities/a%20b", Some("https://example.com")), Some(ClassifiedUrl::Entity { id, .. }) if id == "a b")
    );
    assert!(matches!(
        classify_url(
            "https://other.example/entities/x",
            Some("https://example.com")
        ),
        Some(ClassifiedUrl::External(_))
    ));
    assert!(matches!(
        classify_url("/entities/one/child", Some("https://example.com")),
        Some(ClassifiedUrl::External(_))
    ));
    assert!(classify_url("/entities/%ZZ", Some("https://example.com")).is_none());
    assert!(classify_url("/entities/%FF", Some("https://example.com")).is_none());
    assert!(classify_url("javascript:alert(1)", None).is_none());
}

#[test]
fn composer_clipboard_retains_chips_and_routes_attachments() {
    let value = ComposerValue {
        segments: vec![
            ComposerSegment::Text("Hi ".into()),
            ComposerSegment::Chip(ComposerChip {
                kind: "entity".into(),
                label: "Person".into(),
                payload: json!({"id":"one"}),
            }),
        ],
    };
    let encoded = encode_composer_clipboard(&value).unwrap();
    assert_eq!(decode_composer_clipboard(&encoded).unwrap().value, value);
    let mut encoded: Value = serde_json::from_str(&encoded).unwrap();
    encoded["attachments"] = json!([{"name":"image.png"}]);
    assert_eq!(
        decode_composer_clipboard(&encoded.to_string())
            .unwrap()
            .attachments
            .unwrap()
            .len(),
        1
    );
    for malformed in [
        "{}",
        "{\"segments\":[{\"Chip\":{\"kind\":\"a\",\"label\":\"b\"}}]}",
        "{\"segments\":[{\"Text\":3}]}",
    ] {
        assert!(decode_composer_clipboard(malformed).is_none());
    }
}
