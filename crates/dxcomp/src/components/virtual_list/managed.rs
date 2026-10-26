// Adapted from DioxusLabs/dioxus-components at 02801f27e4b3e30606e5c77e435c9d955c708eb3.
// MIT / Apache-2.0; see LICENSE-MIT and LICENSE-APACHE in this directory.
// Local lifecycle fix: explicitly stop the JS bridge and remove both listeners on unmount.
// Remove this compatibility copy after the pinned upstream primitive has equivalent cleanup.
use super::virtualizer::{
    VirtualizerState, VirtualizerStateStoreExt, compute_measurements, get_total_size,
    get_virtual_items, resize_item, set_scroll_offset, set_viewport_size,
};
use dioxus::prelude::*;
use dioxus_primitives::virtual_list::VirtualListProps;
use serde::Deserialize;
use std::collections::HashMap;
#[component]
pub(super) fn ManagedVirtualList(props: VirtualListProps) -> Element {
    let VirtualListProps {
        count,
        buffer,
        estimate_size,
        render_item,
        attributes,
    } = props;

    let container_id =
        use_hook(|| format!("dx-managed-virtual-{}", dioxus::core::current_scope_id().0));
    let bridge = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<document::Eval>)));
    let cleanup_bridge = bridge.clone();
    use_drop(move || {
        if let Some(eval) = cleanup_bridge.borrow_mut().take() {
            let _ = eval.send(());
        }
    });

    // Create the Store — only holds mutable shared state
    let state: Store<VirtualizerState> = use_store(|| VirtualizerState {
        scroll_offset: 0,
        viewport_size: 0,
        is_scrolling: false,
        item_size_cache: HashMap::new(),
        scroll_adjustments: 0,
        stable_total_size: None,
        stable_measurement_count: None,
        deferred_adjustments: 0,
    });

    // Measurements as a memo — recomputes when count or item_size_cache change.
    // Read (not peeked) by the render body so the component re-renders when the
    // memo invalidates; peeking a dirty memo returns stale data (Memo::peek does
    // not check the dirty flag), which can yield out-of-bounds indices when
    // `count` shrinks between renders.
    let measurements: Memo<Vec<super::virtualizer::types::VirtualItem>> = use_memo(move || {
        let count = count();
        let isc = state.item_size_cache();
        let item_size_cache = isc.read();
        let estimate_cb = estimate_size.as_ref().map(|c| move |i: usize| c(i));
        compute_measurements(
            count,
            &item_size_cache,
            estimate_cb.as_ref().map(|f| f as &dyn Fn(usize) -> u32),
        )
    });

    // Scroll requests from Rust callers (see `VirtualListScroller`).
    if let Some(scroller) = try_use_context::<VirtualListScroller>() {
        let request_id = container_id.clone();
        use_effect(move || {
            let Some(index) = scroller.take() else {
                return;
            };
            let offset = measurements.peek().get(index).map(|item| item.start());
            if let Some(offset) = offset {
                let _ = set_scroll_offset(&state, &measurements.peek(), offset, false);
                spawn(sync_container_scroll(request_id.clone(), offset));
            }
        });
    }

    // Subscribe to scroll events via JS bridge
    let bridge_id = container_id.clone();
    use_effect(move || {
        let container_id = bridge_id.clone();
        let script = r#"
            const container = document.getElementById(await dioxus.recv());
            if (!container) return;

            let scrollEndTimer = null;
            let lastOffset = null;
            let lastViewport = null;
            let lastIsScrolling = null;

            function publish(isScrolling) {
                const scroll = Math.round(container.scrollTop);
                const viewport = Math.min(container.clientHeight, window.innerHeight) || 600;
                // Deduplicate only if the full scroll state is unchanged.
                if (
                    scroll === lastOffset &&
                    viewport === lastViewport &&
                    isScrolling === lastIsScrolling
                ) {
                    return;
                }
                lastOffset = scroll;
                lastViewport = viewport;
                lastIsScrolling = isScrolling;
                dioxus.send({
                    offset: scroll,
                    viewport: viewport,
                    isScrolling: isScrolling
                });
            }

            function onScroll() {
                // Clear any pending scroll-end detection
                if (scrollEndTimer !== null) {
                    clearTimeout(scrollEndTimer);
                }

                // Send scroll event immediately (no RAF batching)
                // This ensures Rust receives the event before the next render
                publish(true);

                // Debounce scroll-end detection. Firefox in CI can take long
                // enough between scroll events and measurement reads that a
                // shorter timeout unfreezes the scroll canvas mid-scroll.
                scrollEndTimer = setTimeout(() => {
                    scrollEndTimer = null;
                    publish(false);
                }, 600);
            }

            function onScrollTo(event) {
                const index = event.detail?.index;
                if (Number.isInteger(index) && index >= 0) dioxus.send({offset:Math.round(container.scrollTop),viewport:container.clientHeight || 600,isScrolling:false,index});
            }
            container.addEventListener("dx-virtual-scroll-to", onScrollTo);
            // Initial publish
            publish(false);

            container.addEventListener("scroll", onScroll, { passive: true });
            function onWindowResize() { publish(false); }
            window.addEventListener("resize", onWindowResize, { passive: true });

            try { await dioxus.recv(); } finally {
                if (scrollEndTimer !== null) clearTimeout(scrollEndTimer);
                container.removeEventListener("scroll", onScroll);
                container.removeEventListener("dx-virtual-scroll-to", onScrollTo);
                window.removeEventListener("resize", onWindowResize);
            }
        "#;
        let mut eval = document::eval(script);
        let _ = eval.send(container_id.clone());
        *bridge.borrow_mut() = Some(eval);

        spawn(async move {
            while let Ok(scroll_msg) = eval.recv::<ScrollMsg>().await {
                if let Some(index) = scroll_msg.index {
                    let offset = measurements.peek().get(index).map(|item| item.start());
                    if let Some(offset) = offset {
                        let _ = set_scroll_offset(&state, &measurements.peek(), offset, false);
                        sync_container_scroll(container_id.clone(), offset).await;
                    }
                    continue;
                }
                let scrolling = scroll_msg.is_scrolling;

                let correction = {
                    let m = measurements.peek();
                    set_scroll_offset(&state, &m, scroll_msg.offset, scrolling)
                };
                set_viewport_size(&state, scroll_msg.viewport);

                if let Some(delta) = correction {
                    let new_scroll = (scroll_msg.offset as i32 + delta).max(0) as u32;
                    sync_container_scroll(container_id.clone(), new_scroll).await;
                    state.scroll_offset().set(new_scroll);
                }
            }
        });
    });

    let resize_id = container_id.clone();
    let onresize = move |idx| {
        let container_id = resize_id.clone();
        move |event: Event<ResizeData>| {
            let rect = event.data().get_content_box_size().unwrap_or_default();
            let measured = rect.height.max(1.0).round() as u32;

            let m = measurements.peek();
            let adjustment = resize_item(&state, &m, idx, measured);
            drop(m);

            if let Some(delta) = adjustment {
                let current = *state.scroll_offset().peek();
                let new_scroll = (current as i32 + delta).max(0) as u32;
                let container_id = container_id.clone();
                spawn(async move {
                    sync_container_scroll(container_id, new_scroll).await;
                });
            }
        }
    };

    let m = measurements.read();
    let virtual_items = get_virtual_items(&state, &m, buffer());
    let total_height = get_total_size(&state, &m);

    let top_offset = virtual_items.first().map(|item| item.start()).unwrap_or(0);
    let canvas_height = total_height.max(*state.viewport_size().peek());
    let set_size = count.to_string();

    rsx! {
        div {
            id: container_id,
            role: "list",
            tabindex: "0",
            ..attributes,

            div {
                style: "position: relative; height:{canvas_height}px; width: 100%;",
                div {
                    style: "position: absolute; inset: 0 auto auto 0; width: 100%; transform: translateY({top_offset}px); will-change: transform;",
                    {virtual_items.iter().map(move |item| {
                        let idx = item.index();

                        rsx! {
                            div {
                                key: "{item.key()}",
                                role: "listitem",
                                "data-virtual-index": "{idx}",
                                "aria-setsize": "{set_size}",
                                "aria-posinset": "{idx + 1}",
                                onresize: onresize.clone()(idx),
                                {render_item(idx)}
                            }
                        }
                    })}
                }
            }
        }
    }
}

