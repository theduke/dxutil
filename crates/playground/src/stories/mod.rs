mod code;
mod components;
mod editor;
mod form;
mod graph;
mod spinner;
mod terminal;

use dioxus::prelude::*;

pub fn render(id: &str) -> Element {
    match id {
        "inputs" => rsx! { components::Inputs {} },
        "dialog" => rsx! { components::DialogStory {} },
        "spinners" => rsx! { spinner::Spinners {} },
        "virtual-list" => rsx! { components::VirtualListStory {} },
        "markdown" => rsx! { editor::Markdown {} },
        "composer" => rsx! { editor::Composer {} },
        "graph" => rsx! { graph::Graph {} },
        "form" => rsx! { form::ProfileForm {} },
        "terminal" => rsx! { terminal::Terminal {} },
        "code" => rsx! { code::Code {} },
        _ => rsx! { components::Buttons {} },
    }
}

pub fn source(id: &str) -> &'static str {
    match id {
        "markdown" | "composer" => include_str!("editor.rs"),
        "graph" => include_str!("graph.rs"),
        "spinners" => include_str!("spinner.rs"),
        "form" => include_str!("form.rs"),
        "terminal" => include_str!("terminal.rs"),
        "code" => include_str!("code.rs"),
        _ => include_str!("components.rs"),
    }
}
