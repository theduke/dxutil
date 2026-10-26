use crate::{CodeSelection, HighlightSession, LineTarget, language_for_path};
mod lines;
use dioxus::prelude::*;
use lines::RenderedLine;
use std::{
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const CSS: &str = include_str!("code.css");
const LINE_HEIGHT: usize = 20;
const OVERSCAN: usize = 12;
const DEFER_PARSE_BYTES: usize = 128 * 1024;

struct Document {
    session: Option<HighlightSession>,
    path: Option<String>,
    text: String,
    lines: Vec<Range<usize>>,
}
impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.path == other.path
    }
}
impl Document {
    fn new(value: String, path: Option<String>) -> Self {
        let mut document = Self::plain(value, path);
        if document.text.len() <= DEFER_PARSE_BYTES {
            document.session =
                HighlightSession::new(document.language(), document.text.clone()).ok();
        }
        document
    }

    fn language(&self) -> &'static str {
        self.path
            .as_deref()
            .and_then(language_for_path)
            .unwrap_or("")
    }

    fn plain(value: String, path: Option<String>) -> Self {
        let lines = lines::index(&value);
        Self {
            session: None,
            path,
            text: value,
            lines,
        }
    }

    fn render_lines(&self, range: Range<usize>) -> Vec<RenderedLine> {
        let start = self.lines[range.start].start;
        let end = self.lines[range.end - 1].end;
        let spans = self
            .session
            .as_ref()
            .and_then(|s| s.highlight(start..end).ok())
            .unwrap_or_default();
        lines::render(&self.text, &self.lines[range.clone()], range.start, &spans)
    }
}

/// Read-only fixed-height rows, rendered entirely as RSX. `viewport_lines: None`
/// renders every line (useful for SSR/export); the default window is 120 lines.
/// Scroll events update the viewport without JavaScript or renderer-specific APIs.
#[component]
pub fn CodeView(
    value: String,
    path: Option<String>,
    #[props(default)] goto: Option<LineTarget>,
    #[props(default)] on_selection: Option<EventHandler<CodeSelection>>,
    #[props(default = true)] line_numbers: bool,
    #[props(default = Some(120))] viewport_lines: Option<usize>,
) -> Element {
    let id = use_hook(|| format!("dxcode-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    let document = use_memo(use_reactive((&value, &path), |(value, path)| {
        Document::new(value, path)
    }));
    let background = use_resource(use_reactive((&value, &path), |(value, path)| async move {
        if value.len() <= DEFER_PARSE_BYTES {
            return None;
        }
        deferred_document(value, path).await
    }));
    let mut scroll_line = use_signal(|| {
        goto.as_ref()
            .map_or(0, |t| t.line.saturating_sub(1) as usize)
    });
    let mut measured_rows = use_signal(|| viewport_lines.unwrap_or(120).max(1));
    let mut previous_goto = use_signal(|| goto.clone());
    use_effect(use_reactive((&goto,), move |(goto,)| {
        if *previous_goto.peek() != goto {
            scroll_line.set(
                goto.as_ref()
                    .map_or(0, |t| t.line.saturating_sub(1) as usize),
            );
            previous_goto.set(goto);
        }
    }));
    let initial = document.read();
    let completed = background.read();
    let doc = completed
        .as_ref()
        .and_then(Option::as_ref)
        .filter(|doc| doc.text == value && doc.path == path)
        .unwrap_or(&initial);
    let count = doc.lines.len();
    let requested = if *previous_goto.peek() == goto {
        scroll_line()
    } else {
        goto.as_ref()
            .map_or(0, |t| t.line.saturating_sub(1) as usize)
    };
    let start = if viewport_lines.is_some() {
        requested.min(count - 1).saturating_sub(OVERSCAN)
    } else {
        0
    };
    let end = if viewport_lines.is_some() {
        (start + measured_rows() + OVERSCAN * 2).min(count)
    } else {
        count
    };
    let rows = doc.render_lines(start..end);
    let selection_id = id.clone();
    let key_selection_id = id.clone();
    rsx! {
        style { "{CSS}" }
        div {
            id,
            class: "dxcode",
            role: "region",
            "aria-label": "Code viewer",
            "data-line-count": "{count}",
            "data-highlight-pending": if value.len() > DEFER_PARSE_BYTES && doc.session.is_none() { "true" } else { "false" },
            "data-first-line": "{start + 1}",
            "data-last-line": "{end}",
            onscroll: move |event| {
                scroll_line.set((event.scroll_top().max(0.0) as usize) / LINE_HEIGHT);
                measured_rows.set((event.client_height().max(1) as usize).div_ceil(LINE_HEIGHT).max(viewport_lines.unwrap_or(1)));
            },
            onmouseup: move |_| selection(&selection_id, on_selection),
            onkeyup: move |_| selection(&key_selection_id, on_selection),
            pre {
                class: "dxcode-lines",
                if start > 0 { div { "aria-hidden": "true", style: "height:{start * LINE_HEIGHT}px" } }
                for row in rows {
                    div {
                        key: "{row.number}:{goto.as_ref().is_some_and(|t| t.line as usize == row.number)}",
                        class: "dxcode-line",
                        "data-line": "{row.number}",
                        "data-target": if goto.as_ref().is_some_and(|t| t.line as usize == row.number) { "true" } else { "false" },
                        "data-column": goto.as_ref().filter(|t| t.line as usize == row.number).and_then(|t| t.column).map(|c| c.to_string()),
                        tabindex: "-1",
                        onmounted: {
                            let target = goto.as_ref().is_some_and(|t| t.line as usize == row.number);
                            move |event| async move {
                                if target {
                                    let _ = event.scroll_to_with_options(ScrollToOptions { behavior: ScrollBehavior::Instant, vertical: ScrollLogicalPosition::Center, horizontal: ScrollLogicalPosition::Nearest }).await;
                                    let _ = event.set_focus(true).await;
                                }
                            }
                        },
                        if line_numbers { span { class: "dxcode-number", "aria-hidden": "true", "{row.number}" } }
                        span { class: "dxcode-text", for (text, class) in row.parts { span { class, "{text}" } } "\n" }
                    }
                }
                if end < count { div { "aria-hidden": "true", style: "height:{(count - end) * LINE_HEIGHT}px" } }
            }
        }
    }
}

