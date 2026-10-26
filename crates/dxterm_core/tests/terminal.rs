use dxterm_core::input::{Key, KeyEvent, Modifiers};
use dxterm_core::{
    CellPoint, SelectionKind, Terminal, TerminalEvent, TerminalLink, TerminalTheme, Underline,
};
use std::sync::Arc;

fn text(term: &mut Terminal, row: usize) -> String {
    term.snapshot().rows[row].text().trim_end().to_string()
}
#[test]
fn sgr_palette_truecolor_and_attributes() {
    let mut t = Terminal::new(30, 3, 100);
    let mut theme = TerminalTheme::default();
    theme.colors.insert("red".into(), "#123456".into());
    t.set_theme(theme);
    t.write(b"\x1b[31mR\x1b[38;2;1;2;3;48;5;196;1;3;4;9mS\x1b[0mN");
    let s = t.snapshot();
    let runs = &s.rows[0].runs;
    assert_eq!(runs[0].style.foreground, "#123456");
    assert_eq!(runs[1].style.foreground, "#010203");
    assert_eq!(runs[1].style.background, "#ff0000");
    assert!(runs[1].style.bold && runs[1].style.italic && runs[1].style.strike);
    assert_eq!(runs[1].style.underline, Underline::Single);
    assert!(!runs[2].style.bold);
    t.write(b"\x1b[2;7;8mX");
    let s = t.snapshot();
    let run = &s.rows[0].runs[3];
    assert!(run.style.dim && run.style.hidden);
    assert!(run.style.foreground.contains("--terminal-background"));
}
#[test]
fn incremental_damage_preserves_rows() {
    let mut t = Terminal::new(20, 4, 100);
    let before = t.snapshot();
    assert!(t.snapshot().damaged_rows.is_empty());
    t.write(b"hello");
    let after = t.snapshot();
    assert!(after.damaged_rows.contains(&0));
    assert!(Arc::ptr_eq(&before.rows[2], &after.rows[2]));
    t.set_theme(TerminalTheme {
        foreground: Some("#abcdef".into()),
        ..Default::default()
    });
    assert_eq!(t.snapshot().damaged_rows.len(), 4);
}
#[test]
fn wide_combining_and_split_utf8() {
    let mut t = Terminal::new(20, 3, 100);
    for byte in "界e\u{301}".as_bytes() {
        t.write(&[*byte]);
    }
    let s = t.snapshot();
    assert_eq!(s.rows[0].runs[0].text, "界");
    assert_eq!(s.rows[0].runs[0].cell_width, 2);
    assert!(s.rows[0].runs[0].wide);
    assert!(s.rows[0].text().starts_with("界e\u{301}"));
    assert_eq!(s.cursor.position.col, 3);
}
#[test]
fn zwj_clustering_is_opt_in_and_can_be_reset() {
    let mut t = Terminal::new(20, 3, 100);
    t.write("👩\u{200d}💻".as_bytes());
    let legacy = t.snapshot();
    assert_eq!(legacy.cursor.position.col, 4);
    t.reset();
    t.write(b"\x1b[?2027h");
    for chunk in "👩\u{200d}💻".as_bytes().chunks(2) {
        t.write(chunk);
    }
    let s = t.snapshot();
    assert_eq!(s.cursor.position.col, 2);
    assert_eq!(s.rows[0].runs[0].text, "👩\u{200d}💻");
    t.reset();
    t.write("👩\u{200d}💻".as_bytes());
    assert_eq!(t.snapshot().cursor.position.col, 4);
}
#[test]
fn resize_reflows_and_selection_joins_wrapped_lines() {
    let mut t = Terminal::new(5, 4, 100);
    t.write(b"abcdefghij");
    assert!(t.snapshot().rows[0].wrapped);
    t.start_selection(CellPoint { row: 0, col: 1 }, SelectionKind::Simple);
    t.update_selection(CellPoint { row: 1, col: 3 });
    assert_eq!(t.selection_text(), "bcdefghi");
    assert_eq!(t.snapshot().selection.len(), 2);
    t.resize(10, 4);
    assert_eq!(text(&mut t, 0), "abcdefghij");
    assert!(t.selection_text().is_empty());
}
#[test]
fn alternate_screen_restores_primary() {
    let mut t = Terminal::new(20, 3, 100);
    t.write(b"primary\x1b[?1049h\x1b[Halternate");
    assert_eq!(text(&mut t, 0), "alternate");
    t.write(b"\x1b[?1049l");
    assert_eq!(text(&mut t, 0), "primary");
}
#[test]
fn scroll_regions_and_scrollback() {
    let mut t = Terminal::new(10, 4, 100);
    t.write(b"top\r\nA\r\nB\r\nbottom\x1b[2;3r\x1b[3;1H\n");
    assert_eq!(text(&mut t, 0), "top");
    assert_eq!(text(&mut t, 1), "B");
    assert_eq!(text(&mut t, 3), "bottom");
    t.reset();
    t.write(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
    t.scroll(1);
    assert_eq!(t.snapshot().display_offset, 1);
    assert_eq!(text(&mut t, 0), "one");
    t.scroll_to_bottom();
    assert_eq!(t.snapshot().display_offset, 0);
    assert_eq!(text(&mut t, 0), "two");
}
#[test]
fn dsr_title_bell_clipboard_and_modes() {
    let mut t = Terminal::new(20, 4, 100);
    t.write(b"\x1b[3;3H\x1b[6n\x1b]2;hello\x07\x07\x1b]52;c;aGk=\x07");
    let events = t.drain_events();
    assert!(events.contains(&TerminalEvent::PtyWrite(b"\x1b[3;3R".to_vec())));
    assert!(events.contains(&TerminalEvent::Title("hello".into())));
    assert!(events.contains(&TerminalEvent::Bell));
    assert!(events.contains(&TerminalEvent::ClipboardStore("hi".into())));
    t.write(b"\x1b[?2004h\x1b[?1h\x1b[>1u");
    assert_eq!(t.paste("hello\x1b[201~"), b"\x1b[200~hello[201~\x1b[201~");
    assert_eq!(
        t.encode_key(&KeyEvent::new(
            Key::Character("c".into()),
            Modifiers {
                control: true,
                ..Default::default()
            }
        )),
        b"\x1b[99;5u"
    );
    t.write(b"\x1b[<u");
    assert_eq!(
        t.encode_key(&KeyEvent::new(Key::ArrowUp, Modifiers::default())),
        b"\x1bOA"
    );
}
#[test]
fn selection_kinds_and_hyperlinks() {
    let mut t = Terminal::new(60, 4, 100);
    t.write(
        b"hello world\r\n\x1b]8;;https://example.com\x1b\\click\x1b]8;;\x1b\\ src/main.rs:12:4",
    );
    t.start_selection(CellPoint { row: 0, col: 7 }, SelectionKind::Word);
    assert_eq!(t.selection_text(), "world");
    t.start_selection(CellPoint { row: 0, col: 1 }, SelectionKind::Line);
    assert_eq!(t.selection_text(), "hello world\n");
    assert_eq!(
        t.link_at(CellPoint { row: 1, col: 2 }),
        Some(TerminalLink::Url("https://example.com".into()))
    );
    assert_eq!(
        t.link_at(CellPoint { row: 1, col: 9 }),
        Some(TerminalLink::File {
            path: "src/main.rs".into(),
            line: 12,
            column: Some(4)
        })
    );
    let s = t.snapshot();
    assert_eq!(
        s.rows[1].runs[0].hyperlink,
        Some(TerminalLink::Url("https://example.com".into()))
    );
}
#[test]
fn links_across_wrapped_and_wide_cells() {
    let mut t = Terminal::new(20, 4, 100);
    t.write("界 https://example.com/path".as_bytes());
    assert_eq!(
        t.link_at(CellPoint { row: 0, col: 4 }),
        Some(TerminalLink::Url("https://example.com/path".into()))
    );
    assert_eq!(
        t.link_at(CellPoint { row: 1, col: 2 }),
        Some(TerminalLink::Url("https://example.com/path".into()))
    );
}

#[test]
fn cursor_shape_visibility_and_wide_cell_width() {
    use dxterm_core::CursorShape;
    let mut t = Terminal::new(20, 3, 100);
    t.write("界".as_bytes());
    t.write(b"\x1b[H");
    assert_eq!(t.snapshot().cursor.cell_width, 2);
    t.write(b"\x1b[6 q");
    assert_eq!(t.snapshot().cursor.shape, CursorShape::Beam);
    t.write(b"\x1b[?25l");
    assert!(!t.snapshot().cursor.visible);
    t.write(b"\x1b[?25h\x1b[4 q");
    let cursor = t.snapshot().cursor;
    assert!(cursor.visible);
    assert_eq!(cursor.shape, CursorShape::Underline);
}

#[test]
fn backwards_selection_and_palette_damage() {
    let mut t = Terminal::new(20, 3, 100);
    t.write(b"\x1b[31mabcdef\x1b[0m");
    t.snapshot();
    t.start_selection(CellPoint { row: 0, col: 4 }, SelectionKind::Simple);
    assert!(t.selection_text().is_empty());
    t.update_selection(CellPoint { row: 0, col: 1 });
    assert_eq!(t.selection_text(), "bcde");
    t.write(b"\x1b]4;1;rgb:12/34/56\x07");
    let snapshot = t.snapshot();
    assert!(snapshot.damaged_rows.contains(&0));
    assert_eq!(snapshot.rows[0].runs[0].style.foreground, "#123456");
}

#[test]
fn wide_wrap_padding_does_not_break_logical_links() {
    let mut t = Terminal::new(8, 3, 100);
    t.write("http://界".as_bytes());
    assert_eq!(
        t.link_at(CellPoint { row: 1, col: 0 }),
        Some(TerminalLink::Url("http://界".into()))
    );
}
