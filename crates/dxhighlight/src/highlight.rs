use crate::{
    CodeError, HighlightSession, TokenKind, TokenSpan, language_named, languages::Configuration,
};
use std::{cmp::Reverse, collections::BTreeSet, ops::Range};
use tree_sitter::{Parser, QueryCursor, StreamingIterator, Tree};

/// A non-overlapping, half-open UTF-8 byte span in the original document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub kind: TokenKind,
}

/// Highlight a sequence of lines together, retaining multiline parser context.
/// Returned offsets count Unicode scalar values within each line, excluding a
/// trailing CR (CRLF). There is no arbitrary line limit or renderer dependency.
pub fn highlight_lines(
    language: &str,
    lines: &[impl AsRef<str>],
) -> Result<Vec<Vec<TokenSpan>>, CodeError> {
    let text = lines
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join("\n");
    let session = HighlightSession::new(language, text)?;
    let spans = session.highlight_all()?;
    let mut offset = 0;
    let mut first = 0;
    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        let raw = line.as_ref();
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        while first < spans.len() && spans[first].range.end <= offset {
            first += 1;
        }
        let mut tokens = Vec::new();
        for span in spans[first..]
            .iter()
            .take_while(|s| s.range.start < offset + line.len())
        {
            let start = span.range.start.max(offset) - offset;
            let end = span.range.end.min(offset + line.len()) - offset;
            if start < end {
                tokens.push(TokenSpan {
                    start: line[..start].chars().count() as u32,
                    end: line[..end].chars().count() as u32,
                    class: span.kind.class().into(),
                });
            }
        }
        result.push(tokens);
        offset += raw.len() + 1;
    }
    Ok(result)
}

struct Capture {
    range: Range<usize>,
    kind: Option<TokenKind>,
    pattern: usize,
    layer: usize,
    width: usize,
}

pub(crate) fn tree(
    text: &str,
    tree: &Tree,
    config: &Configuration,
    range: Range<usize>,
    depth: usize,
) -> Result<Vec<HighlightSpan>, CodeError> {
    let mut captures = Vec::new();
    collect(text, tree, config, range, depth, &mut captures)?;
    Ok(flatten(captures))
}

fn collect(
    text: &str,
    tree: &Tree,
    config: &Configuration,
    viewport: Range<usize>,
    depth: usize,
    output: &mut Vec<Capture>,
) -> Result<(), CodeError> {
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(viewport.clone());
    let query = &config.highlights;
    let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());
    while let Some(m) = matches.next() {
        for c in m.captures() {
            let name = query.capture_names()[c.index as usize];
            let kind = TokenKind::capture(name);
            if kind.is_none() && name != "none" {
                continue;
            }
            let start = c.node.start_byte().max(viewport.start);
            let end = c.node.end_byte().min(viewport.end);
            if start < end {
                output.push(Capture {
                    range: start..end,
                    kind,
                    pattern: m.pattern_index,
                    width: c.node.byte_range().len(),
                    layer: depth * 2,
                });
            }
        }
    }
    if depth >= 8 {
        return Ok(());
    }
    for injection in injections(text, tree, config, viewport.clone()) {
        let start = injection.range.start_byte.max(viewport.start);
        let end = injection.range.end_byte.min(viewport.end);
        if start >= end {
            continue;
        }
        // An unavailable injected grammar falls back to plain text, even when
        // the host query marks the entire code block as a string.
        if injection.language != "markdown_inline" {
            output.push(Capture {
                range: start..end,
                kind: None,
                pattern: 0,
                width: injection.range.end_byte - injection.range.start_byte,
                layer: depth * 2 + 1,
            });
        }
        let Some(language) = language_named(&injection.language) else {
            continue;
        };
        let config = language.configuration()?;
        let mut parser = Parser::new();
        parser
            .set_language(&config.language)
            .map_err(|e| CodeError::Parser(e.to_string()))?;
        parser
            .set_included_ranges(&[injection.range])
            .map_err(|e| CodeError::Parser(e.to_string()))?;
        let tree = parser
            .parse(text, None)
            .ok_or_else(|| CodeError::Parser("injected parse cancelled".into()))?;
        collect(text, &tree, &config, start..end, depth + 1, output)?;
    }
    Ok(())
}

struct Injection {
    language: String,
    range: tree_sitter::Range,
}

fn injections(
    text: &str,
    tree: &Tree,
    config: &Configuration,
    viewport: Range<usize>,
) -> Vec<Injection> {
    let query = &config.injections;
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(viewport);
    let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());
    let mut result = Vec::new();
    while let Some(m) = matches.next() {
        let mut language = query
            .property_settings(m.pattern_index)
            .iter()
            .find(|p| p.key.as_ref() == "injection.language")
            .and_then(|p| p.value.as_deref())
            .map(str::to_owned);
        for c in m.captures() {
            if query.capture_names()[c.index as usize] == "injection.language" {
                language = Some(text[c.node.byte_range()].to_owned());
            }
        }
        for c in m.captures() {
            if query.capture_names()[c.index as usize] == "injection.content" {
                result.push(Injection {
                    language: language.clone().unwrap_or_default(),
                    range: c.node.range(),
                });
            }
        }
    }
    result
}

pub(crate) fn injection_ranges(
    text: &str,
    tree: &Tree,
    config: &Configuration,
    range: Range<usize>,
) -> Vec<Range<usize>> {
    injections(text, tree, config, range)
        .into_iter()
        .map(|i| i.range.start_byte..i.range.end_byte)
        .collect()
}

// Sweep boundaries once rather than visiting every byte. Inner captures win;
// ties prefer later query patterns, and injected captures override their host.
fn flatten(captures: Vec<Capture>) -> Vec<HighlightSpan> {
    let mut events = Vec::with_capacity(captures.len() * 2);
    for (i, c) in captures.iter().enumerate() {
        events.push((c.range.start, true, i));
        events.push((c.range.end, false, i));
    }
    events.sort_unstable();
    let mut active: BTreeSet<(usize, Reverse<usize>, usize, usize)> = BTreeSet::new();
    let mut output: Vec<HighlightSpan> = Vec::new();
    let mut previous = 0;
    for (byte, start, i) in events {
        if byte > previous {
            if let Some(&(_, _, _, index)) = active.last()
                && let Some(kind) = captures[index].kind
            {
                if let Some(last) = output
                    .last_mut()
                    .filter(|s| s.kind == kind && s.range.end == previous)
                {
                    last.range.end = byte;
                } else {
                    output.push(HighlightSpan {
                        range: previous..byte,
                        kind,
                    });
                }
            }
            previous = byte;
        }
        let c = &captures[i];
        let key = (c.layer, Reverse(c.width), c.pattern, i);
        if start {
            active.insert(key);
        } else {
            active.remove(&key);
        }
    }
    output
}
