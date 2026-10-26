#[cfg(feature = "rust")]
use dxhighlight::language_named;
use dxhighlight::{HighlightSession, highlight_lines, language_for_path, languages};
#[cfg(feature = "rust")]
use dxhighlight::{TextEdit, TokenKind};

#[cfg(any(
    feature = "rust",
    feature = "typescript",
    feature = "tsx",
    feature = "html"
))]
fn golden(language: &str, text: &str) -> String {
    let session = HighlightSession::new(language, text).unwrap();
    session
        .highlight_all()
        .unwrap()
        .iter()
        .map(|span| format!("{}:{:?}", span.kind.class(), &text[span.range.clone()]))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn enabled_queries_compile() {
    for language in languages() {
        HighlightSession::new(language.name, "")
            .unwrap_or_else(|e| panic!("{}: {e}", language.name));
    }
}

#[test]
fn unknown_language_is_plain() {
    let session = HighlightSession::new("nonexistent", "hello 😀\r\nworld").unwrap();
    assert!(session.language().is_none());
    assert!(session.highlight_all().unwrap().is_empty());
    assert_eq!(
        highlight_lines("nonexistent", &["hi", "世界"]).unwrap(),
        vec![vec![], vec![]]
    );
    assert_eq!(language_for_path("readme.unknown"), None);
}

#[test]
#[cfg(feature = "rust")]
fn rust_golden() {
    assert_eq!(
        golden("rust", "fn main() { let café = 42; }"),
        "tok-keyword:\"fn\"\ntok-name:\"main\"\ntok-punctuation:\"()\"\ntok-punctuation:\"{\"\ntok-keyword:\"let\"\ntok-number:\"42\"\ntok-punctuation:\";\"\ntok-punctuation:\"}\""
    );
    assert_eq!(language_for_path("src\\lib.RS"), Some("rust"));
    assert_eq!(language_named("rs").unwrap().name, "rust");
}

#[test]
#[cfg(feature = "typescript")]
fn typescript_golden() {
    assert_eq!(
        golden("ts", "const x: number = 1;"),
        "tok-keyword:\"const\"\ntok-name:\"x\"\ntok-type:\"number\"\ntok-operator:\"=\"\ntok-number:\"1\"\ntok-punctuation:\";\""
    );
}

#[test]
#[cfg(feature = "tsx")]
fn tsx_golden() {
    assert_eq!(
        golden("tsx", "const x = <div>{42}</div>;"),
        "tok-keyword:\"const\"\ntok-name:\"x\"\ntok-operator:\"=\"\ntok-punctuation:\"<\"\ntok-name:\"div\"\ntok-punctuation:\">{\"\ntok-number:\"42\"\ntok-punctuation:\"}</\"\ntok-name:\"div\"\ntok-punctuation:\">;\""
    );
}

#[test]
#[cfg(all(feature = "markdown", feature = "rust", feature = "json"))]
fn markdown_injections_and_inline() {
    let text = "# Title\n\n*hello*\n\n```rust\nfn main() {}\n```\n\n```json\n{\"a\":1}\n```\n\n```missing\nfn main() {}\n```\n";
    let output = golden("md", text);
    assert_eq!(output, include_str!("golden/markdown.txt").trim_end());
    assert_eq!(language_named("language-rust extra").unwrap().name, "rust");
}

#[test]
#[cfg(all(feature = "html", feature = "javascript", feature = "css"))]
fn html_injections() {
    assert_eq!(
        golden(
            "html",
            "<script>const x = 42;</script><style>a { color: red; }</style>"
        ),
        include_str!("golden/html.txt").trim_end()
    );
}

#[test]
#[cfg(feature = "rust")]
fn unicode_crlf_and_char_offsets() {
    let lines = ["// 😀\r", "let café = \"é😀\";\r", ""];
    let tokens = highlight_lines("rust", &lines).unwrap();
    assert_eq!(tokens[0][0].start, 0);
    assert_eq!(tokens[0][0].end, 4);
    let string = tokens[1].iter().find(|s| s.class == "tok-string").unwrap();
    assert_eq!((string.start, string.end), (11, 15));
    assert!(tokens[2].is_empty());
    let session = HighlightSession::new("rust", lines.join("\n")).unwrap();
    assert!(session.highlight(4..5).is_err());
}

#[test]
#[cfg(feature = "rust")]
fn incremental_edits_and_viewport() {
    let mut session =
        HighlightSession::new("rust", "fn main() {\r\n let café = 12;\r\n}\r\n").unwrap();
    for (needle, replacement) in [
        ("12", "\"😀\""),
        ("café", "other"),
        ("main", "hello"),
        ("\"😀\"", "false"),
        (" let other = false;", " // commented"),
        (" // commented", ""),
    ] {
        let start = session.text().find(needle).unwrap();
        let changed = session
            .apply_edit(TextEdit {
                range: start..start + needle.len(),
                replacement: replacement.into(),
            })
            .unwrap();
        assert!(
            changed
                .iter()
                .any(|r| r.start <= start && r.end >= start + replacement.len())
        );
        let fresh = HighlightSession::new("rust", session.text()).unwrap();
        assert_eq!(
            session.highlight_all().unwrap(),
            fresh.highlight_all().unwrap()
        );
        for range in changed {
            let expected: Vec<_> = fresh
                .highlight_all()
                .unwrap()
                .into_iter()
                .filter_map(|mut s| {
                    s.range = s.range.start.max(range.start)..s.range.end.min(range.end);
                    (!s.range.is_empty()).then_some(s)
                })
                .collect();
            assert_eq!(session.highlight(range).unwrap(), expected);
        }
    }
    let text = "fn main() { let x = 42; }\n".repeat(8000);
    let session = HighlightSession::new("rust", text).unwrap();
    let start = session.text().find("42").unwrap();
    let spans = session.highlight(start..start + 2).unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, TokenKind::Number);
    assert_eq!(spans[0].range, start..start + 2);
}

