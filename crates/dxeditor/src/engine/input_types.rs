//! Input Events Level 2 translation; the DOM adapter supplies target ranges.
use super::{
    Command, EngineInput, TextRange,
    text::{DeleteUnit, Direction},
};
use crate::{AttributeMap, ComponentMark};
#[derive(Clone, Debug, PartialEq)]
pub enum InputDisposition {
    AllowComposition,
    HandledElsewhere,
    Input(EngineInput),
    Command(Command),
    Ignore,
}
pub fn translate(
    input_type: &str,
    data: Option<&str>,
    target: Option<TextRange>,
    shift: bool,
) -> InputDisposition {
    use InputDisposition::*;
    match input_type {
        "insertCompositionText"
        | "deleteCompositionText"
        | "deleteByComposition"
        | "insertFromComposition" => AllowComposition,
        "insertFromPaste"
        | "insertFromPasteAsQuotation"
        | "insertFromDrop"
        | "deleteByCut"
        | "deleteByDrag" => HandledElsewhere,
        "insertText" | "insertFromYank" | "insertTranspose" => Input(EngineInput::InsertText {
            text: data.unwrap_or_default().into(),
        }),
        "insertReplacementText" => match target {
            Some(range) => Input(EngineInput::ReplaceRange {
                range,
                text: data.unwrap_or_default().into(),
            }),
            None => Input(EngineInput::InsertText {
                text: data.unwrap_or_default().into(),
            }),
        },
        "insertParagraph" => Input(if shift {
            EngineInput::InsertLineBreak
        } else {
            EngineInput::InsertParagraph
        }),
        "insertLineBreak" => Input(EngineInput::InsertLineBreak),
        "deleteContentBackward" => Input(EngineInput::Delete {
            direction: Direction::Backward,
            unit: DeleteUnit::Grapheme,
        }),
        "deleteContentForward" => Input(EngineInput::Delete {
            direction: Direction::Forward,
            unit: DeleteUnit::Grapheme,
        }),
        "deleteContent" => Input(EngineInput::Delete {
            direction: Direction::Backward,
            unit: DeleteUnit::Selection,
        }),
        "deleteWordBackward"
        | "deleteWordForward"
        | "deleteSoftLineBackward"
        | "deleteSoftLineForward"
        | "deleteEntireSoftLine"
        | "deleteHardLineBackward"
        | "deleteHardLineForward" => {
            let direction = if input_type.ends_with("Forward") {
                Direction::Forward
            } else {
                Direction::Backward
            };
            let unit = target.map(DeleteUnit::TargetRange).unwrap_or_else(|| {
                if input_type.starts_with("deleteWord") {
                    DeleteUnit::Word
                } else if input_type.starts_with("deleteHard") {
                    DeleteUnit::HardLine
                } else {
                    DeleteUnit::SoftLine
                }
            });
            Input(EngineInput::Delete { direction, unit })
        }
        "historyUndo" => Command(super::Command::Undo),
        "historyRedo" => Command(super::Command::Redo),
        "formatBold" | "formatItalic" | "formatUnderline" | "formatStrikeThrough" => Command(
            super::Command::ToggleMark(ComponentMark::new(match input_type {
                "formatBold" => "bold",
                "formatItalic" => "italic",
                "formatUnderline" => "underline",
                _ => "strike",
            })),
        ),
        "formatRemove" => Command(super::Command::ClearMarks),
        "insertLink" => {
            if let Some(href) = data.filter(|url| crate::is_safe_url(url)) {
                let mut mark = ComponentMark::new("link");
                mark.attrs.insert("href".into(), serde_json::json!(href));
                Command(super::Command::SetMark(mark))
            } else {
                Ignore
            }
        }
        "formatIndent" => Command(super::Command::Indent),
        "formatOutdent" => Command(super::Command::Outdent),
        "insertOrderedList" | "insertUnorderedList" => Command(super::Command::TurnInto {
            kind: if input_type == "insertOrderedList" {
                "numbered_list_item"
            } else {
                "bulleted_list_item"
            }
            .into(),
            attrs: AttributeMap::new(),
        }),
        "insertHorizontalRule" => Command(super::Command::InsertBlocks {
            at: crate::InsertAt::Selection,
            nodes: vec![super::model::block_node(crate::ComponentNode::new(
                "divider",
            ))],
        }),
        _ => Ignore,
    }
}
