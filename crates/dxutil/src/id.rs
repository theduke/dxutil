//! Process-wide unique IDs and stable component IDs.
use dioxus::prelude::use_hook;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Return `"{prefix}-{n}"` using one process-wide counter shared by all prefixes.
pub fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Allocate an ID once and keep it stable for the component's lifetime.
///
/// Later changes to `prefix` do not change the ID.
pub fn use_unique_id(prefix: &str) -> String {
    use_hook(|| unique_id(prefix))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use dioxus::prelude::*;
    use std::{cell::RefCell, collections::HashSet};

    #[test]
    fn prefixed_ids_are_unique_across_calls_and_threads() {
        let ids: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| (0..128).map(|_| unique_id("worker")).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
        for id in ids {
            id.strip_prefix("worker-").unwrap().parse::<u64>().unwrap();
        }
        let a = unique_id("first");
        let b = unique_id("second");
        assert_ne!(a.rsplit('-').next(), b.rsplit('-').next());
        assert!(unique_id("").starts_with('-'));
    }

    thread_local! {
        static IDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn component() -> Element {
        let id = use_unique_id("component");
        IDS.with(|ids| ids.borrow_mut().push(id));
        rsx! { div {} }
    }

    #[test]
    fn hook_is_stable_across_rerenders_and_unique_after_remount() {
        IDS.with(|ids| ids.borrow_mut().clear());
        let mut dom = VirtualDom::new(component);
        dom.rebuild_in_place();
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate_to_vec();
        drop(dom);
        VirtualDom::new(component).rebuild_in_place();
        IDS.with(|ids| {
            let ids = ids.borrow();
            assert_eq!(ids.len(), 3);
            assert_eq!(ids[0], ids[1]);
            assert_ne!(ids[0], ids[2]);
        });
    }
}
