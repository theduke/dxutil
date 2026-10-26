use crate::{Frame, TerminalLink, TerminalTheme, TerminalWriter, keyboard, platform};
use dioxus::core::Task;
use dioxus::prelude::*;
use dxterm_core::{
    CellPoint, CursorShape, Row, SelectionKind, Snapshot, Terminal, TerminalEvent, Underline,
    input::{
        Key, MouseButton as Button, MouseEvent as Report, MouseEventKind as Action, MouseTracking,
    },
};
use std::{rc::Rc, sync::Arc};

#[derive(Clone, Copy, PartialEq)]
struct Metrics {
    width: f64,
    height: f64,
    left: f64,
    top: f64,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            width: 8.,
            height: 16.,
            left: 0.,
            top: 0.,
        }
    }
}
fn cell(x: f64, y: f64, metrics: Metrics, snapshot: &Snapshot) -> CellPoint {
    CellPoint {
        col: ((x - metrics.left).max(0.) / metrics.width)
            .floor()
            .min(f64::from(snapshot.cols - 1)) as u16,
        row: ((y - metrics.top).max(0.) / metrics.height)
            .floor()
            .min(snapshot.rows.len().saturating_sub(1) as f64) as u16,
    }
}
fn emit(input: EventHandler<Vec<u8>>, bytes: Vec<u8>) {
    if !bytes.is_empty() {
        input.call(bytes);
    }
}

async fn measure(host: &MountedData, probe: &MountedData) -> Option<(Metrics, (u16, u16))> {
    let (rect, glyph) = futures::join!(host.get_client_rect(), probe.get_client_rect());
    let (rect, glyph) = (rect.ok()?, glyph.ok()?);
    if !(glyph.size.width > 0.
        && glyph.size.height > 0.
        && rect.size.width > 0.
        && rect.size.height > 0.)
    {
        return None;
    }
    let metrics = Metrics {
        width: glyph.size.width / 10.,
        height: glyph.size.height,
        left: rect.origin.x,
        top: rect.origin.y,
    };
    let size = (
        (rect.size.width / metrics.width)
            .floor()
            .clamp(2., f64::from(u16::MAX)) as u16,
        (rect.size.height / metrics.height)
            .floor()
            .clamp(1., f64::from(u16::MAX)) as u16,
    );
    Some((metrics, size))
}

#[derive(Default)]
struct Announcements {
    text: String,
    last_announced: f64,
    timer: Option<Task>,
}

fn announce(mut state: Signal<Announcements>, mut live: Signal<String>, text: &str) {
    if text.is_empty() {
        return;
    }
    let delay = {
        let mut state = state.write();
        state.text.push_str(text);
        if state.text.len() > 4096 {
            let mut start = state.text.len() - 4096;
            while !state.text.is_char_boundary(start) {
                start += 1;
            }
            state.text.drain(..start);
        }
        if state.timer.is_some() {
            return;
        }
        (500. - (dxutil::time::now_ms() - state.last_announced)).clamp(0., 500.)
    };
    let timer = spawn(async move {
        futures_timer::Delay::new(std::time::Duration::from_secs_f64(delay / 1000.)).await;
        let text = {
            let mut state = state.write();
            state.timer = None;
            state.last_announced = dxutil::time::now_ms();
            std::mem::take(&mut state.text)
        };
        live.set(text);
    });
    state.write().timer = Some(timer);
}

fn clear_announcements(mut state: Signal<Announcements>, mut live: Signal<String>) {
    let timer = {
        let mut state = state.write();
        state.text.clear();
        state.timer.take()
    };
    if let Some(timer) = timer {
        timer.cancel();
    }
    live.set(String::new());
}

