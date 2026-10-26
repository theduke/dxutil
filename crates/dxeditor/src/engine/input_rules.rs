use super::{
    EditorEngine, EditorSelection, InlineContainerRef, Op, Transaction, TxKind, model, text,
};
use crate::{AttributeMap, ComponentMark};
use serde_json::{Value, json};

/// Applies the first matching rule after a text input, with its own history entry.
pub fn transaction(engine: &EditorEngine, enter: bool) -> Option<Transaction> {
    if engine.profile == super::Profile::Composer || engine.composing || engine.options.read_only {
        return None;
    }
    let Some(range) = engine
        .selection
        .text_range()
        .filter(|r| r.is_collapsed())
        .cloned()
    else {
        return None;
    };
    let InlineContainerRef::Block(uid) = &range.head.container else {
        return None;
    };
    let uid = uid.clone();
    let Some(before) = engine.content(&uid).cloned() else {
        return None;
    };
    let mut after = before.clone();
    let prefix = text::plain(&text::slice(&before.content, 0, range.head.offset));
    if before.kind.0 == "paragraph" {
        let block_rule = block_rule(&prefix, enter);
        if let Some((kind, attrs)) = block_rule.filter(|(kind, _)| engine.schema.has_kind(kind)) {
            if kind == "divider"
                && model::children(&engine.doc, Some(&uid)).is_some_and(|n| !n.is_empty())
            {
                return None;
            }
            after.kind = kind.into();
            after.attrs = attrs;
            after.content = text::split(&before.content, range.head.offset).1;
            let mut ops = vec![Op::SetContent {
                uid: uid.clone(),
                before,
                after,
            }];
            let selection = if kind == "divider" {
                let Some((parent, index)) = engine.location(&uid) else {
                    return None;
                };
                let node = model::paragraph();
                let next = node.id.as_ref().unwrap().0.clone();
                ops.push(Op::InsertBlocks {
                    parent,
                    index: index + 1,
                    blocks: vec![node],
                });
                EditorSelection::caret(InlineContainerRef::Block(next), 0)
            } else {
                EditorSelection::caret(range.head.container.clone(), 0)
            };
            return prepared(engine, ops, selection);
        }
    }
    if enter || before.kind.0 == "code_block" {
        return None;
    }
    for (delimiter, mark) in [
        ("**", "bold"),
        ("__", "bold"),
        ("~~", "strike"),
        ("`", "code"),
        ("*", "italic"),
        ("_", "italic"),
    ] {
        if !engine.profile.allows_mark(mark) || !engine.schema.allows_mark(mark) {
            continue;
        }
        let Some(body) = prefix.strip_suffix(delimiter) else {
            continue;
        };
        let Some(start_byte) = body.rfind(delimiter) else {
            continue;
        };
        if start_byte > 0
            && !body[..start_byte]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let value = &body[start_byte + delimiter.len()..];
        if value.is_empty()
            || value.starts_with(char::is_whitespace)
            || value.ends_with(char::is_whitespace)
        {
            continue;
        }
        let start = body[..start_byte].chars().count();
        let delimiter_len = delimiter.chars().count();
        let mut inside = text::slice(
            &before.content,
            start + delimiter_len,
            range.head.offset - delimiter_len,
        );
        for node in &mut inside {
            if node.kind.0 == "text" && !node.marks.iter().any(|m| m.kind.0 == mark) {
                node.marks.push(ComponentMark::new(mark));
            }
        }
        let caret = start + text::len(&inside);
        after.content = text::replace(&before.content, start, range.head.offset, inside);
        return prepared(
            engine,
            vec![Op::SetContent { uid, before, after }],
            EditorSelection::caret(range.head.container.clone(), caret),
        );
    }
    // Whitespace terminates an autolink token. Reject unsafe URI schemes.
    if prefix.chars().next_back().is_some_and(char::is_whitespace)
        && engine.schema.allows_mark("link")
    {
        let token = prefix
            .trim_end()
            .split_whitespace()
            .next_back()
            .unwrap_or_default();
        if !token.is_empty() {
            let href = if token.starts_with("www.") {
                format!("https://{token}")
            } else {
                token.into()
            };
            if (href.starts_with("https://")
                || href.starts_with("http://")
                || href.starts_with("mailto:"))
                && crate::is_safe_url(&href)
            {
                let end = prefix.trim_end().chars().count();
                let start = end - token.chars().count();
                let mut nodes = text::slice(&before.content, start, end);
                let mut mark = ComponentMark::new("link");
                mark.attrs.insert("href".into(), json!(href));
                for node in &mut nodes {
                    node.marks.retain(|m| m.kind.0 != "link");
                    node.marks.push(mark.clone());
                }
                after.content = text::replace(&before.content, start, end, nodes);
                if after != before {
                    return prepared(
                        engine,
                        vec![Op::SetContent { uid, before, after }],
                        engine.selection.clone(),
                    );
                }
            }
        }
    }
    None
}
fn prepared(
    engine: &EditorEngine,
    ops: Vec<Op>,
    selection: EditorSelection,
) -> Option<Transaction> {
    Some(Transaction::new(
        ops,
        engine.selection.clone(),
        selection,
        TxKind::InputRule,
    ))
}
pub fn apply(engine: &mut EditorEngine, enter: bool) -> Vec<super::EngineOutput> {
    let Some(tx) = transaction(engine, enter) else {
        return Vec::new();
    };
    apply_transaction(engine, tx)
}
pub fn apply_transaction(engine: &mut EditorEngine, tx: Transaction) -> Vec<super::EngineOutput> {
    engine.history.close_group();
    match engine.transact(tx) {
        Ok(out) => {
            engine.last_input_rule = true;
            engine.history.close_group();
            out
        }
        Err(error) => vec![super::error_output(error)],
    }
}

fn attrs(value: Value) -> AttributeMap {
    if let Value::Object(attrs) = value {
        attrs
    } else {
        AttributeMap::new()
    }
}
pub fn block_rule(prefix: &str, enter: bool) -> Option<(&'static str, AttributeMap)> {
    if let Some(language) = prefix.strip_prefix("```") {
        let language = if enter {
            language
        } else {
            language.strip_suffix(' ')?
        };
        if language
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '+' | '-'))
        {
            let mut attributes = attrs(json!({"wrap":false}));
            if !language.is_empty() {
                attributes.insert("language".into(), json!(language));
            }
            return Some(("code_block", attributes));
        }
    }
    if enter {
        return None;
    }
    if prefix == "---" {
        return Some(("divider", AttributeMap::new()));
    }
    let trimmed = prefix.strip_suffix(' ')?;
    if !trimmed.is_empty() && trimmed.len() <= 6 && trimmed.chars().all(|c| c == '#') {
        return Some(("heading", attrs(json!({"level":trimmed.len()}))));
    }
    match trimmed {
        "-" | "*" | "+" => return Some(("bulleted_list_item", AttributeMap::new())),
        "[]" | "[ ]" => return Some(("todo", attrs(json!({"checked":false})))),
        "[x]" | "[X]" => return Some(("todo", attrs(json!({"checked":true})))),
        ">" => return Some(("quote", AttributeMap::new())),
        _ => {}
    }
    if let Some(number) = trimmed.strip_suffix('.')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && let Ok(start) = number.parse::<u64>()
    {
        return Some(("numbered_list_item", attrs(json!({"start":start}))));
    }
    None
}
