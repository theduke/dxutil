//! Optional pointer capture; native renderers finish gestures on pointerleave.
use dioxus::prelude::MountedData;

/// Capture ancestor scrolls too: they move the canvas without resizing it.
pub(crate) fn listen_for_scroll(callback: dioxus::prelude::Callback<()>) -> ScrollListener {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        use wasm_bindgen::{JsCast, closure::Closure};
        let listener =
            Closure::wrap(Box::new(move |_: web_sys::Event| callback.call(()))
                as Box<dyn FnMut(web_sys::Event)>);
        let window = web_sys::window();
        if let Some(window) = &window {
            let _ = window.add_event_listener_with_callback_and_bool(
                "scroll",
                listener.as_ref().unchecked_ref(),
                true,
            );
        }
        ScrollListener { window, listener }
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = callback;
        ScrollListener {}
    }
}

pub(crate) struct ScrollListener {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    window: Option<web_sys::Window>,
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    listener: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
impl Drop for ScrollListener {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;
        if let Some(window) = &self.window {
            let _ = window.remove_event_listener_with_callback_and_bool(
                "scroll",
                self.listener.as_ref().unchecked_ref(),
                true,
            );
        }
    }
}

/// Focus without scrolling the page: returning focus to the canvas after a
/// toolbar click must not jump the document.
pub(crate) async fn focus(element: std::rc::Rc<MountedData>) {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = element
            .downcast::<web_sys::Element>()
            .and_then(|element| element.dyn_ref::<web_sys::HtmlElement>())
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = element.focus_with_options(&options);
            return;
        }
    }
    let _ = element.set_focus(true).await;
}
pub fn capture(element: Option<&MountedData>, pointer: i32) {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    if let Some(element) = element.and_then(|data| data.downcast::<web_sys::Element>()) {
        let _ = element.set_pointer_capture(pointer);
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    let _ = (element, pointer);
}
pub fn release(element: Option<&MountedData>, pointer: i32) {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    if let Some(element) = element.and_then(|data| data.downcast::<web_sys::Element>()) {
        let _ = element.release_pointer_capture(pointer);
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    let _ = (element, pointer);
}
pub fn timestamp_ms() -> u64 {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        web_sys::window()
            .and_then(|window| window.performance())
            .map(|clock| clock.now() as u64)
            .unwrap_or(0)
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis() as u64
    }
}

/// Fetch the current client origin synchronously where DOM access is available.
pub(crate) fn client_geometry(
    element: Option<&MountedData>,
) -> Option<(crate::Point, crate::Size)> {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        let element = element?.downcast::<web_sys::Element>()?;
        let rect = element.get_bounding_client_rect();
        Some((
            crate::Point::new(rect.x(), rect.y()),
            crate::Size::new(rect.width(), rect.height()),
        ))
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = element;
        None
    }
}