/// Interactive terminal. Connect `on_input` directly to the same PTY that feeds
/// `writer`; this includes replies to terminal queries as well as user input.
#[component]
pub fn TerminalView(
    writer: TerminalWriter,
    on_input: EventHandler<Vec<u8>>,
    on_resize: EventHandler<(u16, u16)>,
    on_link: EventHandler<TerminalLink>,
    on_selection: EventHandler<String>,
    #[props(default)] theme: TerminalTheme,
    #[props(default)] font_family: Option<String>,
    #[props(default)] font_size: Option<u16>,
    #[props(default)] on_title: EventHandler<String>,
    #[props(default)] on_bell: EventHandler<()>,
    #[props(default)] on_clipboard_store: EventHandler<String>,
) -> Element {
    let id = dxutil::id::use_unique_id("dxterm");
    let mut model = use_signal(|| Terminal::new(80, 24, 10_000));
    let mut snapshot = use_signal(|| model.write().snapshot());
    #[allow(unused_mut)]
    let mut host = use_signal(|| None::<Rc<MountedData>>);
    #[allow(unused_mut)]
    let mut probe = use_signal(|| None::<Rc<MountedData>>);
    let focus_target = use_signal(|| None::<Rc<MountedData>>);
    let mut metrics = use_signal(Metrics::default);
    let live = use_signal(String::new);
    let mut dragging = use_signal(|| false);
    let mut drag_origin = use_signal(CellPoint::default);
    let mut click_state = use_signal(|| (0f64, CellPoint::default(), 0u8));
    let preedit = use_signal(String::new);
    let composing = use_signal(|| false);
    let mut focused = use_signal(|| cfg!(feature = "native"));

    let announcements = use_signal(Announcements::default);
    use_effect(use_reactive(
        (&theme, &font_family, &font_size),
        move |(theme, _, _)| {
            model.write().set_theme(theme);
            writer.request_measure();
            writer.request_render();
        },
    ));
    use_future(move || async move {
        #[cfg(feature = "native")]
        on_resize.call((80, 24));
        let mut size = (0, 0);
        loop {
            writer.notified().await;
            // Drain only after the frame boundary so bursts share one snapshot.
            platform::next_frame().await;
            let pending = writer.take_pending();
            let selected_before = model.peek().has_selection();
            let mut changed = pending.render || !pending.frames.is_empty();
            if pending.measure {
                let host = host.peek().clone();
                let probe = probe.peek().clone();
                if let (Some(host), Some(probe)) = (host, probe)
                    && let Some((measured, new_size)) = measure(&host, &probe).await
                {
                    if *metrics.peek() != measured {
                        metrics.set(measured);
                    }
                    if size != new_size {
                        size = new_size;
                        model.write().resize(size.0, size.1);
                        on_resize.call(size);
                        changed = true;
                    }
                }
            }
            let mut completions = Vec::with_capacity(pending.frames.len());
            let mut events = model.write().drain_events();
            for frame in pending.frames {
                match frame {
                    Frame::Output(bytes, done) => {
                        model.write().write(&bytes);
                        completions.push(done);
                    }
                    Frame::Reset(done) => {
                        model.write().reset();
                        clear_announcements(announcements, live);
                        completions.push(done);
                    }
                }
                // Preserve notifications before a later reset in this batch.
                events.extend(model.write().drain_events());
            }
            // Drop the model borrow before calling host callbacks.
            for event in events {
                match event {
                    TerminalEvent::PtyWrite(bytes) => emit(on_input, bytes),
                    TerminalEvent::Title(title) => on_title.call(title),
                    TerminalEvent::Bell => on_bell.call(()),
                    TerminalEvent::ClipboardStore(text) => on_clipboard_store.call(text),
                }
            }
            if changed {
                if selected_before && !model.peek().has_selection() {
                    on_selection.call(String::new());
                }
                let next = model.write().snapshot();
                let mut text = String::new();
                for &row in &next.damaged_rows {
                    if let Some(data) = next.rows.get(row as usize) {
                        if snapshot
                            .peek()
                            .rows
                            .get(row as usize)
                            .is_some_and(|old| Arc::ptr_eq(old, data))
                        {
                            continue;
                        }
                        let row_text = data.text();
                        let row_text = row_text.trim_end();
                        if !row_text.is_empty() {
                            text.push_str(row_text);
                            text.push('\n');
                        }
                    }
                }
                announce(announcements, live, &text);
                snapshot.set(next);
            }
            for done in completions {
                let _ = done.send(());
            }
        }
    });
    let wheel = use_callback(move |event: WheelEvent| {
        let delta = event.delta().strip_units();
        if delta.y == 0. {
            return;
        }
        event.prevent_default();
        let mods = keyboard::modifiers(event.modifiers());
        if model.peek().modes().mouse != MouseTracking::None && !mods.shift {
            let coords = event.client_coordinates();
            let point = cell(coords.x, coords.y, *metrics.peek(), &snapshot.peek());
            emit(
                on_input,
                model.peek().encode_mouse(Report {
                    position: point,
                    button: if delta.y < 0. {
                        Button::WheelUp
                    } else {
                        Button::WheelDown
                    },
                    kind: Action::Press,
                    modifiers: mods,
                }),
            );
        } else {
            model.write().scroll(if delta.y < 0. { 3 } else { -3 });
            writer.request_render();
        }
    });
    #[cfg(not(feature = "native"))]
    let wheel_attributes = vec![dioxus::html::onwheel(wheel)];
    #[cfg(feature = "native")]
    let wheel_attributes: Vec<Attribute> = {
        let _ = wheel;
        Vec::new()
    };
    #[cfg(not(feature = "native"))]
    let host_attributes = vec![
        dioxus::html::onmounted(move |event: MountedEvent| {
            host.set(Some(event.data()));
            writer.request_measure();
        }),
        dioxus::html::onresize(move |_: ResizeEvent| writer.request_measure()),
    ];
    #[cfg(feature = "native")]
    let host_attributes: Vec<Attribute> = Vec::new();
    #[cfg(not(feature = "native"))]
    let probe_attributes = vec![
        dioxus::html::onmounted(move |event: MountedEvent| {
            probe.set(Some(event.data()));
            writer.request_measure();
        }),
        dioxus::html::onresize(move |_: ResizeEvent| writer.request_measure()),
    ];
    #[cfg(feature = "native")]
    let probe_attributes: Vec<Attribute> = Vec::new();
    let onmousedown = move |event: MouseEvent| {
        if let Some(target) = focus_target.peek().clone().or_else(|| host.peek().clone()) {
            spawn(async move {
                let _ = target.set_focus(true).await;
            });
        }
        let coords = event.client_coordinates();
        let point = cell(coords.x, coords.y, *metrics.peek(), &snapshot.peek());
        let mods = keyboard::modifiers(event.modifiers());
        let button = button(event.trigger_button());
        if model.peek().modes().mouse != MouseTracking::None && !mods.shift {
            let bytes = model.peek().encode_mouse(Report {
                position: point,
                button,
                kind: Action::Press,
                modifiers: mods,
            });
            emit(on_input, bytes);
            return;
        }
        if button != Button::Left {
            return;
        }
        let (last, previous, count) = *click_state.peek();
        let now = dxutil::time::now_ms();
        let count = if now - last < 400. && previous == point {
            count % 3 + 1
        } else {
            1
        };
        click_state.set((now, point, count));
        drag_origin.set(point);
        dragging.set(true);
        model.write().start_selection(
            point,
            match count {
                2 => SelectionKind::Word,
                3 => SelectionKind::Line,
                _ => SelectionKind::Simple,
            },
        );
        writer.request_render();
        on_selection.call(model.peek().selection_text());
    };
    let onmousemove = move |event: MouseEvent| {
        let coords = event.client_coordinates();
        let point = cell(coords.x, coords.y, *metrics.peek(), &snapshot.peek());
        let mods = keyboard::modifiers(event.modifiers());
        if model.peek().modes().mouse != MouseTracking::None && !mods.shift {
            let held = event.held_buttons();
            let button = if held.contains(dioxus::html::input_data::MouseButton::Primary) {
                Button::Left
            } else if held.contains(dioxus::html::input_data::MouseButton::Secondary) {
                Button::Right
            } else if held.contains(dioxus::html::input_data::MouseButton::Auxiliary) {
                Button::Middle
            } else {
                Button::None
            };
            emit(
                on_input,
                model.peek().encode_mouse(Report {
                    position: point,
                    button,
                    kind: Action::Move,
                    modifiers: mods,
                }),
            );
        } else if *dragging.peek() {
            model.write().update_selection(point);
            writer.request_render();
            on_selection.call(model.peek().selection_text());
        }
    };
    let onmouseup = move |event: MouseEvent| {
        let coords = event.client_coordinates();
        let point = cell(coords.x, coords.y, *metrics.peek(), &snapshot.peek());
        let mods = keyboard::modifiers(event.modifiers());
        if model.peek().modes().mouse != MouseTracking::None && !mods.shift {
            emit(
                on_input,
                model.peek().encode_mouse(Report {
                    position: point,
                    button: button(event.trigger_button()),
                    kind: Action::Release,
                    modifiers: mods,
                }),
            );
        } else if *dragging.peek() {
            if point == *drag_origin.peek() {
                if click_state.peek().2 == 1 {
                    model.write().clear_selection();
                    writer.request_render();
                }
                let link = model.write().link_at(point);
                if let Some(link) = link {
                    on_link.call(link);
                }
            }
            on_selection.call(model.peek().selection_text());
        }
        dragging.set(false);
    };
    let data = snapshot.read();
    let m = *metrics.read();
    let font=font_family.unwrap_or_else(||"var(--semantic-font-mono, var(--font-mono, ui-monospace, SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace))".into());
    // Blitz cannot measure glyphs. Its documented fallback assumes the usual
    // 0.6-em monospace advance: 13 1/3 px gives an 8 px cell.
    let size = font_size
        .map(f64::from)
        .unwrap_or(if cfg!(feature = "native") {
            40. / 3.
        } else {
            13.
        });
    let line_height = if cfg!(feature = "native") {
        "16px"
    } else {
        "1.2"
    };
    let cursor = data.cursor;
    let cursor_x = f64::from(cursor.position.col) * m.width;
    let cursor_y = f64::from(cursor.position.row) * m.height;
    let cursor_style = match cursor.shape {
        CursorShape::Block => format!(
            "width:{}px;height:{}px;opacity:0.5",
            f64::from(cursor.cell_width) * m.width,
            m.height
        ),
        CursorShape::Beam => format!("width:2px;height:{}px", m.height),
        CursorShape::Underline => format!(
            "width:{}px;height:2px;transform:translateY({}px)",
            f64::from(cursor.cell_width) * m.width,
            m.height - 2.
        ),
    };
    let input_style = format!(
        "position:absolute;left:{cursor_x}px;top:{cursor_y}px;width:1px;height:{}px;opacity:0.01;padding:0;border:0;resize:none;overflow:hidden;",
        m.height
    );
    let input_id = format!("{id}-input");
    rsx! {
        style { {include_str!("terminal.css")} }
        div {
            id, class:"dxterm", role:"application", aria_label:"Interactive terminal", aria_roledescription:"terminal",
            tabindex:if cfg!(all(any(feature="web",feature="desktop"),not(feature="native"))) {"-1"} else {"0"},
            style:"position:relative;overflow:hidden;width:100%;height:100%;min-width:0;min-height:0;outline:none;touch-action:none;font-family:{font};font-size:{size}px;line-height:{line_height};color:{theme.foreground()};background:{theme.background()};",
            onfocusin:move |_| {focused.set(true);emit(on_input,model.peek().focus(true));},
            onfocusout:move |_| {focused.set(false);emit(on_input,model.peek().focus(false));},
            onkeydown:move |event|handle_key(event,false,model,snapshot,on_input,on_selection,writer),
            onkeyup:move |event|handle_key(event,true,model,snapshot,on_input,on_selection,writer),
            onmousedown:onmousedown,
            onmousemove:onmousemove,
            onmouseup:onmouseup,
            onmouseleave:move |_|dragging.set(false),
            ..host_attributes,
            ..wheel_attributes,

            span {class:"dxterm-probe",aria_hidden:"true",style:"position:absolute;visibility:hidden;white-space:pre;pointer-events:none;",..probe_attributes,"MMMMMMMMMM"}
            div {class:"dxterm-rows",role:"log",aria_live:"off",
                for (index,row) in data.rows.iter().enumerate() {
                    TerminalRow {key:"{index}",row:row.clone(),width:m.width,height:m.height}
                }
            }
            for range in &data.selection {
                div {class:"dxterm-selection",aria_hidden:"true",style:"position:absolute;pointer-events:none;left:{f64::from(range.start_col)*m.width}px;top:{f64::from(range.row)*m.height}px;width:{f64::from(range.end_col-range.start_col)*m.width}px;height:{m.height}px;background:{theme.selection_background()};opacity:0.65;"}
            }
            if cursor.visible && *focused.read() {
                div {class:if cursor.blinking {"dxterm-cursor dxterm-blink"} else {"dxterm-cursor"},aria_hidden:"true",style:"position:absolute;pointer-events:none;left:{cursor_x}px;top:{cursor_y}px;background:{theme.cursor()};{cursor_style}"}
            }
            if !preedit.read().is_empty() {
                div {class:"dxterm-preedit",aria_hidden:"true",style:"position:absolute;pointer-events:none;left:{cursor_x}px;top:{cursor_y}px;white-space:pre;text-decoration:underline;background:{theme.background()};", "{preedit}"}
            }
            InputSurface {id:input_id,style:input_style,model,on_input,focus_target,preedit,composing}
            div {class:"dxterm-live",role:"status",aria_live:"polite",aria_atomic:"true",style:"position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);", "{live}"}
        }
    }
}

