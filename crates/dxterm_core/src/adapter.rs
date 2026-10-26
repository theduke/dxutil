//! The sole dependency boundary for rio-vt. No rio types escape this module.
use crate::{
    CellPoint, Cursor, CursorShape, Row, SelectionKind, SelectionRange, Snapshot, Style, StyleRun,
    TerminalEvent, TerminalLink, TerminalTheme, Underline,
    input::{Modes, MouseTracking},
    theme::rgb,
};
use rio_vt::{
    ansi::CursorShape as RioCursor,
    config::colors::{AnsiColor, NamedColor},
    crosswords::{
        Crosswords, CrosswordsSize, Mode, TermDamage,
        grid::Scroll,
        pos::{Column, Line, Pos, Side},
        square::{CellFlags, Wide},
        style::{StyleFlags, UnderlineKind},
    },
    event::{EventListener, RioEvent, WindowId},
    performer::handler::Processor,
    selection::{Selection, SelectionType},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone, Default)]
struct Listener(Rc<RefCell<Vec<RioEvent>>>);
impl EventListener for Listener {
    fn send_event(&self, event: RioEvent, _: WindowId) {
        self.0.borrow_mut().push(event);
    }
    fn send_event_with_high_priority(&self, event: RioEvent, id: WindowId) {
        self.send_event(event, id);
    }
    fn send_global_event(&self, event: RioEvent) {
        self.send_event(event, WindowId::from(0));
    }
}

