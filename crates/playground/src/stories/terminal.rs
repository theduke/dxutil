use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};
use dxterm::{TerminalTheme, TerminalView, use_terminal_writer};

const LOG: &str = "\r\n\x1b[36m   Compiling\x1b[0m dxutil v0.0.1\r\n\x1b[32m    Finished\x1b[0m demo in 0.42s\r\n\x1b[33m        Note\x1b[0m this output is generated locally\r\n\r\n  ✓ Components ready\r\n  ✓ Keyboard connected\r\n  https://github.com/theduke/dxutil\r\n\r\n\x1b[35mdemo>\x1b[0m ";

#[component]
pub fn Terminal() -> Element {
    let writer = use_terminal_writer();
    let mut font_size = use_signal(|| 14u16);
    let mut dimensions = use_signal(|| (0u16, 0u16));
    let mut event = use_signal(|| "Type to echo input locally".to_string());
    use_effect(move || {
        spawn(async move {
            writer.write(b"\x1b[1;36mdxutil terminal playground\x1b[0m\r\nLocal echo transport. No commands are executed.\r\n\r\ndemo> ").await;
        });
    });
    rsx! {
        div { class: "controls-strip",
            Button { variant: ButtonVariant::Outline, onclick: move |_| async move { writer.write(LOG.as_bytes()).await; event.set("Sample log sent".into()); }, "Send sample log" }
            Button { variant: ButtonVariant::Outline, onclick: move |_| async move { writer.reset().await; writer.write(b"demo> ").await; event.set("Terminal cleared".into()); }, "Clear terminal" }
            label { "Font size"
                select { aria_label: "Font size", value: "{font_size}", onchange: move |e| { if let Ok(size) = e.value().parse() { font_size.set(size); } },
                    option { value: "12", "12 px" } option { value: "14", "14 px" } option { value: "18", "18 px" }
                }
            }
        }
        div { class: "terminal-surface",
            TerminalView {
                writer, font_size: Some(font_size()),
                theme: TerminalTheme { background: Some("#101821".into()), foreground: Some("#e2edf5".into()), cursor: Some("#67e8b3".into()), ..Default::default() },
                on_input: move |input: Vec<u8>| async move {
                    // Local echo handles CR and Backspace; it is deliberately not a shell.
                    let mut echo = Vec::with_capacity(input.len());
                    for &byte in &input {
                        match byte {
                            b'\r' => echo.extend_from_slice(b"\r\n"),
                            127 => echo.extend_from_slice(b"\x08 \x08"),
                            _ => echo.push(byte),
                        }
                    }
                    writer.write(echo).await;
                    event.set(format!("Echoed {} bytes locally", input.len()));
                },
                on_resize: move |size| dimensions.set(size),
                on_selection: move |text: String| event.set(format!("Selected {} characters", text.chars().count())),
                on_link: move |link| event.set(format!("Link activated: {link:?}")),
            }
        }
        div { class: "state-line", role: "status", "{dimensions().0} columns × {dimensions().1} rows · {event}" }
    }
}