fn button(button: Option<dioxus::html::input_data::MouseButton>) -> Button {
    use dioxus::html::input_data::MouseButton::*;
    match button {
        Some(Primary) => Button::Left,
        Some(Auxiliary) => Button::Middle,
        Some(Secondary) => Button::Right,
        _ => Button::None,
    }
}
fn handle_key(
    event: KeyboardEvent,
    release: bool,
    mut model: Signal<Terminal>,
    snapshot: Signal<Snapshot>,
    input: EventHandler<Vec<u8>>,
    on_selection: EventHandler<String>,
    writer: TerminalWriter,
) {
    if event.is_composing() {
        return;
    }
    let Some(key) = keyboard::key_event(&event, release) else {
        return;
    };
    let mods = key.modifiers;
    if mods.shift && matches!(key.key, Key::PageUp | Key::PageDown | Key::Home | Key::End) {
        if !release {
            let page = snapshot.peek().rows.len() as i32;
            match key.key {
                Key::PageUp => model.write().scroll(page),
                Key::PageDown => model.write().scroll(-page),
                Key::Home => model.write().scroll(i32::MAX),
                Key::End => model.write().scroll_to_bottom(),
                _ => {}
            }
            writer.request_render();
        }
        event.prevent_default();
        event.stop_propagation();
        return;
    }
    if mods.control || mods.super_key {
        if key.key == Key::Character("`".into()) {
            return;
        }
        if matches!(&key.key,Key::Character(text) if text.eq_ignore_ascii_case("c"))
            && !model.peek().selection_text().is_empty()
        {
            if !release {
                platform::copy(model.peek().selection_text());
            }
            event.prevent_default();
            event.stop_propagation();
            return;
        }
        #[cfg(all(
            any(feature = "desktop", feature = "native"),
            not(target_arch = "wasm32")
        ))]
        if matches!(&key.key,Key::Character(text) if text.eq_ignore_ascii_case("v")) {
            if !release && let Some(text) = platform::read_clipboard() {
                emit(input, model.peek().paste(&text));
            }
            event.prevent_default();
            event.stop_propagation();
            return;
        }
    }
    // On web, let the browser produce the paste event for Ctrl/Cmd+V.
    #[cfg(all(feature = "web", target_arch = "wasm32", not(feature = "native")))]
    if (mods.control || mods.super_key)
        && matches!(&key.key,Key::Character(text) if text.eq_ignore_ascii_case("v"))
    {
        return;
    }
    let bytes = model.peek().encode_key(&key);
    if !bytes.is_empty() {
        event.prevent_default();
        event.stop_propagation();
        if !release {
            let selected = !model.peek().selection_text().is_empty();
            model.write().clear_selection();
            if selected {
                on_selection.call(String::new());
            }
            model.write().scroll_to_bottom();
            writer.request_render();
        }
        emit(input, bytes);
    }
}

