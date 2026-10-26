use dioxus::prelude::*;
use dxcode::{CodeView, LineTarget};
use dxcomp::button::{Button, ButtonVariant};

fn sample(path: &str) -> &'static str {
    match path {
        "app.ts" => {
            "// A small, typed greeting\ninterface Project { name: string; stars: number }\n\nconst project: Project = { name: 'dxutil', stars: 42 };\nconsole.log(`Hello, ${project.name}!`);\n"
        }
        "package.json" => {
            "{\n  \"name\": \"playground\",\n  \"private\": true,\n  \"components\": [\"button\", \"editor\", \"graph\"],\n  \"version\": 1\n}\n"
        }
        _ => {
            "// A small component with local state\nuse dioxus::prelude::*;\n\n#[component]\nfn Greeting(name: String) -> Element {\n    rsx! { h1 { \"Hello, {name}!\" } }\n}\n"
        }
    }
}

#[component]
pub fn Code() -> Element {
    let mut path = use_signal(|| "main.rs".to_string());
    let mut value = use_signal(|| sample("main.rs").to_string());
    let mut read_only = use_signal(|| false);
    let mut goto = use_signal(|| None::<LineTarget>);
    let mut selection = use_signal(|| "No selection".to_string());
    let mut saves = use_signal(|| 0u64);
    rsx! {
        div { class: "controls-strip",
            label { "Language"
                select { aria_label: "Language", value: path, onchange: move |e| { let next = e.value(); value.set(sample(&next).into()); path.set(next); goto.set(None); },
                    option { value: "main.rs", "Rust" } option { value: "app.ts", "TypeScript" } option { value: "package.json", "JSON" }
                }
            }
            label { class: "check-label", input { r#type: "checkbox", checked: read_only, onchange: move |e| read_only.set(e.checked()) } "Read only" }
            Button { variant: ButtonVariant::Outline, onclick: move |_| goto.set(Some(LineTarget { line: 3, column: Some(1) })), "Go to line 3" }
        }
        div { class: "code-filebar", code { "{path}" } span { "{value().lines().count()} lines" } }
        div { class: "code-surface",
            CodeView {
                value: value(), path: Some(path()), read_only: read_only(), goto: goto(),
                on_change: move |text| value.set(text),
                on_selection: move |selected: dxcode::CodeSelection| selection.set(format!("Lines {}–{} · {} selected characters", selected.start_line, selected.end_line, selected.text.chars().count())),
                on_save: move |_| saves += 1,
            }
        }
        div { class: "state-line", role: "status", "{selection} · {saves} local saves" }
    }
}
