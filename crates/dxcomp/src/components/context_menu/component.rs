use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use dioxus_primitives::context_menu::{
    self, ContextMenuContentProps, ContextMenuItemProps, ContextMenuProps, ContextMenuTriggerProps,
};
use dioxus_primitives::dioxus_attributes::attributes;

#[derive(Clone, Copy)]
struct MenuState {
    set_open: Callback<bool>,
}

#[component]
pub fn ContextMenu(mut props: ContextMenuProps) -> Element {
    let floating = crate::floating::use_context_floating(".dx-context-menu-content");
    props
        .attributes
        .extend(attributes!(div { "data-dx-floating": floating }));
    let parent: Option<MenuState> = try_use_context();
    let (open, set_open) =
        dioxus_primitives::use_controlled(props.open, props.default_open, props.on_open_change);
    use_context_provider(|| MenuState { set_open });

    // Let the trigger handle the event before stopping it at this menu's
    // boundary. Otherwise nested menus also open their ancestor's menu.
    props.attributes.extend([
        oncontextmenu(|event: MouseEvent| event.stop_propagation()),
        onpointerdown(move |event: PointerEvent| {
            // The primitive dismisses outside its entire root. Presses on its
            // trigger are inside that root, but outside the popup.
            // A secondary press repositions the open menu without closing it.
            if event.trigger_button() != Some(MouseButton::Secondary) {
                set_open.call(false);
            }
            if let Some(parent) = parent {
                parent.set_open.call(false);
            }
            event.stop_propagation();
        }),
    ]);

    rsx! {
        context_menu::ContextMenu {
            disabled: props.disabled,
            open: Some(open()),
            on_open_change: move |next_open| {
                if next_open
                    && let Some(parent) = parent {
                        parent.set_open.call(false);
                    }
                set_open.call(next_open);
            },
            roving_loop: props.roving_loop,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn ContextMenuTrigger(props: ContextMenuTriggerProps) -> Element {
    rsx! {
        context_menu::ContextMenuTrigger {
            cursor: "context-menu",
            user_select: "none",
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn ContextMenuContent(mut props: ContextMenuContentProps) -> Element {
    let state: MenuState = use_context();
    props.attributes.extend([
        onmouseleave(move |_| state.set_open.call(false)),
        // Interacting with popup content must not reach the trigger-dismiss
        // handler on the root. Item handlers still run before this boundary.
        onpointerdown(|event: PointerEvent| event.stop_propagation()),
    ]);

    rsx! {
        context_menu::ContextMenuContent {
            class: "dx-context-menu-content",
            id: props.id,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn ContextMenuItem(props: ContextMenuItemProps) -> Element {
    rsx! {
        context_menu::ContextMenuItem {
            class: "dx-context-menu-item",
            disabled: props.disabled,
            value: props.value,
            index: props.index,
            on_select: props.on_select,
            attributes: props.attributes,
            {props.children}
        }
    }
}
