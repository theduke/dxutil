use crate::{
    CodeSelection, FindQuery, FindSummary, HighlightSession, LineTarget, language_for_path, search,
};
mod find;
mod lines;
use dioxus::prelude::*;
pub use find::{FindBar, FindBarProps, SearchableCodeView, SearchableCodeViewProps};
use lines::{Mark, Marks, RenderedLine};
use std::{
    cell::OnceCell,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) const CSS: &str = include_str!("code.css");
const LINE_HEIGHT: usize = 20;
const OVERSCAN: usize = 12;
const DEFER_PARSE_BYTES: usize = 128 * 1024;

pub(crate) fn next_id() -> String {
    format!("dxhighlight-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Document-wide find input for [`CodeView`]. `active` wraps modulo the match count.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeFind {
    pub query: FindQuery,
    pub active: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Matches {
    ranges: Vec<Range<usize>>,
    capped: bool,
}

/// What the viewport last jumped to; `seq` remounts the jump row so it scrolls again.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Jump {
    line: usize,
    seq: u64,
    find: bool,
}

struct Document {
    session: Option<HighlightSession>,
    path: Option<String>,
    text: String,
    lines: Vec<Range<usize>>,
    folded: OnceCell<String>,
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
            folded: OnceCell::new(),
        }
    }

    fn find(&self, query: &FindQuery) -> Matches {
        let (ranges, capped) = if query.case_sensitive || !search::searchable(&query.text) {
            search::find_in(&self.text, &query.text)
        } else {
            let folded = self.folded.get_or_init(|| self.text.to_ascii_lowercase());
            search::find_in(folded, &query.text.to_ascii_lowercase())
        };
        Matches { ranges, capped }
    }

    fn line_of(&self, byte: usize) -> usize {
        self.lines.partition_point(|line| line.end < byte)
    }

    fn render_lines(&self, range: Range<usize>, marks: Marks<'_>) -> Vec<RenderedLine> {
        let start = self.lines[range.start].start;
        let end = self.lines[range.end - 1].end;
        let spans = self
            .session
            .as_ref()
            .and_then(|s| s.highlight(start..end).ok())
            .unwrap_or_default();
        lines::render(
            &self.text,
            &self.lines[range.clone()],
            range.start,
            &spans,
            marks,
        )
    }
}

fn wrap(active: usize, count: usize) -> Option<usize> {
    (count > 0).then(|| active % count)
}

/// The zero-based line a changed `goto` or find state should bring into view.
fn jump_line(
    previous: &(Option<LineTarget>, Option<CodeFind>),
    next: &(Option<LineTarget>, Option<CodeFind>),
    document: &Document,
    matches: &Matches,
) -> Option<(usize, bool)> {
    if previous.0 != next.0 {
        return next
            .0
            .as_ref()
            .map(|t| (t.line.saturating_sub(1) as usize, false));
    }
    if previous.1 != next.1 {
        let active = wrap(next.1.as_ref()?.active, matches.ranges.len())?;
        return Some((document.line_of(matches.ranges[active].start), true));
    }
    None
}

