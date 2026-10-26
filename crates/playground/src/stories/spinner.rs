use dioxus::prelude::*;
use dxcomp::{BlockSpinner, Spinner};

#[component]
pub fn Spinners() -> Element {
    let mut label = use_signal(|| "Loading".to_string());
    let mut inline_size = use_signal(|| "20".to_string());
    let mut block_size = use_signal(|| "32".to_string());

    rsx! {
        div { class: "settings-card",
            label { r#for: "spinner-label", "Accessible label (both examples)" }
            input { class: "demo-input", id: "spinner-label", value: label,
                oninput: move |event| label.set(event.value()),
            }
        }
        div { class: "demo-columns",
            div { class: "state-card",
                h3 { "Spinner · inline" }
                div { class: "controls-strip",
                    label { "Size"
                        select { aria_label: "Spinner size", value: inline_size,
                            onchange: move |event| inline_size.set(event.value()),
                            for size in [16, 20, 24, 32, 48] {
                                option { value: "{size}", "{size}px" }
                            }
                        }
                    }
                }
                p { Spinner { size: inline_size(), label: label() } " Saving changes…" }
                p { class: "muted", "Sits alongside text. Default size: 20px." }
            }
            div { class: "state-card",
                h3 { "BlockSpinner · centered" }
                div { class: "controls-strip",
                    label { "Size"
                        select { aria_label: "BlockSpinner size", value: block_size,
                            onchange: move |event| block_size.set(event.value()),
                            for size in [16, 20, 24, 32, 48] {
                                option { value: "{size}", "{size}px" }
                            }
                        }
                    }
                }
                BlockSpinner { size: block_size(), label: label(), style: "min-height: 160px;" }
                p { class: "muted", "Centers in the available area. Default size: 32px." }
            }
        }
    }
}
