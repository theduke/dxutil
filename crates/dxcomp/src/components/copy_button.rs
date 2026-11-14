use std::{cell::RefCell, rc::Rc};

use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, Copy};

use super::form::attrs::attr;
use crate::{ButtonSize, ButtonVariant};

/// Platform-independent clipboard operation. Complete with the real outcome,
/// including errors; retaining the request across asynchronous work is supported.
#[derive(Clone, Debug)]
pub struct CopyRequest {
    value: String,
    completion: Callback<Result<(), String>>,
    lifetime: Option<std::rc::Weak<()>>,
}

impl CopyRequest {
    pub fn new(value: String, completion: Callback<Result<(), String>>) -> Self {
        Self {
            value,
            completion,
            lifetime: None,
        }
    }

    /// Exact, untruncated text requested by the user.
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn complete(self, result: Result<(), String>) {
        if self
            .lifetime
            .as_ref()
            .is_some_and(|lifetime| lifetime.upgrade().is_none())
        {
            return;
        }
        self.completion.call(result);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
enum CopyStatus {
    #[default]
    Idle,
    Pending,
    Succeeded,
    Failed(String),
}

#[derive(Default)]
struct CopyState {
    value: String,
    generation: u64,
    status: CopyStatus,
}

impl CopyState {
    fn update_value(&mut self, value: &str) {
        if self.value != value {
            self.value = value.to_owned();
            self.generation = self.generation.wrapping_add(1);
            self.status = CopyStatus::Idle;
        }
    }

    fn begin(&mut self) -> Option<u64> {
        if self.status == CopyStatus::Pending {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.status = CopyStatus::Pending;
        Some(self.generation)
    }

    fn complete(&mut self, generation: u64, result: Result<(), String>) -> bool {
        if generation != self.generation || self.status != CopyStatus::Pending {
            return false;
        }
        self.status = match result {
            Ok(()) => CopyStatus::Succeeded,
            Err(error) => CopyStatus::Failed(error),
        };
        true
    }
}

/// Copy action with pending, success and failure feedback announced in a live region.
/// The application supplies the clipboard implementation through `on_copy`.
/// A changed value resets feedback and invalidates earlier completions, including
/// when the value changes away and back. The button never submits its containing form.
/// Extended attributes (including class) apply to the button, not the live region.
#[component]
pub fn CopyButton(
    value: String,
    #[props(default = "Copy text".to_string())] label: String,
    #[props(default)] disabled: bool,
    #[props(default)] show_label: bool,
    on_copy: EventHandler<CopyRequest>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = button)]
    attributes: Vec<Attribute>,
) -> Element {
    // The operation state is synchronous so two events before a render cannot start
    // duplicate copies. A revision signal only requests rendering after an operation.
    let state = use_hook(|| Rc::new(RefCell::new(CopyState::default())));
    let lifetime = use_hook(|| Rc::new(()));
    let lifetime = Rc::downgrade(&lifetime);
    let mut revision = use_signal(|| 0_u64);
    revision.read();
    let status = {
        let mut state = state.borrow_mut();
        state.update_value(&value);
        state.status.clone()
    };
    let pending = status == CopyStatus::Pending;
    let copied = status == CopyStatus::Succeeded;
    let failed = matches!(status, CopyStatus::Failed(_));
    let feedback = match &status {
        CopyStatus::Idle => String::new(),
        CopyStatus::Pending => "Copying…".to_owned(),
        CopyStatus::Succeeded => "Copied to clipboard.".to_owned(),
        CopyStatus::Failed(_) => {
            "Copy unavailable. Select the text and copy it manually.".to_owned()
        }
    };
    let base = vec![attr("class", "dx-copy-button")];
    let owned = vec![
        attr("type", "button"),
        attr("disabled", disabled || pending),
        attr("aria-busy", pending.to_string()),
        attr("aria-label", if copied { "Copied" } else { &label }),
        attr("title", if copied { "Copied" } else { &label }),
    ];
    let attributes = crate::primitives::merge_attributes(vec![base, attributes, owned]);
    rsx! {
        crate::Button {
            variant: ButtonVariant::Ghost,
            size: if show_label { ButtonSize::Sm } else { ButtonSize::IconSm },
            attributes,
            onclick: move |_| {
                if disabled { return; }
                let Some(generation) = state.borrow_mut().begin() else { return; };
                revision += 1;
                let state = state.clone();
                let completion = Callback::new(move |result| {
                    if state.borrow_mut().complete(generation, result) {
                        if let Ok(mut revision) = revision.try_write() {
                            *revision = revision.wrapping_add(1);
                        }
                    }
                });
                on_copy.call(CopyRequest {
                    value: value.clone(),
                    lifetime: Some(lifetime.clone()),
                    completion,
                });
            },
            span { aria_hidden: "true",
                if pending { crate::Spinner { size: "14", aria_hidden: "true" } }
                else if copied { Check { size: "14" } }
                else { Copy { size: "14" } }
            }
            if show_label { span { "{label}" } }
        }
        span { class: if failed { "dx-copy-feedback" } else { "dx-copy-feedback dx-sr-only" }, role: "status", aria_live: "polite", aria_atomic: "true", "{feedback}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_reject_duplicates_reset_on_value_changes_and_ignore_obsolete_results() {
        for result in [Ok(()), Err("Permission denied".to_owned())] {
            let mut state = CopyState::default();
            state.update_value("original");
            let original = state.begin().unwrap();
            assert_eq!(state.begin(), None);
            state.update_value("replacement");
            assert_eq!(state.status, CopyStatus::Idle);
            state.update_value("original");
            let current = state.begin().unwrap();
            assert!(!state.complete(original, result.clone()));
            assert_eq!(state.status, CopyStatus::Pending);
            assert!(state.complete(current, result.clone()));
            assert!(!state.complete(current, Ok(())));
            assert_eq!(
                state.status,
                match result {
                    Ok(()) => CopyStatus::Succeeded,
                    Err(error) => CopyStatus::Failed(error),
                }
            );
            state.update_value("new text");
            assert_eq!(state.status, CopyStatus::Idle);
            assert!(state.begin().is_some());
        }
    }
}
