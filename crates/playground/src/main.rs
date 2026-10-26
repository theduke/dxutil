mod catalog;
mod stories;

use catalog::{GROUPS, STORIES};
use dioxus::prelude::*;

fn main() {
    #[cfg(any(feature = "web", feature = "desktop"))]
    dioxus::launch(App);
    #[cfg(not(any(feature = "web", feature = "desktop")))]
    {
        let _ = App;
        println!("Use make serve-playground, or make serve-playground-desktop.");
    }
}

#[derive(Clone, PartialEq, Routable)]
enum Route {
    #[route("/?:story")]
    Playground { story: String },
}

#[component]
fn App() -> Element {
    rsx! {
        dxcomp::Stylesheet {}
        dxgraph::Stylesheet {}
        style { {include_str!("playground.css")} }
        Router::<Route> {}
    }
}

#[component]
fn Playground(story: String) -> Element {
    let mut query = use_signal(String::new);
    let mut dark = use_signal(|| false);
    let mut revision = use_signal(|| 0u64);
    let mut narrow = use_signal(|| false);
    use_effect(move || {
        let theme = if dark() { "dark" } else { "light" };
        // Dioxus document evaluation works in both WASM and the desktop webview.
        let _ = document::eval(&format!(
            "document.documentElement.dataset.theme = '{theme}';"
        ));
    });
    let selected = STORIES.iter().find(|item| item.id == story);
    let current = selected.unwrap_or(&STORIES[0]);
    let search = query().to_lowercase();
    let matches = |item: &&catalog::Story| {
        format!("{} {} {}", item.group, item.title, item.description)
            .to_lowercase()
            .contains(&search)
    };
    let result_count = STORIES.iter().filter(matches).count();
    rsx! {
        a { class: "skip-link", href: "#story-content", "Skip to story" }
        div { class: "shell",
            aside { class: "sidebar",
                a { class: "brand", href: "https://github.com/theduke/dxutil", "dx" span { "util" } }
                p { class: "sidebar-caption", "COMPONENT PLAYGROUND" }
                label { class: "search-label", r#for: "story-search", "Find a story" }
                input {
                    id: "story-search", class: "search", r#type: "search", placeholder: "Search components…",
                    value: query, oninput: move |event| query.set(event.value()),
                }
                nav { "aria-label": "Component stories",
                    for (group, description) in GROUPS {
                        if STORIES.iter().filter(|item| item.group == *group).any(|item| matches(&item)) {
                            section { class: "nav-group",
                                h2 { "{group}" }
                                p { "{description}" }
                                for item in STORIES.iter().filter(|item| item.group == *group).filter(matches) {
                                    Link {
                                        to: Route::Playground { story: item.id.into() },
                                        class: if item.id == current.id { "story-link active" } else { "story-link" },
                                        "aria-current": if item.id == current.id { "page" } else { "false" },
                                        span { class: "story-dot", "" } "{item.title}"
                                    }
                                }
                            }
                        }
                    }
                    if result_count == 0 { p { role: "status", "No stories match. Try a crate name." } }
                }
                p { class: "sidebar-footer", "{STORIES.len()} stories · 6 crates" }
            }
            div { class: "workspace",
                header { class: "topbar",
                    span { "Rust + Dioxus" }
                    div { class: "toolbar",
                        span { class: "platform-tag", if cfg!(target_arch = "wasm32") { "Web / WASM" } else { "Desktop webview" } }
                        button { class: "plain-button", "aria-pressed": dark(), onclick: move |_| dark.toggle(),
                            if dark() { "Light theme" } else { "Dark theme" }
                        }
                        a { href: "https://github.com/theduke/dxutil", "GitHub ↗" }
                    }
                }
                main { id: "story-content", tabindex: "-1",
                    if selected.is_none() && !story.is_empty() {
                        p { role: "status", "Unknown story. Showing Buttons; choose another from the navigation." }
                    }
                    div { class: "story-heading",
                        div {
                            p { class: "eyebrow", "{current.group} / {current.category}" }
                            h1 { "{current.title}" }
                            p { class: "description", "{current.description}" }
                        }
                        a { class: "api-link", href: "https://github.com/theduke/dxutil/tree/main/crates/{current.group}", "Crate API ↗" }
                    }
                    section { class: "preview-panel", "aria-label": "Interactive preview",
                        div { class: "preview-toolbar",
                            span { class: "preview-label", span { class: "live-dot" } "LIVE PREVIEW" }
                            div { class: "toolbar",
                                button { class: "plain-button", "aria-pressed": narrow(), onclick: move |_| narrow.toggle(),
                                    if narrow() { "Full width" } else { "Narrow preview" }
                                }
                                button { class: "plain-button", onclick: move |_| revision += 1, "Reset story" }
                            }
                        }
                        div { class: if narrow() { "preview narrow" } else { "preview" },
                            for instance in [format!("{}-{}", current.id, revision())] {
                                div { key: "{instance}", class: "story-mount", {stories::render(current.id)} }
                            }
                        }
                    }
                    div { class: "story-notes",
                        div { h2 { "Try it out" } p { "{current.instructions}" } }
                        div { h2 { "Integration notes" } p { "{current.notes}" } }
                    }
                    details { class: "source-panel",
                        summary { "Story source · Rust" }
                        pre { code { "{stories::source(current.id)}" } }
                    }
                    footer { class: "page-footer", "Real components. Local state. No account or backend required." }
                }
            }
        }
    }
}