#[derive(Props, Clone)]
struct RowProps {
    row: Arc<Row>,
    width: f64,
    height: f64,
}
impl PartialEq for RowProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.row, &other.row)
            && self.width == other.width
            && self.height == other.height
    }
}
#[allow(non_snake_case)]
fn TerminalRow(props: RowProps) -> Element {
    rsx! {div {class:"dxterm-row",style:"height:{props.height}px;white-space:pre;user-select:none;",
        for run in &props.row.runs {
            span {class:if run.style.blink {"dxterm-run dxterm-blink"} else {"dxterm-run"},
                style:run_style(run,props.width),
                "data-col":"{run.col}","data-cells":"{run.cell_width}",
                {run.text.clone()}
            }
        }
    }}
}
fn run_style(run: &dxterm_core::StyleRun, width: f64) -> String {
    let s = &run.style;
    let mut decoration = if s.strike {
        "line-through ".to_string()
    } else {
        String::new()
    };
    if s.underline != Underline::None {
        decoration.push_str("underline");
    }
    let underline = match s.underline {
        Underline::Double => "double",
        Underline::Curly => "wavy",
        Underline::Dotted => "dotted",
        Underline::Dashed => "dashed",
        _ => "solid",
    };
    format!(
        "display:inline-block;vertical-align:top;box-sizing:border-box;width:{}px;color:{};background:{};font-weight:{};font-style:{};opacity:{};text-decoration-line:{};text-decoration-style:{underline};text-decoration-color:{};{}",
        f64::from(run.cell_width) * width,
        if s.hidden {
            &s.background
        } else {
            &s.foreground
        },
        s.background,
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" },
        if s.dim { "0.66" } else { "1" },
        if decoration.is_empty() {
            "none"
        } else {
            &decoration
        },
        if s.hidden {
            &s.background
        } else {
            s.underline_color.as_deref().unwrap_or("currentColor")
        },
        if run.hyperlink.is_some() {
            "cursor:pointer;"
        } else {
            ""
        }
    )
}

