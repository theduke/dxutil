use dioxus::prelude::*;
use dioxus_primitives::dialog::{
    self, DialogCtx, DialogDescriptionProps, DialogRootProps, DialogTitleProps,
};
use dioxus_primitives::{dioxus_attributes::attributes, merge_attributes, use_controlled};

#[component]
pub fn Dialog(props: DialogRootProps) -> Element {
    let (open, set_open) = use_controlled(props.open, props.default_open, props.on_open_change);
    let backdrop = attributes!(div {
        class: "dx-dialog-backdrop",
    });
    // Keep clicks on non-interactive content focused inside the dialog.
    // Otherwise the outside-focus dismissal can close it when a heading or padding is clicked.
    let content = attributes!(div {
        class: "dx-dialog",
        tabindex: "-1"
    });
    let content = merge_attributes(vec![props.attributes, content]);

    rsx! {
        dialog::DialogRoot {
            id: props.id,
            is_modal: props.is_modal,
            open: open(),
            on_open_change: move |next_open| set_open.call(next_open),
            attributes: backdrop,
            dialog::DialogContent {
                class: None,
                attributes: content,
                {props.children}
            }
        }
    }
}

/// A close button connected to its nearest [`Dialog`].
///
/// Keeping dismissal in the dialog context means callers do not need to duplicate
/// state-reset logic for pointer, keyboard, and explicit close interactions.
#[component]
pub fn DialogClose(
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    r#as: Option<Callback<Vec<Attribute>, Element>>,
    children: Element,
) -> Element {
    let ctx: DialogCtx = use_context();
    let base = attributes!(button {
        class: "dx-dialog-close",
        r#type: "button",
        onclick: move |_| ctx.set_open(false),
    });
    let merged = merge_attributes(vec![base, attributes]);

    if let Some(dynamic) = r#as {
        dynamic.call(merged)
    } else {
        rsx! {
            button { ..merged, {children} }
        }
    }
}

#[component]
pub fn DialogTitle(props: DialogTitleProps) -> Element {
    let base = attributes!(h2 {
        class: "dx-dialog-title",
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        dialog::DialogTitle {
            id: props.id,
            attributes: merged,
            {props.children}
        }
    }
}

#[component]
pub fn DialogDescription(props: DialogDescriptionProps) -> Element {
    let base = attributes!(p {
        class: "dx-dialog-description",
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        dialog::DialogDescription {
            id: props.id,
            attributes: merged,
            {props.children}
        }
    }
}
