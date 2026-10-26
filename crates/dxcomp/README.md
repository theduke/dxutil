# dxcomp

Styled Dioxus components: buttons and inputs, dialogs and menus, layout controls,
date and color pickers, and virtual lists.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

Render `Stylesheet` once near your app root:

```rust
use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};

fn App() -> Element {
    let mut count = use_signal(|| 0);
    rsx! {
        dxcomp::Stylesheet {}
        Button {
            variant: ButtonVariant::Outline,
            onclick: move |_| count += 1,
            "Clicked {count} times"
        }
    }
}
```

Components are grouped in modules such as `button`, `dialog`, `combobox`, and
`virtual_list`. Lower-level primitives are available through `dxcomp::primitives`.

## Styling and interaction

The theme follows `prefers-color-scheme`. Set `data-theme="light"` or
`data-theme="dark"` on the HTML element to override it. Customize shared `--dx-*`
and component CSS variables in your app stylesheet. `dxcomp::CSS` exposes the
stylesheet for applications that manage style injection themselves.

Popups, sidebar hooks, and virtual lists require a DOM-capable renderer with
Dioxus JavaScript evaluation support. Static rendering produces markup without
those interactions.

Give `VirtualList` a scrolling container with a bounded height. For imperative
scrolling, call `use_virtual_list_scroller()` in an ancestor component, then call
`scroll_to(index)` on its returned handle. The hook provides the handle to
descendant lists through Dioxus context.

## Features

No features are enabled by default. Choose the feature matching your renderer:

| Feature | Enables |
| --- | --- |
| `dxform` | Typed field, select, toggle, submit, and reset bindings to dxform |
| `web` | Dioxus web renderer and web primitives |
| `desktop` | Dioxus desktop renderer |
| `fullstack` | Fullstack support |
| `server` | Server and fullstack support |
| `native` | Dioxus native renderer |

## Form fields

`dxcomp::form` is always available. `Field`, `FieldLabel`, `FieldDescription`,
`FieldErrorList`, `PasswordInput`, and `SubmitButton` work with application-owned
signals and do not require dxform. Render `Stylesheet` once for their styles.

```rust
use dioxus::prelude::*;
use dxcomp::form::*;

fn PasswordField() -> Element {
    let mut password = use_signal(String::new);
    rsx! {
        Field { id: "login-password", label: "Password", description: "Use your account password.",
            PasswordInput {
                value: password(),
                oninput: move |event: FormEvent| password.set(event.value()),
                autocomplete: "current-password",
            }
        }
        SubmitButton { pending: false, pending_label: "Signing in…", "Sign in" }
    }
}
```

`Field` stacks label, control, description, and errors. Set `layout:
FieldLayout::Inline` for checkbox or switch rows. For a rich label, omit `label`,
set `labelling: FieldLabelling::Native`, and render `FieldLabel` in children.
`FieldErrorList` remains mounted as a polite live region even when empty.
`SubmitButton` uses the existing `Spinner` while pending, disables itself, and
sets `aria-busy`. Its spinner is decorative; keep any application status
announcement in a separate live region. Password visibility is uncontrolled by
default; `visible` and `on_visible_change` support controlled visibility.

### Typed dxform binding

Enable `dxcomp`'s `dxform` feature and add `dxform` as a direct dependency:

```toml
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web", "dxform"] }
dxform = { git = "https://github.com/theduke/dxutil" }
```

