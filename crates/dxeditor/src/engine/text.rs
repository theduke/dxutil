//! Inline editing works in Unicode scalar/atom units. DOM UTF-16 offsets never enter here.
use crate::{ComponentMark, ComponentNode};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DeleteUnit {
    Grapheme,
    Word,
    SoftLine,
    HardLine,
    Selection,
    TargetRange(super::selection::TextRange),
}

pub fn len(nodes: &[ComponentNode]) -> usize {
    nodes.iter().map(node_len).sum()
}
pub fn node_len(node: &ComponentNode) -> usize {
    node.text.as_ref().map(|s| s.chars().count()).unwrap_or(1)
}

/// Split inline runs without losing marks or bisecting a scalar value.
pub fn split(nodes: &[ComponentNode], offset: usize) -> (Vec<ComponentNode>, Vec<ComponentNode>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut remaining = offset;
    for node in nodes {
        let length = node_len(node);
        if remaining >= length {
            left.push(node.clone());
            remaining -= length;
        } else if remaining == 0 {
            right.push(node.clone());
        } else if let Some(text) = &node.text {
            let byte = text
                .char_indices()
                .nth(remaining)
                .map(|(i, _)| i)
                .unwrap_or(text.len());
            let mut a = node.clone();
            a.text = Some(text[..byte].to_owned());
            left.push(a);
            let mut b = node.clone();
            b.text = Some(text[byte..].to_owned());
            right.push(b);
            remaining = 0;
        } else {
            right.push(node.clone());
            remaining = 0;
        }
    }
    (left, right)
}
pub fn slice(nodes: &[ComponentNode], start: usize, end: usize) -> Vec<ComponentNode> {
    let (_, tail) = split(nodes, start);
    split(&tail, end.saturating_sub(start)).0
}
pub fn replace(
    nodes: &[ComponentNode],
    start: usize,
    end: usize,
    inserted: Vec<ComponentNode>,
) -> Vec<ComponentNode> {
    let (mut head, _) = split(nodes, start);
    let (_, tail) = split(nodes, end);
    head.extend(inserted);
    head.extend(tail);
    compact(head)
}
pub fn compact(nodes: Vec<ComponentNode>) -> Vec<ComponentNode> {
    let mut result: Vec<ComponentNode> = Vec::new();
    for node in nodes {
        if node.text.as_ref().is_some_and(String::is_empty) {
            continue;
        }
        if let Some(last) = result.last_mut() {
            if node.kind.0 == "text"
                && last.kind.0 == "text"
                && node.marks == last.marks
                && node.attrs == last.attrs
            {
                last.text
                    .get_or_insert_default()
                    .push_str(node.text.as_deref().unwrap_or_default());
                continue;
            }
        }
        result.push(node);
    }
    result
}
pub fn plain(nodes: &[ComponentNode]) -> String {
    nodes
        .iter()
        .map(|n| {
            n.text.clone().unwrap_or_else(|| {
                if n.kind.0 == "hard_break" {
                    "\n".into()
                } else {
                    "\u{fffc}".into()
                }
            })
        })
        .collect()
}
pub fn marks_at(nodes: &[ComponentNode], offset: usize) -> Vec<ComponentMark> {
    let mut cursor = 0;
    for node in nodes {
        cursor += node_len(node);
        if cursor >= offset && node.kind.0 == "text" {
            return node.marks.clone();
        }
    }
    Vec::new()
}
pub fn deletion_boundary(
    nodes: &[ComponentNode],
    offset: usize,
    direction: Direction,
    unit: &DeleteUnit,
) -> usize {
    let source = plain(nodes);
    let chars = source.chars().count();
    let offset = offset.min(chars);
    let byte = source
        .char_indices()
        .nth(offset)
        .map(|(i, _)| i)
        .unwrap_or(source.len());
    match unit {
        DeleteUnit::Grapheme => {
            let boundaries: Vec<usize> = source
                .grapheme_indices(true)
                .map(|(b, _)| source[..b].chars().count())
                .chain(std::iter::once(chars))
                .collect();
            match direction {
                Direction::Backward => boundaries
                    .into_iter()
                    .filter(|&b| b < offset)
                    .next_back()
                    .unwrap_or(0),
                Direction::Forward => boundaries
                    .into_iter()
                    .find(|&b| b > offset)
                    .unwrap_or(chars),
            }
        }
        DeleteUnit::Word => match direction {
            Direction::Backward => {
                let prefix = &source[..byte];
                let trimmed = prefix.trim_end_matches(char::is_whitespace);
                trimmed
                    .split_word_bound_indices()
                    .rev()
                    .find(|(_, w)| w.chars().any(char::is_alphanumeric))
                    .map(|(b, _)| trimmed[..b].chars().count())
                    .unwrap_or(0)
            }
            Direction::Forward => {
                let suffix = &source[byte..];
                let skip = suffix.len() - suffix.trim_start_matches(char::is_whitespace).len();
                let trimmed = &suffix[skip..];
                offset
                    + suffix[..skip].chars().count()
                    + trimmed
                        .split_word_bounds()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .count()
            }
        },
        DeleteUnit::SoftLine | DeleteUnit::HardLine => match direction {
            Direction::Backward => source[..byte]
                .rfind('\n')
                .map(|b| source[..b + 1].chars().count())
                .unwrap_or(0),
            Direction::Forward => source[byte..]
                .find('\n')
                .map(|b| offset + source[byte..byte + b].chars().count())
                .unwrap_or(chars),
        },
        _ => offset,
    }
}
pub fn change_mark(
    nodes: &[ComponentNode],
    start: usize,
    end: usize,
    mark: &ComponentMark,
    remove: bool,
) -> Vec<ComponentNode> {
    let (mut head, rest) = split(nodes, start);
    let (mut middle, tail) = split(&rest, end.saturating_sub(start));
    for node in &mut middle {
        if node.kind.0 == "text" {
            node.marks.retain(|m| {
                m.kind != mark.kind
                    && !(mark.kind.0 == "link" && m.kind.0 == "entity_link")
                    && !(mark.kind.0 == "entity_link" && m.kind.0 == "link")
            });
            if !remove {
                node.marks.push(mark.clone());
            }
        }
    }
    head.extend(middle);
    head.extend(tail);
    compact(head)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grapheme_and_scalar_offsets() {
        let n = vec![ComponentNode::text("a👩‍👩‍👦é")];
        let end = len(&n);
        assert_eq!(
            deletion_boundary(&n, end - 1, Direction::Backward, &DeleteUnit::Grapheme),
            1
        );
        let (a, b) = split(&n, 2);
        assert_eq!(plain(&a), "a👩");
        assert_eq!(plain(&b), "‍👩‍👦é");
    }
    #[test]
    fn runs_keep_marks() {
        let mut n = ComponentNode::text("abc");
        n.marks.push(ComponentMark::new("bold"));
        let r = replace(&[n], 1, 2, vec![ComponentNode::text("X")]);
        assert_eq!(r.len(), 3);
        assert_eq!(plain(&r), "aXc");
        assert_eq!(r[2].marks[0].kind.0, "bold");
    }
}
