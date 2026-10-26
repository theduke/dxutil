#![cfg(all(feature = "viewer", not(target_arch = "wasm32")))]
use dioxus::prelude::*;
use dxhighlight::{CodeView, LineTarget};

fn render(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
#[cfg(feature = "rust")]
fn tokens_numbers_and_goto_in_ssr() {
    let html = render(|| {
        rsx! { CodeView {
            value: "fn main() {}\r\n// 😀\r\n".to_string(), path: "main.rs".to_string(),
            goto: LineTarget { line: 2, column: Some(4) }, viewport_lines: None,
        } }
    });
    assert!(html.contains("class=\"tok-keyword\">fn</span>"));
    assert!(html.contains("class=\"dxhighlight-number\""));
    assert!(html.contains("data-line=\"2\" data-target=\"true\" data-column=\"4\""));
    assert!(html.contains("data-line-count=\"3\""));
    assert!(!html.contains("\r"));
    assert!(!html.contains("<script"));
}

#[test]
fn plain_text_and_no_numbers() {
    let html = render(
        || rsx! { CodeView { value: "<plain> & 😀".to_string(), path: "log.txt".to_string(), line_numbers: false } },
    );
    assert!(html.contains("&#60;plain&#62; &#38; 😀"));
    // The stylesheet contains the class name, so inspect actual elements.
    assert!(!html.contains("class=\"dxhighlight-number\""));
    assert!(html.contains("data-line=\"1\""));
}

#[test]
fn viewport_and_distant_goto() {
    let html = render(|| {
        rsx! { CodeView {
            value: "line\n".repeat(8000), path: "log.txt".to_string(),
            viewport_lines: Some(40), goto: LineTarget { line: 4000, column: None },
        } }
    });
    assert!(html.contains("data-line-count=\"8001\""));
    assert!(html.contains("data-line=\"4000\" data-target=\"true\""));
    assert!(html.matches("class=\"dxhighlight-line\"").count() <= 64);
    assert!(!html.contains("data-line=\"8000\""));
}

#[test]
fn large_input_renders_plain_rows_before_background_highlighting() {
    let html = render(|| {
        rsx! { CodeView {
            value: "fn main() {}\n".repeat(20000), path: "main.rs".to_string(),
        } }
    });
    assert!(html.contains("data-highlight-pending=\"true\""));
    assert!(html.contains("fn main() {}"));
    assert!(html.matches("class=\"dxhighlight-line\"").count() <= 144);
}
