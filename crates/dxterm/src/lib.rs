//! Reusable browser PTY view. Output remains bytes, preserving xterm's UTF-8 decoder.
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
const ENGINE: &str = include_str!("../web/dist/terminal.iife.js");
const CSS: &str = include_str!("../web/dist/terminal.iife.css");
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalLink {
    File {
        path: String,
        line: u32,
        column: Option<u32>,
    },
    Url(String),
}
#[derive(Clone, Default, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTheme {
    #[serde(flatten)]
    pub colors: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreground: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_background: Option<String>,
}
enum Frame {
    Output(String, futures::channel::oneshot::Sender<()>),
    Reset(futures::channel::oneshot::Sender<()>),
}
#[derive(Clone, Copy, PartialEq)]
pub struct TerminalWriter {
    frames: Signal<Vec<Frame>>,
}
impl TerminalWriter {
    pub async fn write_base64(&self, data: String) {
        let (send, wait) = futures::channel::oneshot::channel();
        let mut frames = self.frames;
        frames.write().push(Frame::Output(data, send));
        let _ = wait.await;
    }
    pub async fn reset(&self) {
        let (send, wait) = futures::channel::oneshot::channel();
        let mut frames = self.frames;
        frames.write().push(Frame::Reset(send));
        let _ = wait.await;
    }
}
pub fn use_terminal_writer() -> TerminalWriter {
    TerminalWriter {
        frames: use_signal(Vec::new),
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Event {
    Input {
        data: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Selection {
        text: String,
    },
    File {
        path: String,
        line: u32,
        column: Option<u32>,
    },
    Url {
        url: String,
    },
}
#[component]
pub fn TerminalView(
    on_input: EventHandler<String>,
    on_resize: EventHandler<(u16, u16)>,
    on_link: EventHandler<TerminalLink>,
    on_selection: EventHandler<String>,
    writer: TerminalWriter,
    #[props(default)] theme: TerminalTheme,
    #[props(default)] font_family: Option<String>,
    #[props(default)] font_size: Option<u16>,
) -> Element {
    let id = use_hook(|| {
        format!(
            "dxterm-{}",
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    });
    let initial_theme = use_hook(|| theme.clone());
    let mount_id = id.clone();
    use_effect(move || {
        let script = format!(
            "if (!window.SemanticTerminal) {{ {ENGINE}\n window.SemanticTerminal = SemanticTerminal; }} window.SemanticTerminal.mount({}, document.getElementById({}), {}, event => dioxus.send(event));",
            serde_json::to_string(&mount_id).unwrap(),
            serde_json::to_string(&mount_id).unwrap(),
            serde_json::to_string(&initial_theme).unwrap()
        );
        spawn(async move {
            let mut eval = document::eval(&script);
            while let Ok(event) = eval.recv::<Event>().await {
                match event {
                    Event::Input { data } => on_input.call(data),
                    Event::Resize { cols, rows } => on_resize.call((cols, rows)),
                    Event::Selection { text } => on_selection.call(text),
                    Event::File { path, line, column } => {
                        on_link.call(TerminalLink::File { path, line, column })
                    }
                    Event::Url { url } => on_link.call(TerminalLink::Url(url)),
                }
            }
        });
    });
    let appearance_id = id.clone();
    use_effect(use_reactive(
        (&theme, &font_family, &font_size),
        move |(theme, font, size)| {
            let eval = document::eval(
                "const [id,theme,font,size]=await dioxus.recv();window.SemanticTerminal?.setAppearance(id,theme,font,size);",
            );
            let _ = eval.send((appearance_id.clone(), theme, font, size));
        },
    ));
    let write_id = id.clone();
    let mut busy = use_signal(|| false);
    use_effect(move || {
        if busy() || writer.frames.read().is_empty() {
            return;
        }
        let pending = std::mem::take(&mut *writer.frames.write());
        busy.set(true);
        let id = serde_json::to_string(&write_id).unwrap();
        spawn(async move {
            for frame in pending {
                let (script, send) = match frame {
                    Frame::Output(data, send) => (
                        format!(
                            "await window.SemanticTerminal.write({id}, {}); dioxus.send(true);",
                            serde_json::to_string(&data).unwrap()
                        ),
                        send,
                    ),
                    Frame::Reset(send) => (
                        format!("window.SemanticTerminal.reset({id}); dioxus.send(true);"),
                        send,
                    ),
                };
                let _ = document::eval(&script).recv::<bool>().await;
                let _ = send.send(());
            }
            busy.set(false);
        });
    });
    let drop_id = id.clone();
    use_drop(move || {
        let _ = document::eval(&format!(
            "window.SemanticTerminal?.destroy({});",
            serde_json::to_string(&drop_id).unwrap()
        ));
    });
    rsx! { style { "{CSS}" } div { id, class: "dxterm", style: "height:100%;width:100%;min-width:0;min-height:0;", aria_label: "Interactive terminal" } }
}
