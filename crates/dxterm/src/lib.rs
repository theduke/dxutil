//! A DOM-row terminal renderer shared by Dioxus web, desktop and Native.
mod keyboard;
mod platform;
mod view;

use dioxus::prelude::*;
pub use dxterm_core::{TerminalLink, TerminalTheme};
pub use view::{TerminalView, TerminalViewProps};

enum Frame {
    Output(Vec<u8>, futures::channel::oneshot::Sender<()>),
    Reset(futures::channel::oneshot::Sender<()>),
}

/// Ordered, awaitable output queue. Await completion before fetching the next
/// transport chunk to apply backpressure. Completion means parsed into the model.
#[derive(Clone, Copy, PartialEq)]
pub struct TerminalWriter {
    frames: Signal<Vec<Frame>>,
}
impl TerminalWriter {
    pub async fn write(&self, bytes: impl AsRef<[u8]>) {
        let (send, wait) = futures::channel::oneshot::channel();
        let mut frames = self.frames;
        frames
            .write()
            .push(Frame::Output(bytes.as_ref().to_vec(), send));
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