#[cfg(not(feature = "dom"))]
fn selection(_: &str, _: Option<EventHandler<CodeSelection>>) {}

#[cfg(feature = "dom")]
fn selection(id: &str, handler: Option<EventHandler<CodeSelection>>) {
    let Some(handler) = handler else {
        return;
    };
    let id = serde_json::to_string(id).expect("serialize viewer id");
    // Selection is the only DOM-specific extra. Unsupported eval returns an
    // error that is ignored; it never participates in rendering or scrolling.
    let script = format!(
        r#"
const host = document.getElementById({id});
const s = window.getSelection();
const row = n => (n?.nodeType === 1 ? n : n?.parentElement)?.closest('[data-line]');
const a = row(s?.anchorNode), b = row(s?.focusNode);
if (host && s && !s.isCollapsed && a && b && host.contains(a) && host.contains(b)) {{
    const range = s.getRangeAt(0).cloneContents();
    range.querySelectorAll('.dxcode-number').forEach(n => n.remove());
    dioxus.send({{start_line: Math.min(+a.dataset.line, +b.dataset.line), end_line: Math.max(+a.dataset.line, +b.dataset.line), text: range.textContent}});
}} else {{ dioxus.send(null); }}
"#
    );
    spawn(async move {
        if let Ok(Some(selection)) = document::eval(&script)
            .recv::<Option<CodeSelection>>()
            .await
        {
            handler.call(selection);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
async fn deferred_document(value: String, path: Option<String>) -> Option<Document> {
    let (send, receive) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let mut document = Document::plain(value, path);
        document.session = HighlightSession::new(document.language(), document.text.clone()).ok();
        let _ = send.send(document);
    });
    receive.await.ok()
}

#[cfg(target_arch = "wasm32")]
async fn deferred_document(value: String, path: Option<String>) -> Option<Document> {
    gloo_timers::future::TimeoutFuture::new(0).await;
    let mut document = Document::plain(value, path);
    if let Ok(mut session) = HighlightSession::prepare(document.language(), document.text.clone()) {
        while !session.parse_step() {
            gloo_timers::future::TimeoutFuture::new(0).await;
        }
        document.session = Some(session);
    }
    Some(document)
}

#[cfg(all(test, target_arch = "wasm32", feature = "rust"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test(async)]
    async fn deferred_parse_resumes_and_matches_full_parse() {
        let text = "fn main() { let café = 42; }\n".repeat(8000);
        let mut pending = HighlightSession::prepare("rust", text.clone()).unwrap();
        let mut steps = 0;
        while !pending.parse_step() {
            steps += 1;
            assert!(steps < 10000, "parser did not resume");
        }
        assert!(steps > 0);
        let expected = HighlightSession::new("rust", &text)
            .unwrap()
            .highlight(0..100)
            .unwrap();
        assert_eq!(pending.highlight(0..100).unwrap(), expected);
        let document = deferred_document(text, Some("main.rs".into()))
            .await
            .unwrap();
        assert_eq!(
            document.session.unwrap().highlight(0..100).unwrap(),
            expected
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "rust"))]
mod native_tests {
    use super::*;

    #[test]
    fn deferred_host_parse_matches_synchronous_parse() {
        let text = "fn main() { let café = 42; }\n".repeat(8000);
        let expected = HighlightSession::new("rust", &text)
            .unwrap()
            .highlight(0..100)
            .unwrap();
        let document =
            futures::executor::block_on(deferred_document(text, Some("main.rs".into()))).unwrap();
        assert_eq!(
            document.session.unwrap().highlight(0..100).unwrap(),
            expected
        );
    }
}
