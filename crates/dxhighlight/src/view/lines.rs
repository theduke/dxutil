use crate::HighlightSpan;
use std::ops::Range;

pub(super) struct RenderedLine {
    pub number: usize,
    pub parts: Vec<Part>,
}

pub(super) struct Part {
    pub text: String,
    pub class: &'static str,
    pub mark: Mark,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    None,
    Match,
    Active,
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

/// Find matches as sorted document byte ranges and the index of the active one.
#[derive(Clone, Copy)]
pub(super) struct Marks<'a> {
    pub matches: &'a [Range<usize>],
    pub active: Option<usize>,
}

// Row construction stays independent of DOM APIs, parsing and virtualization.
pub(super) fn render(
    text: &str,
    lines: &[Range<usize>],
    first: usize,
    spans: &[HighlightSpan],
    marks: Marks<'_>,
) -> Vec<RenderedLine> {
    let window_start = lines.first().map_or(0, |line| line.start);
    let offset = marks.matches.partition_point(|m| m.end <= window_start);
    let marks = Marks {
        matches: &marks.matches[offset..],
        active: marks.active.and_then(|active| active.checked_sub(offset)),
    };
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| RenderedLine {
            number: first + i + 1,
            parts: parts(text, line.clone(), spans, marks),
        })
        .collect()
}

fn parts(text: &str, line: Range<usize>, spans: &[HighlightSpan], marks: Marks<'_>) -> Vec<Part> {
    let mut result = Vec::new();
    let mut next = marks.matches.partition_point(|m| m.end <= line.start);
    let mut push = |range: Range<usize>, class: &'static str| {
        let mut pos = range.start;
        while pos < range.end {
            while marks.matches.get(next).is_some_and(|m| m.end <= pos) {
                next += 1;
            }
            let (end, mark) = match marks.matches.get(next) {
                Some(m) if m.start <= pos => (
                    m.end.min(range.end),
                    if marks.active == Some(next) {
                        Mark::Active
                    } else {
                        Mark::Match
                    },
                ),
                Some(m) => (m.start.min(range.end), Mark::None),
                None => (range.end, Mark::None),
            };
            result.push(Part {
                text: text[pos..end].into(),
                class,
                mark,
            });
            pos = end;
        }
    };
    let mut byte = line.start;
    for span in spans
        .iter()
        .skip_while(|s| s.range.end <= line.start)
        .take_while(|s| s.range.start < line.end)
    {
        let start = span.range.start.max(line.start);
        let end = span.range.end.min(line.end);
        if byte < start {
            push(byte..start, "");
        }
        push(start..end, span.kind.class());
        byte = end;
    }
    if byte < line.end {
        push(byte..line.end, "");
    }
    result
}
