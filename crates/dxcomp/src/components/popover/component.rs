use dioxus::prelude::*;
use dioxus_primitives::popover::{
    self, PopoverContentProps, PopoverRootProps, PopoverTriggerProps,
};
use dioxus_primitives::{dioxus_attributes::attributes, merge_attributes};

#[component]
pub fn PopoverRoot(props: PopoverRootProps) -> Element {
    let floating = crate::floating::use_floating(".dx-popover-trigger", ".dx-popover-content");
    let base = attributes!(div {
        class: "dx-popover",
        "data-dx-floating": floating
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        popover::PopoverRoot {
            is_modal: props.is_modal,
            open: props.open,
            default_open: props.default_open,
            on_open_change: props.on_open_change,
            attributes: merged,
            {props.children}
        }
    }
}

#[component]
pub fn PopoverTrigger(props: PopoverTriggerProps) -> Element {
    let base = attributes!(button {
        class: "dx-popover-trigger"
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        popover::PopoverTrigger { attributes: merged, {props.children} }
    }
}

#[component]
pub fn PopoverContent(props: PopoverContentProps) -> Element {
    let class = if let Some(class) = props.class {
        format!("{} {}", "dx-popover-content", class)
    } else {
        "dx-popover-content".to_string()
    };

    rsx! {
        popover::PopoverContent {
            class,
            id: props.id,
            side: props.side,
            align: props.align,
            attributes: props.attributes,
            {props.children}
        }
    }
}
