use crate::{
    CodeError, HighlightSpan, Language, highlight, language_named, languages::Configuration,
};
use std::{ops::Range, sync::Arc};
use tree_sitter::{InputEdit, Parser, Point, Tree};

/// Replace a half-open UTF-8 byte range. Boundaries must be char boundaries.
#[derive(Clone, Debug)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub replacement: String,
}

/// Incremental parser state. Unknown or disabled languages remain plain text.
/// Query configurations are cached per thread; sessions can move between threads.
pub struct HighlightSession {
    text: String,
    parse_text: Option<String>,
    language: Option<&'static Language>,
    config: Option<Arc<Configuration>>,
    parser: Parser,
    tree: Option<Tree>,
}

impl HighlightSession {
    pub fn new(language: &str, text: impl Into<String>) -> Result<Self, CodeError> {
        let mut session = Self::prepare(language, text.into())?;
        if session.config.is_some() {
            let source = session.parse_text.as_deref().unwrap_or(&session.text);
            session.tree = Some(
                session
                    .parser
                    .parse(source, None)
                    .ok_or_else(|| CodeError::Parser("parse cancelled".into()))?,
            );
        }
        Ok(session)
    }

    pub(crate) fn prepare(language: &str, text: String) -> Result<Self, CodeError> {
        let language = language_named(language);
        let config = language.map(Language::configuration).transpose()?;
        let parse_text = markdown_source(language, &text);
        let mut parser = Parser::new();
        if let Some(config) = &config {
            parser
                .set_language(&config.language)
                .map_err(|e| CodeError::Parser(e.to_string()))?;
        }
        Ok(Self {
            text,
            parse_text,
            language,
            config,
            parser,
            tree: None,
        })
    }

    /// Resume a bounded amount of initial parsing between browser event turns.
    #[cfg(all(feature = "viewer", target_arch = "wasm32"))]
    pub(crate) fn parse_step(&mut self) -> bool {
        if self.config.is_none() || self.tree.is_some() {
            return true;
        }
        let text = self.parse_text.as_deref().unwrap_or(&self.text).as_bytes();
        let mut progress = 0;
        let mut budget = |_: &tree_sitter::ParseState| {
            progress += 1;
            if progress >= 32 {
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        };
        self.tree = self.parser.parse_with_options(
            &mut |byte, _| text.get(byte..).unwrap_or_default(),
            None,
            Some(tree_sitter::ParseOptions::new().progress_callback(&mut budget)),
        );
        self.tree.is_some()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn language(&self) -> Option<&'static Language> {
        self.language
    }

    /// Parse using the edited old tree, returning changed ranges in the NEW text.
    /// Includes the edited range even when the syntax tree's shape is unchanged.
    /// For injected languages, invalidate the enclosing injection too: their own
    /// parser state isn't retained, and edits can change language resolution.
    pub fn apply_edit(&mut self, edit: TextEdit) -> Result<Vec<Range<usize>>, CodeError> {
        self.validate_range(&edit.range)?;
        let start_position = point(&self.text, edit.range.start);
        let old_end_position = point(&self.text, edit.range.end);
        let mut next = self.text.clone();
        next.replace_range(edit.range.clone(), &edit.replacement);
        let new_end_byte = edit.range.start + edit.replacement.len();
        let input = InputEdit {
            start_byte: edit.range.start,
            old_end_byte: edit.range.end,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position: point(&next, new_end_byte),
        };
        let parse_text = markdown_source(self.language, &next);
        let source = parse_text.as_deref().unwrap_or(&next);
        let mut ranges = Vec::new();
        ranges.push(edit.range.start..new_end_byte);
        if let Some(old) = &mut self.tree {
            old.edit(&input);
            let tree = self
                .parser
                .parse(source, Some(old))
                .ok_or_else(|| CodeError::Parser("parse cancelled".into()))?;
            ranges.extend(old.changed_ranges(&tree).map(|r| r.start_byte..r.end_byte));
            // Query captures may depend on ancestors (e.g. a renamed function).
            // Widen to named syntax ancestors spanning the edit, up to a line.
            let end = new_end_byte
                .max(edit.range.start.saturating_add(1))
                .min(next.len());
            if let Some(node) = tree
                .root_node()
                .named_descendant_for_byte_range(edit.range.start, end)
            {
                let mut node = node;
                while let Some(parent) = node.parent() {
                    if parent.kind() == "document"
                        || parent.kind() == "program"
                        || parent.kind() == "source_file"
                    {
                        break;
                    }
                    node = parent;
                    if node.start_position().row < node.end_position().row {
                        break;
                    }
                }
                ranges.push(node.byte_range());
            }
            ranges.extend(highlight::injection_ranges(
                source,
                &tree,
                self.config.as_ref().unwrap(),
                edit.range.start..end,
            ));
            self.tree = Some(tree);
        }
        self.text = next;
        self.parse_text = parse_text;
        for range in &mut ranges {
            range.start = range.start.min(self.text.len());
            range.end = range.end.min(self.text.len());
        }
        ranges.sort_by_key(|r| r.start);
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut().filter(|last| last.end >= range.start) {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        Ok(merged)
    }

    /// Highlight only captures overlapping this UTF-8 byte viewport. Returned
    /// spans are clipped to it. Use changed ranges from `apply_edit` to refresh
    /// cached output; offsets after an edit must be shifted by the byte delta.
    pub fn highlight(&self, range: Range<usize>) -> Result<Vec<HighlightSpan>, CodeError> {
        self.validate_range(&range)?;
        match (&self.tree, &self.config) {
            (Some(tree), Some(config)) if !range.is_empty() => highlight::tree(
                self.parse_text.as_deref().unwrap_or(&self.text),
                tree,
                config,
                range,
                0,
            ),
            _ => Ok(Vec::new()),
        }
    }
    pub fn highlight_all(&self) -> Result<Vec<HighlightSpan>, CodeError> {
        self.highlight(0..self.text.len())
    }

    fn validate_range(&self, range: &Range<usize>) -> Result<(), CodeError> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            Err(CodeError::InvalidRange)
        } else {
            Ok(())
        }
    }
}

fn point(text: &str, byte: usize) -> Point {
    let prefix = &text[..byte];
    Point {
        row: prefix.bytes().filter(|b| *b == b'\n').count(),
        column: prefix.rfind('\n').map_or(byte, |i| byte - i - 1),
    }
}

// The Markdown block scanner rejects some unterminated final lines. Keep one
// private LF in parser input; public text, edits and output never include it.
// Always append it, so incremental edits preserve a consistent sentinel.
fn markdown_source(language: Option<&Language>, text: &str) -> Option<String> {
    language
        .filter(|l| l.name == "markdown")
        .map(|_| format!("{text}\n"))
}
