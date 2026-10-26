# dxform

Headless form state for Dioxus: typed values and field drafts, validation,
submission, nested scopes, and dynamic lists. It supplies state and event helpers;
your app supplies HTML, component styling, and persistence.

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

The example submit handler receives the typed snapshot; replace its body with
your persistence operation. Use `SubmitHandler::async_` for a boxed local future.

## Model and validation

`use_form` and `use_form_with_options` create a `FormRoot<T>` inside a Dioxus
component. Values must be `Clone + PartialEq + 'static`. A `FieldSpec` names a
field and supplies typed getter/setter functions. `FieldHandle` exposes its value,
draft, metadata, and event helpers; field-local signals avoid subscribing every
input to the whole form.

Attach `FieldValidator`, `ScopeValidator`, `ListValidator`, or `FormValidator`
instances at the appropriate level. Validators can be synchronous or asynchronous;
`ValidationStrategy` controls when validation runs. Errors carry field paths and
sources, including parsing and submission failures. Metadata tracks dirty,
touched/focused, validation, and submission state. `form.reset()` restores the
initial values and metadata.

Use `SubformSpec` for nested structs and `ListSpec` for lists. List handles retain
stable item keys across edits and expose operations such as insertion, removal,
and reordering. Context helpers (`provide_form_root`, `provide_form_scope`, and
`provide_list_item`) let nested components access the appropriate root or scope.

`InputValue` supplies text conversion for strings, optional strings, and numeric
types; boolean fields provide checkbox helpers. For drafts that must preserve
invalid intermediate input, use `FieldSpec::with_draft` with explicit format and
parse functions.

## Examples and development

- [Basic form with validation and async submission](examples/basic.rs)
- [Nested subform](examples/subform.rs)
- [Nested dynamic lists](examples/nested_list.rs)

There are no crate feature flags, bundled styles, or custom JavaScript runtime.
Choose a Dioxus renderer in the consuming application. Form state still needs a
Dioxus runtime and scope; it is not a standalone non-UI form engine.

From the workspace root, enter `nix develop .#ui` when Nix is available:

```sh
cargo check -p dxform --all-targets --locked --quiet --message-format=short
cargo test -p dxform --locked --quiet --message-format=short
make check-wasm
```