/// Parsed scroll message from JS bridge.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScrollMsg {
    offset: u32,
    viewport: u32,
    is_scrolling: bool,
    #[serde(default)]
    index: Option<usize>,
}

async fn sync_container_scroll(container_id: String, scroll_top: u32) {
    let eval = document::eval(
        r#"
        const id = await dioxus.recv();
        const targetScroll = await dioxus.recv();
        const container = document.getElementById(id);
        if (container) {
            container.scrollTop = targetScroll;
        }
        "#,
    );
    let _ = eval.send(container_id);
    let _ = eval.send(scroll_top);
}

/// Scrolls a [`super::VirtualList`] rendered below the component that provides it
/// (with [`use_virtual_list_scroller`]) so that a row is rendered and in view.
#[derive(Clone, Copy, PartialEq)]
pub struct VirtualListScroller(Signal<Option<usize>>);

impl VirtualListScroller {
    pub fn scroll_to(mut self, index: usize) {
        self.0.set(Some(index));
    }
    fn take(mut self) -> Option<usize> {
        // Subscribe while idle, but only write when a request needs consuming.
        // Even write_unchecked notifies subscribers, so taking an empty value
        // would keep the consuming effect scheduling itself indefinitely.
        let index = (*self.0.read())?;
        self.0.set(None);
        Some(index)
    }
}

