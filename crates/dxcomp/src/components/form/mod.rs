//! Accessible field layout and controls, with optional `dxform` state bindings.
//!
//! Use explicit IDs when hydrating; generated IDs use a process-wide counter.
//!
//! Presentation works with application-owned signals and no dxform feature:
//! ```rust
//! use dioxus::prelude::*;
//! use dxcomp::form::*;
//!
//! fn PasswordField() -> Element {
//!     let mut value = use_signal(String::new);
//!     rsx! {
//!         Field { id: "password", label: "Password",
//!             PasswordInput { value: value(), oninput: move |event| value.set(event.value()) }
//!         }
//!     }
//! }
//! ```
//!
//! With feature `dxform`, controls bind a typed committed value and a string draft:
//! ```rust
//! # #[cfg(feature = "dxform")]
//! # fn example() {
//! use dioxus::prelude::*;
//! use dxcomp::form::*;
//! use dxform::prelude::*;
//!
//! fn Count() -> Element {
//!     let form = use_form(|| 0u32);
//!     let field = form.scope().field(
//!         FieldSpec::new("count", |n: &u32| *n, |n, value| *n = value).input_draft(),
//!     );
//!     rsx! {
//!         FormField { field: field.clone(), id: "count", label: "Count",
//!             FieldInput { field, inputmode: "numeric" }
//!         }
//!     }
//! }
//! # }
//! ```
//!
//! Bound event handlers use the union of field/form validation strategies. Async
//! validation results can race; avoid overlapping async runs when ordering matters.
//! Native has input events but no focus/blur, select widget, or Dioxus submit event.
//! Use `FormSubmitButton` with `submit_on_click` there, and explicit IDs for hydration.
mod attrs;
mod context;
mod field;
mod ids;
mod password;
mod submit;

pub use attrs::merge_idrefs;
pub use context::*;
pub use field::*;
pub use ids::*;
pub use password::*;
pub use submit::*;

#[cfg(feature = "dxform")]
mod bound;
#[cfg(feature = "dxform")]
pub use bound::*;

#[cfg(test)]
mod tests;