/// Read-only fixed-height rows, rendered entirely as RSX. `viewport_lines: None`
/// renders every line (useful for SSR/export); the default window is 120 lines.
/// Scroll events update the viewport without JavaScript or renderer-specific APIs.
/// `find` marks every match of its query and scrolls the active one into view;
/// `on_find` reports the match count whenever it changes.
#[component]
pub fn CodeView(
    value: String,
    path: Option<String>,
    #[props(default)] goto: Option<LineTarget>,
    #[props(default)] on_selection: Option<EventHandler<CodeSelection>>,
    #[props(default = true)] line_numbers: bool,
    #[props(default = Some(120))] viewport_lines: Option<usize>,
    #[props(default)] find: Option<CodeFind>,
    #[props(default)] on_find: Option<EventHandler<FindSummary>>,
) -> Element {
    let id = use_hook(next_id);
    let document = use_memo(use_reactive((&value, &path), |(value, path)| {
        Document::new(value, path)
    }));
    let query = find.as_ref().map(|f| f.query.clone());
    let matches = use_memo(use_reactive((&query,), move |(query,)| {
        query.map_or_else(Matches::default, |query| document.read().find(&query))
    }));
    let summary = use_memo(move || {
        let matches = matches.read();
        FindSummary {
            count: matches.ranges.len(),
            capped: matches.capped,
        }
    });
    use_effect(move || {
        let summary = summary();
        if let Some(on_find) = on_find {
            on_find.call(summary);
        }
    });
    let background = use_resource(use_reactive((&value, &path), |(value, path)| async move {
        if value.len() <= DEFER_PARSE_BYTES {
            return None;
        }
        deferred_document(value, path).await
    }));
    let mut scroll_line = use_signal(|| 0);
    let mut measured_rows = use_signal(|| viewport_lines.unwrap_or(120).max(1));
    // Jump state commits after render; until then the pending jump is derived
    // from the changed inputs so the first frame already shows its window.
    let mut jumped_for = use_signal(|| (None::<LineTarget>, None::<CodeFind>));
    let mut jump = use_signal(Jump::default);
    let scrolled = use_signal(|| 0u64);
    let key = (goto.clone(), find.clone());
    let pending = jump_line(&jumped_for.peek(), &key, &document.read(), &matches.read()).map(
        |(line, find)| Jump {
            line,
            find,
            seq: jump.peek().seq + 1,
        },
    );
    use_effect(use_reactive((&key,), move |(key,)| {
        if *jumped_for.peek() == key {
            return;
        }
        let next = jump_line(&jumped_for.peek(), &key, &document.peek(), &matches.peek());
        if let Some((line, find)) = next {
            let seq = jump.peek().seq + 1;
            jump.set(Jump { line, seq, find });
            scroll_line.set(line);
        }
        jumped_for.set(key);
    }));
    let jump = pending.unwrap_or(jump());
    let initial = document.read();
    let completed = background.read();
    let doc = completed
        .as_ref()
        .and_then(Option::as_ref)
        .filter(|doc| doc.text == value && doc.path == path)
        .unwrap_or(&initial);
    let count = doc.lines.len();
    let requested = pending.map_or(scroll_line(), |jump| jump.line);
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
    let found = matches.read();
    let active = find
        .as_ref()
        .and_then(|f| wrap(f.active, found.ranges.len()));
    let rows: Vec<_> = doc
        .render_lines(
            start..end,
            Marks {
                matches: &found.ranges,
                active,
            },
        )
        .into_iter()
        .map(|row| {
            let target = goto.as_ref().is_some_and(|t| t.line as usize == row.number);
            let jumped = jump.seq > 0 && jump.line + 1 == row.number;
            (row, target, jumped)
        })
        .collect();
    let match_count = find.as_ref().map(|_| found.ranges.len().to_string());
    let selection_id = id.clone();
    let key_selection_id = id.clone();
    rsx! {
        style { "{CSS}" }
        div {
            id,
            class: "dxhighlight",
            role: "region",
            "aria-label": "Code viewer",
            "data-line-count": "{count}",
            "data-match-count": match_count,
            "data-highlight-pending": if value.len() > DEFER_PARSE_BYTES && doc.session.is_none() { "true" } else { "false" },
            "data-first-line": "{start + 1}",
            "data-last-line": "{end}",
            onscroll: move |event| {
                let next_line = (event.scroll_top().max(0.0) as usize) / LINE_HEIGHT;
                if *scroll_line.peek() != next_line {
                    scroll_line.set(next_line);
                }
                let next_rows = (event.client_height().max(1) as usize).div_ceil(LINE_HEIGHT).max(viewport_lines.unwrap_or(1));
                if *measured_rows.peek() != next_rows {
                    measured_rows.set(next_rows);
                }
            },
            onmouseup: move |_| selection(&selection_id, on_selection),
            onkeyup: move |_| selection(&key_selection_id, on_selection),
            pre {
                class: "dxhighlight-lines",
                if start > 0 { div { "aria-hidden": "true", style: "height:{start * LINE_HEIGHT}px" } }
                for (row, target, jumped) in rows {
                    div {
                        key: "{row.number}:{target}:{jumped.then_some(jump.seq).unwrap_or_default()}",
                        class: "dxhighlight-line",
                        "data-line": "{row.number}",
                        "data-target": if target { "true" } else { "false" },
                        "data-column": goto.as_ref().filter(|_| target).and_then(|t| t.column).map(|c| c.to_string()),
                        tabindex: "-1",
                        onmounted: move |event| async move {
                            if jumped && !jump.find && take_scroll(scrolled, jump.seq) {
                                let _ = event.scroll_to_with_options(ScrollToOptions { behavior: ScrollBehavior::Instant, vertical: ScrollLogicalPosition::Center, horizontal: ScrollLogicalPosition::Nearest }).await;
                                let _ = event.set_focus(true).await;
                            }
                        },
                        if line_numbers { span { class: "dxhighlight-number", "aria-hidden": "true", "{row.number}" } }
                        span { class: "dxhighlight-text",
                            for part in row.parts {
                                if part.mark == Mark::None {
                                    span { class: part.class, "{part.text}" }
                                } else {
                                    span {
                                        class: "{part.class} dxhighlight-match",
                                        "data-active": if part.mark == Mark::Active { "true" } else { "false" },
                                        onmounted: move |event| async move {
                                            if part.mark == Mark::Active && jumped && jump.find && take_scroll(scrolled, jump.seq) {
                                                let _ = event.scroll_to_with_options(ScrollToOptions { behavior: ScrollBehavior::Instant, vertical: ScrollLogicalPosition::Center, horizontal: ScrollLogicalPosition::Nearest }).await;
                                            }
                                        },
                                        "{part.text}"
                                    }
                                }
                            }
                            "\n"
                        }
                    }
                }
                if end < count { div { "aria-hidden": "true", style: "height:{(count - end) * LINE_HEIGHT}px" } }
            }
        }
    }
}

/// Scrolls once per jump, so rows remounted by later scrolling stay put.
fn take_scroll(mut scrolled: Signal<u64>, seq: u64) -> bool {
    if *scrolled.peek() == seq {
        return false;
    }
    scrolled.set(seq);
    true
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
    range.querySelectorAll('.dxhighlight-number').forEach(n => n.remove());
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
