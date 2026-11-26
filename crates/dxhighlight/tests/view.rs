#![cfg(all(feature = "viewer", not(target_arch = "wasm32")))]
use dioxus::prelude::*;
use dxhighlight::{CodeFind, CodeView, FindQuery, LineTarget};

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

#[test]
fn find_marks_matches_and_scrolls_to_the_active_one() {
    let html = render(|| {
        let mut lines = vec!["line".to_string(); 8000];
        lines[3999] = "x Needle needle".into();
        lines[4000] = "needle".into();
        rsx! { CodeView {
            value: lines.join("\n"), path: "log.txt".to_string(), viewport_lines: Some(40),
            find: CodeFind { query: FindQuery { text: "needle".into(), case_sensitive: false }, active: 1 },
        } }
    });
    assert!(html.contains("data-match-count=\"3\""));
    let row = html.split("data-line=\"4000\"").nth(1).unwrap();
    let row = row.split("data-line=\"4001\"").next().unwrap();
    assert!(row.contains("data-active=\"false\">Needle</span>"));
    assert!(row.contains("data-active=\"true\">needle</span>"));
    assert_eq!(html.matches("data-active=\"true\"").count(), 1);
}
