use crate::Viewport;
use dioxus::prelude::*;

#[component]
pub fn GraphBackground(viewport: Signal<Viewport>) -> Element {
    let viewport = viewport();
    rsx! {
        div {
            class: "dxgraph-background",
            aria_hidden: "true",
            style: "--dxg-x:{viewport.x}px;--dxg-y:{viewport.y}px;--dxg-zoom:{viewport.zoom}",
        }
    }
}

/// Only this wrapper subscribes to viewport changes; its children are memoized.
///
/// The world is a compositor layer (`will-change: transform`) only while the
/// view moves: browsers keep such a layer rasterized at the scale it was
/// created with, so text stays blurry after zooming in. Once the view rests
/// the layer is dropped and the content is painted crisply at its zoom.
#[component]
pub(crate) fn GraphWorld(viewport: Signal<Viewport>, children: Element) -> Element {
    let mut moving = use_signal(|| false);
    let mut generation = use_signal(|| 0_u64);
    use_effect(move || {
        let _ = viewport.read();
        let next = generation.peek().wrapping_add(1);
        generation.set(next);
        if !*moving.peek() {
            moving.set(true);
        }
        spawn(async move {
            dioxus_sdk_time::sleep(std::time::Duration::from_millis(MOVING_SETTLE_MS)).await;
            if *generation.peek() == next {
                moving.set(false);
            }
        });
    });
    let viewport = viewport();
    rsx! {
        div {
            class: "dxgraph-world",
            "data-low-zoom": (viewport.zoom < 0.5).to_string(),
            "data-moving": moving().to_string(),
            style: "transform:translate({viewport.x}px,{viewport.y}px) scale({viewport.zoom});transform-origin:0 0;--dxg-zoom:{viewport.zoom}",
            {children}
        }
    }
}

/// How long the view must rest before the world is repainted at its zoom.
const MOVING_SETTLE_MS: u64 = 200;
