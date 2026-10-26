//! Shared viewport-aware positioning for popup controls.
use dioxus::prelude::*;

/// Attach positioning to a control root. Popups retain their DOM ancestry and
/// primitive keyboard/dismissal behavior while the browser top layer avoids clipping.
pub(crate) fn use_floating(trigger: &'static str, content: &'static str) -> String {
    use_floating_anchor(Some(trigger), content)
}

/// Context menus anchor to the primitive's pointer coordinates instead of an
/// element. They share top-layer rendering, viewport bounds and cleanup.
pub(crate) fn use_context_floating(content: &'static str) -> String {
    use_floating_anchor(None, content)
}

fn use_floating_anchor(trigger: Option<&'static str>, content: &'static str) -> String {
    let id = use_hook(|| format!("dx-floating-{}", dioxus::core::current_scope_id().0));
    let setup_id = id.clone();
    use_effect(move || {
        let eval = document::eval(include_str!("position.js"));
        let _ = eval.send((&setup_id, trigger, content));
    });
    let cleanup_id = id.clone();
    use_drop(move || {
        let eval =
            document::eval("const id = await dioxus.recv(); window.dxFloating?.get(id)?.();");
        let _ = eval.send(cleanup_id);
    });
    id
}
