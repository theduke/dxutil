//! A DOM-row terminal renderer shared by Dioxus web, desktop and Native.
mod keyboard;
mod platform;
mod view;

use dioxus::prelude::*;
pub use dxterm_core::{TerminalLink, TerminalTheme};
use futures::{future::poll_fn, task::AtomicWaker};
use std::{cell::RefCell, task::Poll};
pub use view::{TerminalView, TerminalViewProps};

enum Frame {
    Output(Vec<u8>, futures::channel::oneshot::Sender<()>),
    Reset(futures::channel::oneshot::Sender<()>),
}

#[derive(Default)]
struct Pending {
    frames: Vec<Frame>,
    render: bool,
    measure: bool,
}
impl Pending {
    fn is_empty(&self) -> bool {
        self.frames.is_empty() && !self.render && !self.measure
    }
}

#[derive(Default)]
struct Queue {
    pending: RefCell<Pending>,
    waker: AtomicWaker,
}

/// Ordered, awaitable output queue. Await completion before fetching the next
/// transport chunk to apply backpressure. Completion means parsed into the model.
#[derive(Clone, Copy, PartialEq)]
pub struct TerminalWriter {
    queue: Signal<Queue>,
}
impl TerminalWriter {
    pub async fn write(&self, bytes: impl AsRef<[u8]>) {
        let (send, wait) = futures::channel::oneshot::channel();
        self.enqueue(Frame::Output(bytes.as_ref().to_vec(), send));
        let _ = wait.await;
    }
    pub async fn reset(&self) {
        let (send, wait) = futures::channel::oneshot::channel();
        self.enqueue(Frame::Reset(send));
        let _ = wait.await;
    }

    fn enqueue(&self, frame: Frame) {
        let queue = self.queue.peek();
        queue.pending.borrow_mut().frames.push(frame);
        queue.waker.wake();
    }

    fn request_render(&self) {
        let queue = self.queue.peek();
        queue.pending.borrow_mut().render = true;
        queue.waker.wake();
    }

    fn request_measure(&self) {
        let queue = self.queue.peek();
        queue.pending.borrow_mut().measure = true;
        queue.waker.wake();
    }

    async fn notified(&self) {
        poll_fn(|context| {
            let queue = self.queue.peek();
            queue.waker.register(context.waker());
            if queue.pending.borrow().is_empty() {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }

    fn take_pending(&self) -> Pending {
        std::mem::take(&mut *self.queue.peek().pending.borrow_mut())
    }
}
pub fn use_terminal_writer() -> TerminalWriter {
    TerminalWriter {
        queue: use_signal(Queue::default),
    }
}
