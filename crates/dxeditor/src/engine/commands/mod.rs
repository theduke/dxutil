//! Commands shared by browser input, contextual surfaces and host handles.
pub mod blocks;
pub mod islands;
pub mod media;
pub mod tables;
use super::{
    EditorEngine, EngineOutput,
    selection::{EditorSelection, TextRange, Uid},
    text::{DeleteUnit, Direction},
};
use crate::{AttributeMap, ComponentMark, ComponentNode, InsertAt};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    InsertText(String),
    InsertInline(Vec<ComponentNode>),
    ReplaceRange {
        range: TextRange,
        text: String,
    },
    Delete {
        direction: Direction,
        unit: DeleteUnit,
    },
    ClearMarks,
    HardBreak,
    SplitBlock,
    ToggleMark(ComponentMark),
    SetMark(ComponentMark),
    UnsetMark(String),
    Undo,
    Redo,
    Indent,
    Outdent,
    MoveUp,
    MoveDown,
    Duplicate,
    TurnInto {
        kind: String,
        attrs: AttributeMap,
    },
    SetColor {
        key: String,
        color: Option<String>,
    },
    ToggleChecked,
    DeleteBlocks,
    InsertBlocks {
        at: InsertAt,
        nodes: Vec<ComponentNode>,
    },
    MoveBlocks {
        uids: Vec<Uid>,
        target: Uid,
        placement: BlockPlacement,
    },
    ExitCode,
    ExitQuote,
    ExitTrailingQuote,
    SelectBlocks,
    SelectAll,
    MoveSelection {
        direction: Direction,
        extend: bool,
    },
    SetSelection(EditorSelection),
    EditSelected,
    Focus,
    FocusAfterBlock(Uid),
    FocusBeforeBlock(Uid),
    ScrollToBlock {
        uid: Uid,
        highlight: bool,
    },
    Table(TableCommand),
    Image {
        uid: Uid,
        attrs: AttributeMap,
    },
    UpdateNodeAttrs {
        uid: Uid,
        attrs: AttributeMap,
        add_to_history: bool,
    },
    SetTransientAttrs {
        uid: Uid,
        attrs: AttributeMap,
    },
    SelectNode(Uid),
    RemoveNode(Uid),
    ReplaceNode {
        uid: Uid,
        nodes: Vec<ComponentNode>,
    },
    SetCollapsed(Vec<Uid>),
    ToggleCollapsed(Uid),
    SetSuggestionQuery(String),
    OpenEntity(String),
    OpenMenu(String),
    /// Close a menu and return focus to the editor (e.g. after Escape or Apply).
    /// For outside interactions, use `EditorEngine::dismiss_overlays` instead.
    CloseMenu,
    SuggestionNext(i32),
    SuggestionFirst,
    SuggestionLast,
    AcceptSuggestion,
    DismissSuggestion,
    Request(crate::EngineEvent),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockPlacement {
    Before,
    After,
    Inside,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}
#[derive(Clone, Debug, PartialEq)]
pub enum TableCommand {
    Insert {
        rows: usize,
        columns: usize,
        header_row: bool,
    },
    InsertAxis {
        axis: Axis,
        index: usize,
        after: bool,
    },
    DeleteAxis {
        axis: Axis,
        index: usize,
    },
    DuplicateAxis {
        axis: Axis,
        index: usize,
    },
    ToggleHeader(Axis),
    SetAlignment(String),
    SetBackground(Option<String>),
    Clear,
    Delete,
    Navigate {
        backwards: bool,
    },
    MoveRow {
        from: usize,
        to: usize,
    },
    PasteGrid(Vec<Vec<String>>),
    PasteCells(Vec<Vec<Vec<ComponentNode>>>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandAvailability {
    Enabled,
    Disabled(String),
    Hidden,
}
impl Command {
    pub fn mutates(&self) -> bool {
        !matches!(
            self,
            Self::SetSelection(_)
                | Self::SelectBlocks
                | Self::SelectAll
                | Self::MoveSelection { .. }
                | Self::EditSelected
                | Self::Focus
                | Self::FocusBeforeBlock(_)
                | Self::FocusAfterBlock(_)
                | Self::ScrollToBlock { .. }
                | Self::SelectNode(_)
                | Self::SetCollapsed(_)
                | Self::ToggleCollapsed(_)
                | Self::SetSuggestionQuery(_)
                | Self::OpenEntity(_)
                | Self::OpenMenu(_)
                | Self::CloseMenu
                | Self::SuggestionNext(_)
                | Self::SuggestionFirst
                | Self::SuggestionLast
                | Self::DismissSuggestion
                | Self::SetTransientAttrs { .. }
                | Self::Request(_)
        )
    }
}
pub fn run(engine: &mut EditorEngine, command: Command) -> Vec<EngineOutput> {
    if engine.options.read_only && command.mutates() {
        return Vec::new();
    }
    match command {
        Command::Undo => engine.undo(),
        Command::Redo => engine.redo(),
        Command::Table(cmd) => tables::run(engine, cmd),
        Command::Image { uid, attrs } => media::update(engine, uid, attrs),
        Command::UpdateNodeAttrs {
            uid,
            attrs,
            add_to_history,
        } => islands::update(engine, uid, attrs, add_to_history),
        Command::SetTransientAttrs { uid, attrs } => islands::transient(engine, uid, attrs),
        Command::SelectNode(uid) => blocks::select(engine, EditorSelection::Node(uid)),
        Command::RemoveNode(uid) => {
            engine.selection = EditorSelection::Node(uid);
            blocks::run(engine, Command::DeleteBlocks)
        }
        Command::ReplaceNode { uid, nodes } => islands::replace(engine, uid, nodes),
        Command::SetCollapsed(uids) => islands::set_collapsed(engine, uids),
        Command::ToggleCollapsed(uid) => islands::toggle_collapsed(engine, uid),
        Command::ScrollToBlock { uid, highlight } => vec![EngineOutput::Scroll { uid, highlight }],
        Command::Request(event) => vec![EngineOutput::Request(event)],
        other => blocks::run(engine, other),
    }
}