```rust
use dioxus::prelude::*;
use dxcomp::form::*;
use dxform::prelude::*;
use dxform::FieldValidationContext;

#[derive(Clone, Default, PartialEq)]
enum Plan { #[default] Free, Team }

#[derive(Clone, Default, PartialEq)]
struct Signup {
    name: String,
    age: Option<u32>,
    password: String,
    newsletter: bool,
    plan: Plan,
}

fn SignupForm() -> Element {
    let form = use_form_with_options(|| {
        FormOptions::new(Signup::default())
            .validation(ValidationStrategy::blur())
            .on_submit(SubmitHandler::sync(|_ctx: SubmitContext<Signup>| Ok(())))
    });
    let scope = form.scope();
    let name = scope.field(
        FieldSpec::new("name", |s: &Signup| s.name.clone(), |s, v| s.name = v)
            .validator(FieldValidator::sync(|ctx: FieldValidationContext<Signup, String>| {
                if ctx.value.trim().is_empty() {
                    vec![FormError::field(ctx.path, "Enter your name.")]
                } else {
                    vec![]
                }
            })),
    );
    let age = scope.field(
        FieldSpec::new("age", |s: &Signup| s.age, |s, v| s.age = v).input_draft(),
    );
    let password = scope.field(FieldSpec::new(
        "password", |s: &Signup| s.password.clone(), |s, v| s.password = v,
    ));
    let newsletter = scope.field(FieldSpec::new(
        "newsletter", |s: &Signup| s.newsletter, |s, v| s.newsletter = v,
    ));
    let plan = scope.field(FieldSpec::new(
        "plan", |s: &Signup| s.plan.clone(), |s, v| s.plan = v,
    ));
    rsx! {
        form { novalidate: true, onsubmit: form.submit_handler(),
            FormErrors { form: form.clone() }
            FormField { field: name.clone(), id: "signup-name", label: "Name", required: true,
                FieldInput { field: name, autocomplete: "name" }
            }
            FormField { field: age.clone(), id: "signup-age", label: "Age", description: "Optional.",
                FieldInput { field: age, r#type: "text", inputmode: "numeric" }
            }
            FormField { field: password.clone(), id: "signup-password", label: "Password",
                FieldPasswordInput { field: password, autocomplete: "new-password" }
            }
            FormField { field: newsletter.clone(), id: "signup-newsletter", label: "Send updates", layout: FieldLayout::Inline,
                FieldCheckbox { field: newsletter }
            }
            FormField { field: plan.clone(), id: "signup-plan", label: "Plan",
                FieldNativeSelect { field: plan, options: vec![
                    SelectChoice::new(Plan::Free, "Free"),
                    SelectChoice::new(Plan::Team, "Team"),
                ] }
            }
            FormSubmitButton { form: form.clone(), pending_label: "Saving…", "Create account" }
            FormResetButton { form, disabled_when_clean: true, "Reset" }
        }
    }
}
```

`FieldInput<V, R>`, `FieldTextarea<V, R>`, and `FieldPasswordInput<V, R>` accept
`FieldHandle<V, R, String>`: `V` can be any cloneable, comparable application
value. They display `draft()` even if parsing fails, and default `name` to the
field path. Bindings own `value` / `checked`; extended attributes cannot override
them. Native `type="number"` can sanitize partial input to an empty string;
use `type="text"` with `inputmode="decimal"` or `"numeric"` to preserve it.
Caller input/focus/blur handlers run after binding. `onchange` on text controls
is forwarded only. `FieldTextarea` also accepts `TextareaVariant`.

`FieldCheckbox`, `FieldSwitch`, and `FieldNativeCheckbox` bind typed bool values.
The styled controls use labelable primitive buttons. `FieldNativeSelect` maps
option indexes to typed values without serializing them; an unmatched value
shows a disabled placeholder. `FieldSelect` accepts `SelectOption::<T>` children
and applies IDs and ARIA to its trigger button. Set `text_value` on options with
non-string values so the trigger label and typeahead have display text.
For optional choices use
`T = Option<X>` with an explicit `None` option. Read-only value controls ignore
changes; text controls also receive native `readonly`.

### Application types and string drafts

`text_draft` avoids trait implementations on foreign types:

```rust
use dxform::prelude::*;

#[derive(Clone, Default, PartialEq)]
enum AppValue { #[default] Null, Int(i64) }

fn integer_field(scope: &FormScope<AppValue, AppValue>) -> FieldHandle<AppValue, AppValue, String> {
    scope.field(
        FieldSpec::new("value", |v: &AppValue| v.clone(), |v, next| *v = next)
            .text_draft(
                |v| match v { AppValue::Null => String::new(), AppValue::Int(n) => n.to_string() },
                |text| {
                    let text = text.trim();
                    if text.is_empty() {
                        Ok(AppValue::Null)
                    } else {
                        text.parse::<i64>().map(AppValue::Int)
                            .map_err(|error| FormError::new(format!("invalid number: {error}")))
                    }
                },
            ),
    )
}
```

Use the same handle with `FormField` and `FieldInput`; no dxcomp trait is needed.
`FieldSpec::new` requires `Default`. If the foreign type lacks it, construct the
public `FieldSpec { .. }` with its format, parse, and emptiness closures, then use
`text_draft`. `set_draft` assigns parse error sources automatically.

### Custom layouts and accessible editors

`FieldProvider` adds no element, so table rows and plugin editors can share the
same IDs and errors without a wrapper div:

```rust
use dioxus::prelude::*;
use dxcomp::form::*;
use dxform::prelude::*;

#[component]
fn NumberRow(field: FieldHandle<u32, u32, String>) -> Element {
    let ids = FieldIds::from_path("settings-form", field.path().as_str());
    let base = FieldState {
        labelling: FieldLabelling::LabelledBy,
        has_description: true,
        ..FieldState::new(ids.clone())
    };
    let (state, _) = bound_field_state(&field, base, ErrorVisibility::Auto);
    rsx! {
        FieldProvider { state,
            tr {
                th { id: ids.label, scope: "row", "Count" }
                td {
                    FieldInput { field: field.clone(), inputmode: "numeric" }
                    FieldDescription { "Number of entries." }
                    FieldErrors { field }
                }
            }
        }
    }
}

#[component]
fn CustomEditor(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
    let (control, rest) = use_field_control(attributes);
    rsx! { div { role: "textbox", tabindex: "0", ..control.aria_attributes(), ..rest } }
}
```

Custom editors must implement their own editing, keyboard behavior, and native
interaction flags. `aria_attributes()` sets IDs and ARIA only. With a rich custom
description or errors, set `FieldState.has_description` / `has_errors` to match
what is rendered. Render errors at `ids.errors` using the same visibility policy.

| Attribute | Resolution |
| --- | --- |
| `id` | Caller attribute, context control ID, then per-instance fallback |
| `aria-labelledby` | Caller replaces; otherwise context label for `LabelledBy` |
| `aria-describedby` | Context description and visible errors, then caller IDs; deduplicated |
| `aria-invalid` | Caller explicit value wins; otherwise visible errors |
| disabled, required, read-only | Caller flags OR context flags |

Set IDs on `Field` / `FormField`. A control-level ID overrides context and can
break its label's `for`. Generated IDs are unique per component, including
repeated fields, but use a process-wide counter and are **not hydration-safe**.
Supply explicit IDs or `FieldIds::from_path(form_unique_prefix, field_path)` when
hydrating. Use a different prefix for each form instance; use stable list keys
when reordering items. `merge_idrefs` is also available for app-owned markup.

### Validation and actions

`ErrorVisibility::Auto` shows parse/validation errors once touched and either
unfocused, after a submit attempt, or validating on change. Other policies are
`Touched`, `AfterSubmit`, `Always`, and `Never`. Submit errors always appear unless
`Never`. Touched includes typing before blur. `FormErrors` shows root/form errors
after submit, plus submit errors immediately; `include_field_errors` includes all
paths. Supply `format_error: Callback<FormError, String>` to localize messages.

Bindings call `handle_draft_change`, `handle_value_change`, `handle_focus`, and
`handle_blur`. Field and form validation strategies are unioned. Existing setters
and `on_blur()` / `on_focus()` do not validate. Blur/change validation skips a field
while it has parse errors. Set `novalidate` on forms with native `required`
attributes to let dxform own the messages. `disable_while_submitting` on a
`FormField` is opt-in. `FormSubmitButton` derives pending from form metadata;
`FormResetButton` resets drafts and clears parse errors. Reset retains dxform's
`submit_attempted` flag.

Async validation results are not epoch-guarded: overlapping runs may apply stale
results, including after changes or reset. Prefer synchronous on-change validators
and avoid overlapping async validation; use explicit/submit validation where that
fits the application. Mount strategy is reported but not automatically run.
`FieldMeta.errors` is node-local; render leaf `FieldErrors` or gather `FormMeta`
errors when a custom editor contains descendant fields. Legacy
`dxform::on_input_text` drops invalid text; prefer drafts for parsing controls.

### Renderer support

| Capability | Browser | Desktop webview | Native / Blitz |
| --- | --- | --- | --- |
| Shell, IDs, labels, errors | Yes | Yes | Markup; AT depends on Blitz accessibility |
| Text / textarea drafts | Yes | Yes | Input events work |
| Focus / blur and blur validation | Yes | Yes | No focus events; input marks touched, blur validation never runs |
| Native checkbox | Yes | Yes | Input events work |
| Styled checkbox / switch | Yes | Yes | Click toggling expected; programmatic focus is a no-op, unverified |
| Native select | Yes | Yes | No select widget |
| Styled select | Yes | Yes | Unsupported; floating eval is a no-op |
| Form submit event | Yes | Yes | No Dioxus submit event |
| Password visibility | Yes | Yes | Unverified |

For Native or an action outside a form, use `FormSubmitButton { submit_on_click:
true, .. }`. It uses `type="button"` and directly calls `submit()`; do not combine
it with another submitting form handler. On Native, `Auto` visibility behaves
like `Touched` because fields never receive focus events. Basic form controls
add no JavaScript; styled select inherits the existing floating positioning JS.

## Development

See [workspace setup](../../README.md#development). Run crate tests from the root:

```sh
cargo test -p dxcomp --locked --quiet --message-format=short
```