#[cfg(all(any(feature = "web", feature = "desktop"), not(feature = "native")))]
#[component]
fn InputSurface(
    id: String,
    style: String,
    model: Signal<Terminal>,
    on_input: EventHandler<Vec<u8>>,
    mut focus_target: Signal<Option<Rc<MountedData>>>,
    mut preedit: Signal<String>,
    mut composing: Signal<bool>,
) -> Element {
    let input_id = id.clone();
    let end_id = id.clone();
    rsx! {textarea {
        id, class:"dxterm-input",style,aria_label:"Terminal input",spellcheck:"false",autocapitalize:"off",autocomplete:"off",
        onmounted:move |event|focus_target.set(Some(event.data())),
        oncompositionstart:move |_|composing.set(true),
        oncompositionupdate:move |event|preedit.set(event.data().data()),
        oncompositionend:move |event| {composing.set(false);preedit.set(String::new());emit(on_input,event.data().data().into_bytes());platform::clear_textarea(&end_id);},
        oninput:move |event| {if !*composing.peek() {let text=event.value();if !text.is_empty() {emit(on_input,text.into_bytes());}platform::clear_textarea(&input_id);}},
        onpaste:move |event| {if let Some(text)=platform::paste(&event) {event.prevent_default();emit(on_input,model.peek().paste(&text));}},
    }}
}

#[cfg(not(all(any(feature = "web", feature = "desktop"), not(feature = "native"))))]
#[component]
fn InputSurface(
    id: String,
    style: String,
    model: Signal<Terminal>,
    on_input: EventHandler<Vec<u8>>,
    focus_target: Signal<Option<Rc<MountedData>>>,
    preedit: Signal<String>,
    composing: Signal<bool>,
) -> Element {
    let _ = (id, style, model, on_input, focus_target, preedit, composing);
    rsx! {}
}
