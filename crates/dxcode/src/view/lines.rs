use crate::HighlightSpan;
use std::ops::Range;

pub(super) struct RenderedLine {
    pub number: usize,
    pub parts: Vec<(String, &'static str)>,
}

pub(super) fn index(text: &str) -> Vec<Range<usize>> {
    let mut offset = 0;
    text.split('\n')
        .map(|line| {
            let start = offset;
            offset += line.len() + 1;
            start..start + line.strip_suffix('\r').unwrap_or(line).len()
        })
        .collect()
}

// Row construction stays independent of DOM APIs, parsing and virtualization.
pub(super) fn render(
    text: &str,
    lines: &[Range<usize>],
    first: usize,
    spans: &[HighlightSpan],
) -> Vec<RenderedLine> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| RenderedLine {
            number: first + i + 1,
            parts: parts(text, line.clone(), spans),
        })
        .collect()
}

fn parts(text: &str, line: Range<usize>, spans: &[HighlightSpan]) -> Vec<(String, &'static str)> {
    let mut result = Vec::new();
    let mut byte = line.start;
    for span in spans
        .iter()
        .skip_while(|s| s.range.end <= line.start)
        .take_while(|s| s.range.start < line.end)
    {
        let start = span.range.start.max(line.start);
        let end = span.range.end.min(line.end);
        if byte < start {
            result.push((text[byte..start].into(), ""));
        }
        result.push((text[start..end].into(), span.kind.class()));
        byte = end;
    }
    if byte < line.end {
        result.push((text[byte..line.end].into(), ""));
    }
    result
}
