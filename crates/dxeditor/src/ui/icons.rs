use dioxus::prelude::*;
#[component]
pub(super) fn Icon(name: String) -> Element {
    let path = match name.as_str() {
        "paragraph" => "M14 20V4H9a5 5 0 0 0 0 10h2M11 4v16M18 4v16",
        "heading" => "M5 4v16M19 4v16M5 12h14",
        "bulleted_list_item" => "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
        "numbered_list_item" => "M9 6h12M9 12h12M9 18h12M3 4h1v4M3 11h2l-2 4h2M3 18h2v3H3",
        "todo" => "M9 11l3 3 8-8M19 12v7H4V4h12",
        "toggle" => "M8 4l9 8-9 8",
        "quote" => "M4 6h6v6H7c0 4-3 6-3 6M14 6h6v6h-3c0 4-3 6-3 6",
        "callout" => "M9 18h6M10 21h4M8 14c-5-5-1-11 4-11s9 6 4 11l-1 2H9z",
        "divider" => "M3 12h18",
        "code_block" => "M8 6l-6 6 6 6M16 6l6 6-6 6M14 3l-4 18",
        "image" => "M3 3h18v18H3zM3 17l6-6 5 5 3-3 4 4M8 7h.01",
        "file" => "M5 2h9l5 5v15H5zM14 2v6h5",
        "bookmark" => "M6 3h12v18l-6-4-6 4z",
        "entity_embed" | "mention" => "M9 9a4 4 0 1 1 6 5V8M15 14c4 0 6-2 6-5a9 9 0 1 0-4 11",
        "page" => "M4 3h16v18H4zM8 8h8M8 12h8M8 16h5",
        "graph" => "M3 3h5v5H3zM16 3h5v5h-5zM10 16h5v5h-5zM8 6h8M6 8l5 8M18 8l-5 8",
        "table" => "M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18",
        _ => "M4 4h16v16H4zM8 12h8M12 8v8",
    };
    rsx! {svg{width:"16",height:"16",view_box:"0 0 24 24",fill:"none",stroke:"currentColor",stroke_width:"1.7",stroke_linecap:"round",stroke_linejoin:"round","aria-hidden":"true","focusable":"false",path{d:path}}}
}
