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
            novalidate: true,
            label {
                "Name"
                input {
                    value: name.input_value(),
                    oninput: name.on_input_text(),
                    onfocus: name.on_focus(),
                    onblur: move |_| name.handle_blur(),
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

`FieldSpec::input_draft()` uses `InputValue` formatting and parsing with a string
draft. It preserves invalid input, retains the last valid committed value, and
reports a parse-sourced error. `Option<number>` maps whitespace-only input to
`None`, otherwise parses trimmed text. Existing non-optional numeric conversions
keep their original parsing behavior.

```rust
# use dxform::prelude::*;
# fn example(scope: FormScope<u32, u32>) {
let count = scope.field(
    FieldSpec::new("count", |n: &u32| *n, |n, value| *n = value).input_draft(),
);
// In RSX: value: count.draft(), oninput: count.on_input_draft()
# }
```

Use `FieldSpec::text_draft(format, parse)` for foreign application values, dates,
or UUIDs without implementing `InputValue`. Whitespace-only drafts count as
empty. `FieldSpec::new` needs `Default`; types without it can construct the public
`FieldSpec { .. }` directly and then call `text_draft`.

`set_value`, `set_draft`, `set_focused`, and legacy `on_focus` / `on_blur` only
update state. Event-driven controls should use `handle_draft_change`,
`handle_value_change`, `handle_focus`, and `handle_blur`; `on_input_draft` calls
`handle_draft_change` for string drafts. Focus helpers also work with custom
draft types. Event validation runs in the form owner's scope.

`field.validation()` unions the field and form strategies: a field can add phases,
but cannot remove phases enabled by the form. `validate_if(phase)` returns `None`
when disabled or while a parse error exists; validators would otherwise see the
last valid value. Manual `validate()` and `submit()` retain their behavior;
submission runs all validators. Mount strategy is not automatically wired.
Use `novalidate` on HTML forms with native `required` attributes so dxform owns
validation messages.

Async validation is not protected against stale results. Overlapping runs can
apply older results after a newer value or reset. Prefer synchronous change
validators and avoid concurrent async runs where ordering matters. Field errors
are node-local; form metadata aggregates descendant errors. Reset restores drafts
and clears parse errors, but retains the form's `submit_attempted` flag. Legacy
`on_input_text` drops invalid text and stores its parse failures as validation
errors; use string drafts for parsing controls.

Browser and Dioxus desktop webviews deliver focus/blur/input/submit events.
Native/Blitz delivers input but no focus/blur or Dioxus submit events: touched
comes from input, blur validation never runs, and submission must be invoked by
a click action. Native also has no select widget.

The crate has no feature flags or required styles. Form state requires a Dioxus
runtime and component scope.

## Examples and tests

- [Validation and async submission](examples/basic.rs)
- [Nested subform](examples/subform.rs)
- [Nested lists](examples/nested_list.rs)

After [workspace setup](../../README.md#development), run
`cargo test -p dxform --locked --quiet --message-format=short`.
