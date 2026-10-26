//! Small platform boundary. Blitz never executes JavaScript or registers
//! composition, clipboard or resize handlers (unsupported in Dioxus 0.7).
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn unique_id(prefix: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("{prefix}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

pub(crate) fn now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static EPOCH: std::sync::LazyLock<std::time::Instant> =
            std::sync::LazyLock::new(std::time::Instant::now);
        EPOCH.elapsed().as_secs_f64() * 1000.
    }
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    {
        web_sys::window()
            .and_then(|window| window.performance())
            .map(|performance| performance.now())
            .unwrap_or(0.)
    }
    #[cfg(all(target_arch = "wasm32", not(feature = "web")))]
    {
        0.
    }
}
/// Private frame source shared by every renderer.
pub(crate) async fn next_frame() {
    #[cfg(all(feature = "web", target_arch = "wasm32", not(feature = "native")))]
    if let Some(window) = web_sys::window() {
        use wasm_bindgen::{JsCast, closure::Closure};
        let (send, wait) = futures::channel::oneshot::channel();
        let callback = Closure::once(move |_: f64| {
            let _ = send.send(());
        });
        if let Ok(id) = window.request_animation_frame(callback.as_ref().unchecked_ref()) {
            let _guard = AnimationFrame {
                window,
                id,
                _callback: callback,
            };
            let _ = wait.await;
            return;
        }
    }
    futures_timer::Delay::new(std::time::Duration::from_millis(16)).await;
}
#[cfg(all(feature = "web", target_arch = "wasm32", not(feature = "native")))]
struct AnimationFrame {
    window: web_sys::Window,
    id: i32,
    _callback: wasm_bindgen::closure::Closure<dyn FnMut(f64)>,
}
#[cfg(all(feature = "web", target_arch = "wasm32", not(feature = "native")))]
impl Drop for AnimationFrame {
    fn drop(&mut self) {
        let _ = self.window.cancel_animation_frame(self.id);
    }
}

pub(crate) fn copy(text: String) {
    #[cfg(all(feature = "web", target_arch = "wasm32", not(feature = "native")))]
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(window) = web_sys::window() {
            let _ = wasm_bindgen_futures::JsFuture::from(
                window.navigator().clipboard().write_text(&text),
            )
            .await;
        }
    });
    #[cfg(all(
        any(feature = "desktop", feature = "native"),
        not(target_arch = "wasm32")
    ))]
    with_clipboard(|clipboard| {
        let _ = clipboard.set_text(text);
    });
    #[cfg(not(any(
        all(feature = "web", target_arch = "wasm32", not(feature = "native")),
        all(
            any(feature = "desktop", feature = "native"),
            not(target_arch = "wasm32")
        )
    )))]
    let _ = text;
}

#[cfg(all(
    any(feature = "desktop", feature = "native"),
    not(target_arch = "wasm32")
))]
pub(crate) fn read_clipboard() -> Option<String> {
    with_clipboard(|clipboard| clipboard.get_text().ok()).flatten()
}

#[cfg(all(
    any(feature = "desktop", feature = "native"),
    not(target_arch = "wasm32")
))]
fn with_clipboard<T>(f: impl FnOnce(&mut arboard::Clipboard) -> T) -> Option<T> {
    // X11 clipboard contents must retain their owner after set_text returns.
    thread_local! {static CLIPBOARD:std::cell::RefCell<Option<arboard::Clipboard>>=const {std::cell::RefCell::new(None)};}
    CLIPBOARD.with(|slot| {
        let mut clipboard = slot.borrow_mut();
        if clipboard.is_none() {
            *clipboard = arboard::Clipboard::new().ok();
        }
        clipboard.as_mut().map(f)
    })
}

#[cfg(all(any(feature = "web", feature = "desktop"), not(feature = "native")))]
pub(crate) fn paste(event: &dioxus::prelude::ClipboardData) -> Option<String> {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        use wasm_bindgen::JsCast;
        event
            .downcast::<web_sys::Event>()?
            .dyn_ref::<web_sys::ClipboardEvent>()?
            .clipboard_data()?
            .get_data("text/plain")
            .ok()
    }
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        let _ = event;
        read_clipboard()
    }
    #[cfg(not(any(
        all(feature = "web", target_arch = "wasm32"),
        all(feature = "desktop", not(target_arch = "wasm32"))
    )))]
    {
        let _ = event;
        None
    }
}

#[cfg(all(any(feature = "web", feature = "desktop"), not(feature = "native")))]
pub(crate) fn clear_textarea(id: &str) {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        {
            element.set_value("");
        }
    }
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        // Generated ids contain only a fixed ASCII prefix and a counter.
        let _ =
            dioxus::prelude::document::eval(&format!("document.getElementById('{id}').value = ''"));
    }
    #[cfg(not(any(
        all(feature = "web", target_arch = "wasm32"),
        all(feature = "desktop", not(target_arch = "wasm32"))
    )))]
    let _ = id;
}
