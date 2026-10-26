# dxform

Typed form state for Dioxus: validation, submission, nested scopes, dynamic lists,
and field drafts. Use it with your own HTML or styled components.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxform = { git = "https://github.com/theduke/dxutil" }
```

```rust
use dioxus::prelude::*;
use dxform::prelude::*;

#[derive(Clone, PartialEq, Default)]
struct Profile {
    name: String,
}

fn App() -> Element {
    let form = use_form_with_options(|| {
        FormOptions::new(Profile::default()).on_submit(SubmitHandler::sync(
            |ctx: SubmitContext<Profile>| {
                let _submitted_profile = ctx.values;
                Ok(())
            },
        ))
    });
    let name = form.scope().field(FieldSpec::new(
        "name",
        |profile: &Profile| profile.name.clone(),
        |profile, value| profile.name = value,
    ));
    rsx! {
        form {
            onsubmit: form.submit_handler(),
            label {
                "Name"
                input {
                    value: name.input_value(),
                    oninput: name.on_input_text(),
                    onfocus: name.on_focus(),
                    onblur: name.on_blur(),
                }
            }
            button {
                r#type: "submit",
                disabled: form.meta().submitting,
                "Save"
            }
        }
    }
}
```

The submit handler receives a typed snapshot. Replace its body with your
persistence operation, or use `SubmitHandler::async_` for asynchronous submission.

## State and validation

Create forms inside a Dioxus component with `use_form` or `use_form_with_options`.
Values must be `Clone + PartialEq + 'static`. A `FieldSpec` supplies a name and
getter/setter functions; its `FieldHandle` exposes values, drafts, and metadata.

- Attach synchronous or asynchronous `FieldValidator`, `ScopeValidator`,
  `ListValidator`, or `FormValidator` instances. `ValidationStrategy` controls
  when validation runs.
- Read metadata for dirty, touched, focus, validation, and submission state.
  Errors carry paths and sources. `form.reset()` restores the initial values.
- Use `SubformSpec` for nested structs and `ListSpec` for dynamic lists with
  stable item keys. Context helpers expose roots, scopes, and list items to
  nested components.
- Use `InputValue` for text conversion and checkbox helpers for booleans.
  `FieldSpec::with_draft` preserves intermediate input through explicit format
  and parse functions.

The crate has no feature flags or required styles. Form state requires a Dioxus
runtime and component scope.

## Examples and tests

- [Validation and async submission](examples/basic.rs)
- [Nested subform](examples/subform.rs)
- [Nested lists](examples/nested_list.rs)

After [workspace setup](../../README.md#development), run
`cargo test -p dxform --locked --quiet --message-format=short`.
