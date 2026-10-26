//! A transport-independent terminal model. All coordinates are zero based.
//! Engine-specific types are confined to the private `adapter` module.
mod adapter;
pub mod input;
pub mod links;
mod theme;

pub use links::TerminalLink;
use std::sync::Arc;
pub use theme::TerminalTheme;

/// An inclusive cell coordinate in the currently displayed viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellPoint {
    pub row: u16,
    pub col: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    Simple,
    Word,
    Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Exact bytes to send back to the originating application.
    PtyWrite(Vec<u8>),
    Title(String),
    Bell,
    /// OSC 52 store request. Hosts decide whether to grant clipboard access.
    ClipboardStore(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// Resolved CSS-compatible colors and terminal attributes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub foreground: String,
    pub background: String,
    pub underline_color: Option<String>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub hidden: bool,
    pub strike: bool,
    pub blink: bool,
    pub underline: Underline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleRun {
    pub col: u16,
    /// Total occupied cells. Wide graphemes are emitted as separate runs.
    pub cell_width: u16,
    pub text: String,
    pub style: Style,
    pub wide: bool,
    pub hyperlink: Option<TerminalLink>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub runs: Vec<StyleRun>,
    pub wrapped: bool,
}
impl Row {
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Beam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub position: CellPoint,
    pub cell_width: u16,
    pub shape: CursorShape,
    pub visible: bool,
    pub blinking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub cols: u16,
    pub rows: Vec<Arc<Row>>,
    /// Rows rebuilt since the previous snapshot. Unchanged rows retain their Arc.
    pub damaged_rows: Vec<u16>,
    pub cursor: Cursor,
    pub selection: Vec<SelectionRange>,
    pub display_offset: usize,
}

/// Owns parser state, scrollback, selection and the incremental row cache.
pub struct Terminal {
    engine: adapter::Engine,
}
impl Terminal {
    /// Sizes are clamped to at least two columns and one row.
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        Self {
            engine: adapter::Engine::new(cols.max(2), rows.max(1), scrollback),
        }
    }
    /// Feed arbitrary byte chunks, including incomplete UTF-8 and escape sequences.
    pub fn write(&mut self, bytes: &[u8]) {
        self.engine.write(bytes);
    }
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.engine.resize(cols.max(2), rows.max(1));
    }
    pub fn reset(&mut self) {
        self.engine.reset();
    }
    /// Positive lines move towards older history; negative lines towards the bottom.
    pub fn scroll(&mut self, lines: i32) {
        self.engine.scroll(lines);
    }
    pub fn scroll_to_bottom(&mut self) {
        self.engine.scroll_to_bottom();
    }
    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.engine.set_theme(theme);
    }
    pub fn snapshot(&mut self) -> Snapshot {
        self.engine.snapshot()
    }
    pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
        self.engine.drain_events()
    }
    pub fn modes(&self) -> input::Modes {
        self.engine.modes()
    }
    pub fn encode_key(&self, event: &input::KeyEvent) -> Vec<u8> {
        input::encode_key(self.modes(), event)
    }
    pub fn encode_mouse(&self, event: input::MouseEvent) -> Vec<u8> {
        input::encode_mouse(self.modes(), event)
    }
    pub fn paste(&self, text: &str) -> Vec<u8> {
        input::encode_paste(self.modes(), text)
    }
    pub fn focus(&self, focused: bool) -> Vec<u8> {
        input::encode_focus(self.modes(), focused)
    }
    pub fn start_selection(&mut self, point: CellPoint, kind: SelectionKind) {
        self.engine.start_selection(point, kind);
    }
    pub fn update_selection(&mut self, point: CellPoint) {
        self.engine.update_selection(point);
    }
    pub fn clear_selection(&mut self) {
        self.engine.clear_selection();
    }
    pub fn selection_text(&self) -> String {
        self.engine.selection_text()
    }
    pub fn has_selection(&self) -> bool {
        self.engine.has_selection()
    }
    pub fn link_at(&mut self, point: CellPoint) -> Option<TerminalLink> {
        self.engine.link_at(point)
    }
}