/// Provides a [`VirtualListScroller`] to virtual lists in this component's subtree.
pub fn use_virtual_list_scroller() -> VirtualListScroller {
    use_context_provider(|| VirtualListScroller(Signal::new(None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    #[derive(Clone)]
    struct ScrollerHarness {
        scroller: Rc<RefCell<Option<VirtualListScroller>>>,
        effects: Rc<Cell<usize>>,
        received: Rc<RefCell<Vec<usize>>>,
    }

    fn scroller_harness(props: ScrollerHarness) -> Element {
        let scroller = use_virtual_list_scroller();
        *props.scroller.borrow_mut() = Some(scroller);
        use_effect(move || {
            let effects = props.effects.get() + 1;
            props.effects.set(effects);
            // Fail promptly if consuming an empty request schedules this effect forever.
            assert!(effects <= 8, "scroll request effect did not settle");
            if let Some(index) = scroller.take() {
                props.received.borrow_mut().push(index);
            }
        });
        rsx! {}
    }

    #[test]
    fn scroll_requests_settle_when_idle_and_after_consumption() {
        let props = ScrollerHarness {
            scroller: Rc::new(RefCell::new(None)),
            effects: Rc::new(Cell::new(0)),
            received: Rc::new(RefCell::new(Vec::new())),
        };
        let mut dom = VirtualDom::new_with_props(scroller_harness, props.clone());
        dom.rebuild_in_place();
        for _ in 0..3 {
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        assert_eq!(
            props.effects.get(),
            1,
            "idle scroller must not reschedule itself"
        );
        let scroller = props.scroller.borrow().unwrap();
        for index in [7, 42] {
            scroller.scroll_to(index);
            for _ in 0..3 {
                dom.render_immediate(&mut dioxus::core::NoOpMutations);
            }
        }
        assert_eq!(*props.received.borrow(), vec![7, 42]);
        let settled = props.effects.get();
        for _ in 0..3 {
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        assert_eq!(props.effects.get(), settled);
    }
}
