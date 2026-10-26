#![cfg(all(
    target_arch = "wasm32",
    feature = "rust",
    feature = "markdown",
    feature = "json"
))]
use dxhighlight::{HighlightSession, TextEdit, TokenKind};
use wasm_bindgen_test::wasm_bindgen_test;

// Run in Node by default; WASM_BINDGEN_USE_BROWSER=1 also supports a browser.
#[wasm_bindgen_test]
fn rust_markdown_injections_and_allocator_coexistence() {
    let eof = HighlightSession::new("markdown", "# café😀").unwrap();
    assert_eq!(eof.text(), "# café😀");
    assert!(
        eof.highlight_all()
            .unwrap()
            .iter()
            .any(|s| s.kind == TokenKind::Heading)
    );
    for _ in 0..32 {
        let rust = HighlightSession::new("rust", "fn main() { let café = \"😀\"; }").unwrap();
        assert!(
            rust.highlight_all()
                .unwrap()
                .iter()
                .any(|s| s.kind == TokenKind::Keyword)
        );
        let mut markdown = HighlightSession::new(
            "markdown",
            "# Title\n\n*inline*\n\n```rust\nfn main() {}\n```\n\n```json\n{\"é\":42}\n```\n",
        )
        .unwrap();
        let spans = markdown.highlight_all().unwrap();
        assert!(
            spans
                .iter()
                .any(|s| s.kind == TokenKind::Keyword && &markdown.text()[s.range.clone()] == "fn")
        );
        assert!(
            spans
                .iter()
                .any(|s| s.kind == TokenKind::Number && &markdown.text()[s.range.clone()] == "42")
        );
        assert!(spans.iter().any(|s| s.kind == TokenKind::Emphasis));
        let start = markdown.text().find("42").unwrap();
        markdown
            .apply_edit(TextEdit {
                range: start..start + 2,
                replacement: "true".into(),
            })
            .unwrap();
        let fresh = HighlightSession::new("markdown", markdown.text()).unwrap();
        assert_eq!(
            markdown.highlight_all().unwrap(),
            fresh.highlight_all().unwrap()
        );
    }
}