#[test]
#[cfg(all(feature = "markdown", feature = "rust", feature = "json"))]
fn incremental_cache_refresh_across_injections() {
    fn labels(session: &HighlightSession) -> Vec<Option<TokenKind>> {
        let mut labels = vec![None; session.text().len()];
        for span in session.highlight_all().unwrap() {
            labels[span.range].fill(Some(span.kind));
        }
        labels
    }
    let mut session = HighlightSession::new(
        "markdown",
        "# Title\r\n\r\n```rust\r\nfn main() { let n = 42; }\r\n```\r\n",
    )
    .unwrap();
    let mut cache = labels(&session);
    for (needle, replacement) in [
        ("42", "\"😀\""),
        ("rust", "missing"),
        ("missing", "json"),
        ("fn main() { let n = \"😀\"; }", "{\"é\":1}"),
        ("# Title", "plain"),
        ("{\"é\":1}", ""),
    ] {
        let start = session.text().find(needle).unwrap();
        let range = start..start + needle.len();
        cache.splice(range.clone(), std::iter::repeat_n(None, replacement.len()));
        let changed = session
            .apply_edit(TextEdit {
                range,
                replacement: replacement.into(),
            })
            .unwrap();
        for range in changed {
            cache[range.clone()].fill(None);
            for span in session.highlight(range).unwrap() {
                cache[span.range].fill(Some(span.kind));
            }
        }
        assert_eq!(
            cache,
            labels(&HighlightSession::new("markdown", session.text()).unwrap())
        );
    }
}

#[test]
#[cfg(feature = "markdown")]
fn every_enabled_language_can_be_injected() {
    for language in languages().iter().filter(|l| l.name != "markdown_inline") {
        let source = match language.name {
            "markdown" => "# Heading",
            "html" => "<b>text</b>",
            "css" => "a { color: #fff; }",
            "json" => "{\"a\":1}",
            "yaml" | "toml" => "key = 42",
            _ => "fn main() { return 42; }",
        };
        let text = format!("```{}\n{}\n```\n", language.name, source);
        let start = text.find(source).unwrap();
        let session = HighlightSession::new("markdown", &text).unwrap();
        let mut actual = session.highlight(start..start + source.len()).unwrap();
        for span in &mut actual {
            span.range = span.range.start - start..span.range.end - start;
        }
        let expected = HighlightSession::new(language.name, source)
            .unwrap()
            .highlight_all()
            .unwrap();
        assert_eq!(actual, expected, "{} fence", language.name);
    }
}

#[test]
#[cfg(feature = "markdown")]
fn markdown_without_final_newline_preserves_public_offsets() {
    let mut session = HighlightSession::new("markdown", "# café😀").unwrap();
    assert_eq!(session.text(), "# café😀");
    assert!(
        session
            .highlight_all()
            .unwrap()
            .iter()
            .any(|s| s.kind == dxhighlight::TokenKind::Heading
                && &session.text()[s.range.clone()] == "café😀")
    );
    for replacement in ["\n", "tail", "\r\n", ""] {
        let end = session.text().len();
        let ranges = session
            .apply_edit(dxhighlight::TextEdit {
                range: end..end,
                replacement: replacement.into(),
            })
            .unwrap();
        assert!(ranges.iter().all(|r| r.end <= session.text().len()));
        assert_eq!(
            session.highlight_all().unwrap(),
            HighlightSession::new("markdown", session.text())
                .unwrap()
                .highlight_all()
                .unwrap()
        );
    }
}
