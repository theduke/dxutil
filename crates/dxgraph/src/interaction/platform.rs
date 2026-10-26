//! Optional pointer capture; native renderers finish gestures on pointerleave.
use dioxus::prelude::MountedData;

/// Capture ancestor scrolls too: they move the canvas without resizing it.
pub(crate) fn listen_for_scroll(callback: dioxus::prelude::Callback<()>) -> ScrollListener {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        let listener = web_sys::window().and_then(|window| {
            dxutil::web::EventListener::new(window.as_ref(), "scroll", true, move |_| {
                callback.call(())
            })
            .ok()
        });
        ScrollListener {
            _listener: listener,
        }
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = callback;
        ScrollListener {}
    }
}

pub(crate) struct ScrollListener {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    _listener: Option<dxutil::web::EventListener>,
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
    dxutil::time::now_ms() as u64
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
