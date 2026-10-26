//! Browser event listeners that unregister on drop.
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, EventTarget};
/// A listener has exactly the lifetime of its host, including capture listeners.
pub struct EventListener {
    target: EventTarget,
    name: String,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}
impl EventListener {
    /// Register a callback, retaining the target and capture flag for removal.
    ///
    /// Returns the browser's error if registration fails.
    pub fn new(
        target: &EventTarget,
        name: &str,
        capture: bool,
        f: impl FnMut(Event) + 'static,
    ) -> Result<Self, JsValue> {
        let callback = Closure::wrap(Box::new(f) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback_and_bool(
            name,
            callback.as_ref().unchecked_ref(),
            capture,
        )?;
        Ok(Self {
            target: target.clone(),
            name: name.into(),
            capture,
            callback,
        })
    }
}
impl Drop for EventListener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            &self.name,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}