pub(crate) struct Engine {
    term: Crosswords<Listener>,
    parser: Processor,
    listener: Listener,
    theme: TerminalTheme,
    rows: Vec<Arc<Row>>,
    scrollback: usize,
}
impl Engine {
    pub(crate) fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        let listener = Listener::default();
        let mut term = Crosswords::new(
            CrosswordsSize::new(cols as usize, rows as usize),
            RioCursor::Block,
            listener.clone(),
            WindowId::from(0),
            0,
            scrollback,
        );
        term.set_grapheme_clustering(false);
        Self {
            term,
            parser: Processor::default(),
            listener,
            theme: TerminalTheme::default(),
            rows: Vec::new(),
            scrollback,
        }
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) {
        self.term.selection = None;
        self.parser.advance(&mut self.term, bytes);
    }
    pub(crate) fn resize(&mut self, cols: u16, rows: u16) {
        if self.term.columns() != cols as usize || self.term.screen_lines() != rows as usize {
            self.term.selection = None;
            self.term
                .resize(CrosswordsSize::new(cols as usize, rows as usize));
        }
    }
    pub(crate) fn reset(&mut self) {
        let mut new = Self::new(
            self.term.columns() as u16,
            self.term.screen_lines() as u16,
            self.scrollback,
        );
        new.theme = self.theme.clone();
        *self = new;
    }
    pub(crate) fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }
    pub(crate) fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }
    pub(crate) fn set_theme(&mut self, theme: TerminalTheme) {
        if self.theme != theme {
            self.theme = theme;
            self.term.mark_fully_damaged();
        }
    }
    pub(crate) fn drain_events(&mut self) -> Vec<TerminalEvent> {
        let events = std::mem::take(&mut *self.listener.0.borrow_mut());
        events
            .into_iter()
            .filter_map(|event| match event {
                RioEvent::PtyWrite(_, text) => Some(TerminalEvent::PtyWrite(text.into_bytes())),
                RioEvent::Title(_, text) => Some(TerminalEvent::Title(text)),
                RioEvent::ResetTitle => Some(TerminalEvent::Title(String::new())),
                RioEvent::Bell(_) => Some(TerminalEvent::Bell),
                RioEvent::ClipboardStore(_, text) => Some(TerminalEvent::ClipboardStore(text)),
                RioEvent::TextAreaSizeRequest(_, format) => Some(TerminalEvent::PtyWrite(
                    format(rio_vt::event::WindowSize {
                        cols: self.term.columns() as u16,
                        rows: self.term.screen_lines() as u16,
                        width: 0,
                        height: 0,
                    })
                    .into_bytes(),
                )),
                RioEvent::ColorRequest(_, index, format) => {
                    let color = self.color_index(index);
                    // CSS variables cannot be queried headlessly. Use the default palette
                    // when a host has supplied a color that isn't an RGB hex literal.
                    let fallback = if index == 257 { "#111820" } else { "#dce5ed" };
                    let hex = color
                        .strip_prefix('#')
                        .filter(|s| s.len() == 6)
                        .unwrap_or(&fallback[1..]);
                    let value = u32::from_str_radix(hex, 16).unwrap_or(0xdce5ed);
                    Some(TerminalEvent::PtyWrite(
                        format(rio_vt::config::colors::ColorRgb {
                            r: (value >> 16) as u8,
                            g: (value >> 8) as u8,
                            b: value as u8,
                        })
                        .into_bytes(),
                    ))
                }
                _ => None,
            })
            .collect()
    }
    fn color_index(&self, index: usize) -> String {
        if index < 269
            && let Some(color) = self.term.colors()[index]
        {
            return rgb(
                (color[0] * 255.).round() as u8,
                (color[1] * 255.).round() as u8,
                (color[2] * 255.).round() as u8,
            );
        }
        match index {
            0..=255 => self.theme.palette(index as u8),
            257 => self.theme.background(),
            258 => self.theme.cursor(),
            259..=266 => self.theme.palette((index - 259) as u8),
            _ => self.theme.foreground(),
        }
    }
    fn color(&self, color: AnsiColor, bold: bool) -> String {
        match color {
            AnsiColor::Spec(c) => rgb(c.r, c.g, c.b),
            AnsiColor::Indexed(i) => self.color_index(if bold && i < 8 {
                i as usize + 8
            } else {
                i as usize
            }),
            AnsiColor::Named(n) => {
                let index = n as usize;
                let index = if bold && index < 8 {
                    index + 8
                } else if n == NamedColor::LightForeground || n == NamedColor::DimForeground {
                    256
                } else {
                    index
                };
                self.color_index(index)
            }
        }
    }
    fn row(&self, viewport_row: usize) -> Row {
        let line = Line(viewport_row as i32 - self.term.display_offset() as i32);
        let mut row = Row::default();
        let mut byte_offsets = Vec::new();
        let mut text = String::new();
        for col in 0..self.term.columns() {
            let pos = Pos::new(line, Column(col));
            let cell = self.term.grid[pos];
            if col == self.term.columns() - 1 {
                row.wrapped = cell.contains_cell_flag(CellFlags::WRAPLINE);
            }
            if cell.wide() == Wide::Spacer {
                continue;
            }
            let raw = self.term.grid.style_of(&cell);
            let f = raw.flags;
            let bold = f.contains(StyleFlags::BOLD);
            let mut foreground = self.color(raw.fg, bold);
            let mut background = self.color(raw.bg, false);
            if f.contains(StyleFlags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }
            let style = Style {
                foreground,
                background,
                underline_color: raw.underline_color.map(|c| self.color(c, false)),
                bold,
                dim: f.contains(StyleFlags::DIM),
                italic: f.contains(StyleFlags::ITALIC),
                hidden: f.contains(StyleFlags::HIDDEN),
                strike: f.contains(StyleFlags::STRIKEOUT),
                blink: f.intersects(StyleFlags::ALL_BLINK),
                underline: match f.underline_kind() {
                    None => Underline::None,
                    Some(UnderlineKind::Single) => Underline::Single,
                    Some(UnderlineKind::Double) => Underline::Double,
                    Some(UnderlineKind::Curly) => Underline::Curly,
                    Some(UnderlineKind::Dotted) => Underline::Dotted,
                    Some(UnderlineKind::Dashed) => Underline::Dashed,
                },
            };
            // A leading spacer reserves the last physical cell before a wide
            // glyph wraps. It contributes no character to the logical line.
            let cell_text: String = if cell.wide() == Wide::LeadingSpacer {
                String::new()
            } else {
                self.term
                    .grid
                    .cell_text(pos)
                    .map(|c| if c == '\0' { ' ' } else { c })
                    .collect()
            };
            byte_offsets.push(text.len());
            text.push_str(&cell_text);
            row.runs.push(StyleRun {
                col: col as u16,
                cell_width: if cell.wide() == Wide::Wide { 2 } else { 1 },
                text: cell_text,
                style,
                wide: cell.wide() == Wide::Wide,
                hyperlink: self
                    .term
                    .cell_hyperlink(line, Column(col))
                    .map(|link| TerminalLink::Url(link.uri().into())),
            });
        }
        let links = crate::links::detect_links(&text);
        let mut merged: Vec<StyleRun> = Vec::new();
        for (mut run, byte) in row.runs.into_iter().zip(byte_offsets) {
            if run.hyperlink.is_none() {
                run.hyperlink = links
                    .iter()
                    .find(|link| byte >= link.start && byte < link.end)
                    .map(|link| link.link.clone());
            }
            if let Some(last) = merged.last_mut()
                && !last.wide
                && !run.wide
                && last.style == run.style
                && last.hyperlink == run.hyperlink
            {
                last.text.push_str(&run.text);
                last.cell_width += run.cell_width;
            } else {
                merged.push(run);
            }
        }
        row.runs = merged;
        row
    }
    pub(crate) fn snapshot(&mut self) -> Snapshot {
        let count = self.term.screen_lines();
        let mut dirty: Vec<usize> = match self.term.damage() {
            TermDamage::Full => (0..count).collect(),
            TermDamage::Partial(lines) => lines
                .map(|line| line.line)
                .filter(|line| *line < count)
                .collect(),
        };
        if self.rows.len() != count {
            dirty = (0..count).collect();
            self.rows.resize_with(count, || Arc::new(Row::default()));
        }
        for &row in &dirty {
            let next = self.row(row);
            if *self.rows[row] != next {
                self.rows[row] = Arc::new(next);
            }
        }
        self.term.reset_damage();
        let cursor = self.term.cursor();
        Snapshot {
            cols: self.term.columns() as u16,
            rows: self.rows.clone(),
            damaged_rows: dirty.into_iter().map(|r| r as u16).collect(),
            cursor: Cursor {
                position: CellPoint {
                    row: cursor.pos.row.0.max(0) as u16,
                    col: cursor.pos.col.0 as u16,
                },
                cell_width: if self.term.grid[cursor.pos].wide() == Wide::Wide {
                    2
                } else {
                    1
                },
                shape: match cursor.content {
                    RioCursor::Underline => CursorShape::Underline,
                    RioCursor::Beam => CursorShape::Beam,
                    _ => CursorShape::Block,
                },
                visible: cursor.content != RioCursor::Hidden,
                blinking: self.term.blinking_cursor,
            },
            selection: self.selection_ranges(),
            display_offset: self.term.display_offset(),
        }
    }
    pub(crate) fn modes(&self) -> Modes {
        let m = self.term.mode();
        Modes {
            application_cursor: m.contains(Mode::APP_CURSOR),
            application_keypad: m.contains(Mode::APP_KEYPAD),
            bracketed_paste: m.contains(Mode::BRACKETED_PASTE),
            focus_events: m.contains(Mode::FOCUS_IN_OUT),
            sgr_mouse: m.contains(Mode::SGR_MOUSE),
            utf8_mouse: m.contains(Mode::UTF8_MOUSE),
            newline: m.contains(Mode::LINE_FEED_NEW_LINE),
            kitty_flags: self.term.keyboard_mode().bits(),
            modify_other_keys: self.term.modify_other_keys().unwrap_or(0),
            mouse: if m.contains(Mode::MOUSE_MOTION) {
                MouseTracking::Any
            } else if m.contains(Mode::MOUSE_DRAG) {
                MouseTracking::Button
            } else if m.contains(Mode::MOUSE_REPORT_CLICK) {
                MouseTracking::Normal
            } else if m.contains(Mode::MOUSE_REPORT_X10) {
                MouseTracking::X10
            } else {
                MouseTracking::None
            },
        }
    }
    fn pos(&self, point: CellPoint) -> Pos {
        Pos::new(
            Line(
                point.row.min(self.term.screen_lines() as u16 - 1) as i32
                    - self.term.display_offset() as i32,
            ),
            Column(point.col.min(self.term.columns() as u16 - 1) as usize),
        )
    }
    pub(crate) fn start_selection(&mut self, point: CellPoint, kind: SelectionKind) {
        let ty = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Word => SelectionType::Semantic,
            SelectionKind::Line => SelectionType::Lines,
        };
        self.term.selection = Some(Selection::new(ty, self.pos(point), Side::Left));
        if kind != SelectionKind::Simple {
            self.update_selection(point);
        }
    }
    pub(crate) fn update_selection(&mut self, point: CellPoint) {
        let pos = self.pos(point);
        if let Some(selection) = &mut self.term.selection {
            selection.update(pos, Side::Right);
            selection.include_all();
        }
    }
    pub(crate) fn clear_selection(&mut self) {
        self.term.selection = None;
    }
    pub(crate) fn selection_text(&self) -> String {
        self.term.selection_to_string().unwrap_or_default()
    }
    pub(crate) fn has_selection(&self) -> bool {
        self.term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
            .is_some()
    }
    fn selection_ranges(&self) -> Vec<SelectionRange> {
        let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&self.term))
        else {
            return Vec::new();
        };
        let offset = self.term.display_offset() as i32;
        (0..self.term.screen_lines())
            .filter_map(|row| {
                let line = row as i32 - offset;
                if line < range.start.row.0 || line > range.end.row.0 {
                    return None;
                }
                Some(SelectionRange {
                    row: row as u16,
                    start_col: if line == range.start.row.0 {
                        range.start.col.0 as u16
                    } else {
                        0
                    },
                    end_col: if line == range.end.row.0 {
                        range.end.col.0 as u16 + 1
                    } else {
                        self.term.columns() as u16
                    },
                })
            })
            .collect()
    }
    pub(crate) fn link_at(&mut self, point: CellPoint) -> Option<TerminalLink> {
        let pos = self.pos(point);
        if let Some(link) = self.term.cell_hyperlink(pos.row, pos.col) {
            return Some(TerminalLink::Url(link.uri().into()));
        }
        // Search the logical line, including wrapped rows, without relying on
        // the render cache having been refreshed since the most recent write.
        let mut first = point.row.min(self.term.screen_lines() as u16 - 1) as usize;
        while first > 0 && self.row(first - 1).wrapped {
            first -= 1;
        }
        let mut text = String::new();
        let mut target = 0;
        for row in first..self.term.screen_lines() {
            let data = self.row(row);
            for run in &data.runs {
                if row == point.row as usize
                    && point.col >= run.col
                    && point.col < run.col + run.cell_width
                {
                    // Merged runs can contain combining characters; use the
                    // original grid to locate the cell's precise byte offset.
                    target = text.len();
                    for col in run.col..point.col {
                        let p = self.pos(CellPoint {
                            row: row as u16,
                            col,
                        });
                        if self.term.grid[p].wide() != Wide::Spacer {
                            target += self
                                .term
                                .grid
                                .cell_text(p)
                                .map(char::len_utf8)
                                .sum::<usize>();
                        }
                    }
                }
                text.push_str(&run.text);
            }
            if !data.wrapped {
                break;
            }
        }
        crate::links::detect_links(&text)
            .into_iter()
            .find(|l| target >= l.start && target < l.end)
            .map(|l| l.link)
    }
}
