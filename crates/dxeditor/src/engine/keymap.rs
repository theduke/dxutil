//! Shortcut precedence is kept here so every adapter uses the same behavior.
use super::{
    Command, EditorEngine, EditorSelection, InlineContainerRef, Profile,
    commands::TableCommand,
    model,
    text::{DeleteUnit, Direction},
};
use crate::{AttributeMap, ComponentMark};
use serde_json::json;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyChord {
    pub key: String,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
    pub composing: bool,
}
impl KeyChord {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ..Self::default()
        }
    }
    pub fn modified(&self, mac: bool) -> bool {
        if mac { self.meta } else { self.ctrl }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    Command(Command),
    Submit { alternate: bool },
    BackgroundSubmit,
    HistoryPrevious,
    HistoryNext,
    Stash,
    PlainPaste,
    MoveToTextEdge { end: bool },
}
fn command(command: Command) -> Option<KeyAction> {
    Some(KeyAction::Command(command))
}
pub fn resolve(engine: &EditorEngine, key: &KeyChord) -> Option<KeyAction> {
    if key.composing || engine.composing || key.key == "Process" {
        return None;
    }
    let modified = key.modified(engine.options.mac);
    let bare = !modified && !key.ctrl && !key.meta && !key.alt;
    let lower = key.key.to_lowercase();
    if engine.suggestion.active.is_some() && bare {
        match key.key.as_str() {
            "ArrowDown" => return command(Command::SuggestionNext(1)),
            "ArrowUp" => return command(Command::SuggestionNext(-1)),
            "Home" => return command(Command::SuggestionFirst),
            "End" => return command(Command::SuggestionLast),
            "Escape" => return command(Command::DismissSuggestion),
            "Enter" if !key.shift && engine.suggestion.selected().is_some() => {
                return command(Command::AcceptSuggestion);
            }
            "Tab"
                if engine.profile == Profile::Composer
                    && !key.shift
                    && engine.suggestion.selected().is_some() =>
            {
                return command(Command::AcceptSuggestion);
            }
            _ => {}
        }
    }
    if engine.open_menu.is_some() && key.key == "Escape" {
        return command(Command::CloseMenu);
    }
    if engine.profile == Profile::Composer {
        if modified && key.shift && lower == "v" {
            return Some(KeyAction::PlainPaste);
        }
        if modified && key.shift && lower == "s" {
            return Some(KeyAction::Stash);
        }
        if key.key == "Enter" && modified && key.alt && !key.shift {
            return Some(KeyAction::BackgroundSubmit);
        }
        if key.key == "Enter"
            && !key.shift
            && !key.alt
            && (engine.composer_submit_on_enter || modified)
        {
            return Some(KeyAction::Submit {
                alternate: modified,
            });
        }
        if bare
            && !key.shift
            && let EditorSelection::Text(range) = &engine.selection
            && range.is_collapsed()
        {
            let order = &engine.index.order;
            if let InlineContainerRef::Block(uid) = &range.head.container {
                if key.key == "ArrowUp" && range.head.offset == 0 && order.first() == Some(uid) {
                    return Some(KeyAction::HistoryPrevious);
                }
                if key.key == "ArrowDown"
                    && order.last() == Some(uid)
                    && engine
                        .inline(&range.head.container)
                        .is_some_and(|n| range.head.offset == super::text::len(n))
                {
                    return Some(KeyAction::HistoryNext);
                }
            }
        }
    }
    if bare
        && !key.shift
        && let EditorSelection::Node(uid) = &engine.selection
    {
        match key.key.as_str() {
            "ArrowDown" => return command(Command::FocusAfterBlock(uid.clone())),
            "ArrowUp" => return command(Command::FocusBeforeBlock(uid.clone())),
            _ => {}
        }
    }
    if matches!(
        engine.selection,
        EditorSelection::Blocks { .. } | EditorSelection::Node(_)
    ) && bare
    {
        match key.key.as_str() {
            "Enter" => return command(Command::EditSelected),
            "Backspace" | "Delete" => return command(Command::DeleteBlocks),
            "ArrowUp" => {
                return command(Command::MoveSelection {
                    direction: Direction::Backward,
                    extend: key.shift,
                });
            }
            "ArrowDown" => {
                return command(Command::MoveSelection {
                    direction: Direction::Forward,
                    extend: key.shift,
                });
            }
            _ if key.key.chars().count() == 1 => {
                return command(Command::InsertText(key.key.clone()));
            }
            _ => {}
        }
    }
    if modified {
        match (lower.as_str(), key.shift, key.alt) {
            ("z" | "я", false, false) => return command(Command::Undo),
            ("z" | "я", true, false) | ("y", false, false) => return command(Command::Redo),
            ("a", false, false) => return command(Command::SelectAll),
            ("k", false, false) => return command(Command::OpenMenu("link".into())),
            ("/", false, false) if engine.profile.has_blocks() => {
                return command(Command::OpenMenu("block".into()));
            }
            ("d", false, false) if engine.profile.has_blocks() => {
                return command(Command::Duplicate);
            }
            ("arrowup", true, false) => return command(Command::MoveUp),
            ("arrowdown", true, false) => return command(Command::MoveDown),
            ("b", false, false) => return command(Command::ToggleMark(ComponentMark::new("bold"))),
            ("i", false, false) => {
                return command(Command::ToggleMark(ComponentMark::new("italic")));
            }
            ("u", false, false) => {
                return command(Command::ToggleMark(ComponentMark::new("underline")));
            }
            ("e", false, false) => return command(Command::ToggleMark(ComponentMark::new("code"))),
            ("s", true, false) => {
                return command(Command::ToggleMark(ComponentMark::new("strike")));
            }
            ("h", true, false) => {
                let mut mark = ComponentMark::new("highlight");
                mark.attrs
                    .insert("color".into(), json!(engine.last_highlight));
                return command(Command::ToggleMark(mark));
            }
            ("7", true, false) | ("&", true, false) => {
                return turn_into("numbered_list_item", AttributeMap::new());
            }
            ("8", true, false) | ("*", true, false) => {
                return turn_into("bulleted_list_item", AttributeMap::new());
            }
            ("9", true, false) | ("(", true, false) => {
                return turn_into(
                    "todo",
                    serde_json::from_value(json!({"checked":false})).unwrap(),
                );
            }
            ("0", false, true) => return turn_into("paragraph", AttributeMap::new()),
            ("1" | "2" | "3" | "4" | "5" | "6", false, true) => {
                return turn_into(
                    "heading",
                    serde_json::from_value(json!({"level":lower.parse::<u8>().unwrap_or(1)}))
                        .unwrap(),
                );
            }
            ("c", false, true) => return turn_into("code_block", AttributeMap::new()),
            ("b", true, false) => return turn_into("quote", AttributeMap::new()),
            ("enter", false, false) => {
                let uid = super::commands::blocks::current_uid(engine)?;
                let kind = &engine.content(&uid)?.kind.0;
                return match kind.as_str() {
                    "code_block" => command(Command::ExitCode),
                    "quote" => command(Command::ExitQuote),
                    "todo" => command(Command::ToggleChecked),
                    "toggle" => command(Command::ToggleCollapsed(uid)),
                    _ => command(Command::HardBreak),
                };
            }
            ("backspace", _, false) => {
                return command(Command::Delete {
                    direction: Direction::Backward,
                    unit: if engine.options.mac {
                        DeleteUnit::HardLine
                    } else {
                        DeleteUnit::Word
                    },
                });
            }
            ("delete", _, false) => {
                return command(Command::Delete {
                    direction: Direction::Forward,
                    unit: if engine.options.mac {
                        DeleteUnit::HardLine
                    } else {
                        DeleteUnit::Word
                    },
                });
            }
            _ => {}
        }
    }
    if engine.options.mac && key.ctrl && !key.meta && !key.alt {
        match lower.as_str() {
            "h" => {
                return command(Command::Delete {
                    direction: Direction::Backward,
                    unit: DeleteUnit::Grapheme,
                });
            }
            "d" => {
                return command(Command::Delete {
                    direction: Direction::Forward,
                    unit: DeleteUnit::Grapheme,
                });
            }
            "a" => return Some(KeyAction::MoveToTextEdge { end: false }),
            "e" => return Some(KeyAction::MoveToTextEdge { end: true }),
            _ => {}
        }
    }
    if engine.options.mac && key.alt && !modified {
        match key.key.as_str() {
            "Backspace" => {
                return command(Command::Delete {
                    direction: Direction::Backward,
                    unit: DeleteUnit::Word,
                });
            }
            "Delete" => {
                return command(Command::Delete {
                    direction: Direction::Forward,
                    unit: DeleteUnit::Word,
                });
            }
            "d" => {
                return command(Command::Delete {
                    direction: Direction::Forward,
                    unit: DeleteUnit::Word,
                });
            }
            _ => {}
        }
    }
    if bare {
        match key.key.as_str() {
            "Tab" if engine.profile != Profile::Composer => {
                let cell = matches!(&engine.selection, EditorSelection::Cells { .. })
                    || matches!(&engine.selection,EditorSelection::Text(range) if matches!(range.head.container,InlineContainerRef::Cell{..}));
                return command(if cell {
                    Command::Table(TableCommand::Navigate {
                        backwards: key.shift,
                    })
                } else if key.shift {
                    Command::Outdent
                } else {
                    Command::Indent
                });
            }
            "Escape" if engine.profile.has_blocks() => return command(Command::SelectBlocks),
            "ArrowDown" if !key.shift => {
                if let Some(range) = engine.selection.text_range().filter(|r| r.is_collapsed())
                    && let InlineContainerRef::Block(uid) = &range.head.container
                    && engine.index.order.last() == Some(uid)
                    && engine.content(uid).is_some_and(|n| {
                        n.kind.0 == "quote" && range.head.offset == super::text::len(&n.content)
                    })
                    && !model::children(&engine.doc, Some(uid)).is_some_and(|n| !n.is_empty())
                {
                    return command(Command::ExitTrailingQuote);
                }
            }
            // Enter and deletion normally arrive through beforeinput, including on Android.
            _ => {}
        }
    }
    None
}
fn turn_into(kind: &str, attrs: AttributeMap) -> Option<KeyAction> {
    command(Command::TurnInto {
        kind: kind.into(),
        attrs,
    })
}

pub fn host_paragraph_shortcut_eligible(engine: &EditorEngine) -> bool {
    if engine.profile != Profile::GraphNode || engine.composing || !engine.selection.is_collapsed()
    {
        return false;
    }
    let Some(blocks) = model::children(&engine.doc, None) else {
        return false;
    };
    blocks.len() == 1 && blocks[0].content.len() == 1 && blocks[0].content[0].kind.0 == "paragraph"
}
