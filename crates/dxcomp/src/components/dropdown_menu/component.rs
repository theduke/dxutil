use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::dropdown_menu::{
    self, DropdownMenuContentProps, DropdownMenuItemProps, DropdownMenuProps,
    DropdownMenuTriggerProps,
};
use dioxus_primitives::{merge_attributes, use_controlled};

static NEXT_DROPDOWN_ID: AtomicUsize = AtomicUsize::new(0);

#[component]
pub fn DropdownMenu(props: DropdownMenuProps) -> Element {
    let dropdown_id = use_hook(|| NEXT_DROPDOWN_ID.fetch_add(1, Ordering::Relaxed).to_string());
    let (open, set_open) = use_controlled(props.open, props.default_open, props.on_open_change);
    let base = attributes!(div {
        class: "dx-dropdown-menu",
        "data-dx-dropdown-id": dropdown_id.clone(),
    });
    let merged = merge_attributes(vec![props.attributes.clone(), base]);

    use_dropdown_light_dismiss(dropdown_id, set_open);

    rsx! {
        dropdown_menu::DropdownMenu {
            open: open(),
            on_open_change: move |next_open| set_open.call(next_open),
            disabled: props.disabled,
            roving_loop: props.roving_loop,
            attributes: merged,
            {props.children}
        }
    }
}

/// The primitive keeps focus and keyboard behavior inside the menu. This listener
/// adds pointer light-dismiss for non-focusable outside targets, which do not
/// consistently generate a trigger blur in browsers.
fn use_dropdown_light_dismiss(dropdown_id: String, on_dismiss: Callback<bool>) {
    let listener_id = dropdown_id.clone();
    use_effect(move || {
        let listener_id = listener_id.clone();
        spawn(async move {
            let mut eval = document::eval(
                r#"
                const id = await dioxus.recv();
                const key = `semantic-dropdown-${id}`;
                window.__semanticDropdownDismissHandlers ??= new Map();
                const previous = window.__semanticDropdownDismissHandlers.get(key);
                if (previous) document.removeEventListener('pointerdown', previous, true);
                const handler = (event) => {
                    const root = document.querySelector(`[data-dx-dropdown-id="${CSS.escape(id)}"]`);
                    if (root?.dataset.state === 'open' && !root.contains(event.target)) {
                        dioxus.send('dismiss');
                    }
                };
                window.__semanticDropdownDismissHandlers.set(key, handler);
                document.addEventListener('pointerdown', handler, true);
                "#,
            );
            let _ = eval.send(listener_id);
            while eval.recv::<String>().await.is_ok() {
                on_dismiss.call(false);
            }
        });
    });

    use_drop(move || {
        let cleanup = format!(
            r#"
            const id = "{}";
            const handlers = window.__semanticDropdownDismissHandlers;
            const key = `semantic-dropdown-${{id}}`;
            const handler = handlers?.get(key);
            if (handler) document.removeEventListener('pointerdown', handler, true);
            handlers?.delete(key);
            "#,
            dropdown_id
        );
        _ = document::eval(&cleanup);
    });
}

#[component]
pub fn DropdownMenuTrigger(props: DropdownMenuTriggerProps) -> Element {
    let base = attributes!(button {
        class: "dx-dropdown-menu-trigger",
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        dropdown_menu::DropdownMenuTrigger { as: props.r#as, attributes: merged, {props.children} }
    }
}

#[component]
pub fn DropdownMenuContent(props: DropdownMenuContentProps) -> Element {
    let base = attributes!(div {
        class: "dx-dropdown-menu-content",
    });
    let merged = merge_attributes(vec![base, props.attributes.clone()]);

    rsx! {
        dropdown_menu::DropdownMenuContent { id: props.id, attributes: merged, {props.children} }
    }
}

#[component]
pub fn DropdownMenuItem<T: Clone + PartialEq + 'static>(
    props: DropdownMenuItemProps<T>,
) -> Element {
    let base = attributes!(div {
        class: "dx-dropdown-menu-item",
    });
    let merged = merge_attributes(vec![base, props.attributes.clone()]);

    rsx! {
        dropdown_menu::DropdownMenuItem {
            disabled: props.disabled,
            value: props.value,
            index: props.index,
            on_select: props.on_select,
            attributes: merged,
            {props.children}
        }
    }
}
