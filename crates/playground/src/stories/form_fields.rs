use dioxus::prelude::*;
use dxcomp::form::*;
use dxcomp::{SelectOption, TextareaVariant};
use dxform::FieldValidationContext;
use dxform::prelude::*;

#[derive(Clone, Debug, Default, PartialEq)]
enum Plan {
    #[default]
    Free,
    Team,
}

// An application value edited through a string draft without an InputValue implementation.
#[derive(Clone, Debug, Default, PartialEq)]
enum AppValue {
    #[default]
    Null,
    Int(i64),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Signup {
    name: String,
    age: Option<u32>,
    password: String,
    newsletter: bool,
    notifications: bool,
    terms: bool,
    bio: String,
    plan: Plan,
    quota: AppValue,
}

#[component]
pub fn FormFields() -> Element {
    let saved = use_signal(|| None::<Signup>);
    let form = use_form_with_options(move || {
        FormOptions::new(Signup::default())
            .validation(ValidationStrategy::blur())
            .on_submit(SubmitHandler::sync(move |ctx: SubmitContext<Signup>| {
                let mut saved = saved;
                saved.set(Some(ctx.values));
                Ok(())
            }))
    });
    let scope = form.scope();
    let name = scope.field(
        FieldSpec::new(
            "name",
            |s: &Signup| s.name.clone(),
            |s, v| s.name = v,
        )
        .validator(FieldValidator::sync(
            |ctx: FieldValidationContext<Signup, String>| {
                if ctx.value.trim().is_empty() {
                    vec![FormError::field(ctx.path, "Enter your name.")]
                } else {
                    Vec::new()
                }
            },
        )),
    );
    let age = scope.field(
        FieldSpec::new("age", |s: &Signup| s.age, |s, v| s.age = v).input_draft(),
    );
    let password = scope.field(FieldSpec::new(
        "password",
        |s: &Signup| s.password.clone(),
        |s, v| s.password = v,
    ));
    let newsletter = scope.field(FieldSpec::new(
        "newsletter",
        |s: &Signup| s.newsletter,
        |s, v| s.newsletter = v,
    ));
    let notifications = scope.field(FieldSpec::new(
        "notifications",
        |s: &Signup| s.notifications,
        |s, v| s.notifications = v,
    ));
    let terms = scope.field(FieldSpec::new(
        "terms",
        |s: &Signup| s.terms,
        |s, v| s.terms = v,
    ));
    let bio = scope.field(FieldSpec::new(
        "bio",
        |s: &Signup| s.bio.clone(),
        |s, v| s.bio = v,
    ));
    let plan = scope.field(FieldSpec::new(
        "plan",
        |s: &Signup| s.plan.clone(),
        |s, v| s.plan = v,
    ));
    let quota = scope.field(
        FieldSpec::new(
            "quota",
            |s: &Signup| s.quota.clone(),
            |s, v| s.quota = v,
        )
        .text_draft(
            |value| match value {
                AppValue::Null => String::new(),
                AppValue::Int(value) => value.to_string(),
            },
            |text| {
                let text = text.trim();
                if text.is_empty() {
                    Ok(AppValue::Null)
                } else {
                    text.parse::<i64>()
                        .map(AppValue::Int)
                        .map_err(|error| FormError::new(format!("invalid number: {error}")))
                }
            },
        ),
    );
    let meta = form.meta();
    rsx! {
        div { class: "demo-columns",
            form { class: "settings-card", novalidate: true, onsubmit: form.submit_handler(),
                h3 { "Create account" }
                FormErrors { form: form.clone() }
                FormField { field: name.clone(), id: "signup-name", label: "Name", required: true,
                    FieldInput { field: name, autocomplete: "name" }
                }
                FormField { field: age.clone(), id: "signup-age", label: "Age", description: "Optional. Invalid text stays visible.",
                    FieldInput { field: age, r#type: "text", inputmode: "numeric" }
                }
                FormField { field: password.clone(), id: "signup-password", label: "Password",
                    FieldPasswordInput { field: password, autocomplete: "new-password" }
                }
                FormField { field: bio.clone(), label: "About you",
                    FieldTextarea { field: bio, variant: TextareaVariant::Outline, rows: "2" }
                }
                FormField { field: newsletter.clone(), label: "Send me updates", layout: FieldLayout::Inline,
                    FieldCheckbox { field: newsletter }
                }
                FormField { field: notifications.clone(), label: "Enable notifications", layout: FieldLayout::Inline,
                    FieldSwitch { field: notifications }
                }
                FormField { field: terms.clone(), label: "Remember preferences", layout: FieldLayout::Inline,
                    FieldNativeCheckbox { field: terms }
                }
                FormField { field: plan.clone(), id: "signup-plan", label: "Plan",
                    FieldNativeSelect { field: plan.clone(), options: vec![
                        SelectChoice::new(Plan::Free, "Free"),
                        SelectChoice::new(Plan::Team, "Team"),
                    ] }
                }
                FormField { field: plan.clone(), label: "Plan (styled)", description: "Shares the same typed value as Plan.",
                    FieldSelect { field: plan,
                        SelectOption::<Plan> { index: 0usize, value: Plan::Free, "Free" }
                        SelectOption::<Plan> { index: 1usize, value: Plan::Team, "Team" }
                    }
                }
                h3 { "Custom table layout" }
                table { tbody { QuotaRow { field: quota } } }
                div { class: "toolbar",
                    FormSubmitButton { form: form.clone(), pending_label: "Saving…", "Save account" }
                    FormResetButton { form: form.clone(), "Reset fields" }
                }
            }
            div { class: "state-card",
                h3 { "Form state" }
                dl {
                    dt { "Dirty" } dd { "{meta.dirty}" }
                    dt { "Touched" } dd { "{meta.touched}" }
                    dt { "Submit attempts" } dd { "{meta.submit_count}" }
                    dt { "Current plan" } dd { "{form.values().plan:?}" }
                }
                div { role: "status",
                    if let Some(value) = saved() {
                        p { "Saved locally: {value.name} ({value.plan:?})" }
                    } else {
                        p { "No account saved yet." }
                    }
                }
            }
        }
    }
}

#[component]
fn QuotaRow(field: FieldHandle<AppValue, Signup, String>) -> Element {
    let ids = FieldIds::from_path("signup", field.path().as_str());
    let base = FieldState {
        labelling: FieldLabelling::LabelledBy,
        has_description: true,
        ..FieldState::new(ids.clone())
    };
    let (state, _) = bound_field_state(&field, base, ErrorVisibility::Auto);
    rsx! {
        FieldProvider { state,
            tr {
                th { id: ids.label, scope: "row", "Quota" }
                td {
                    FieldInput { field: field.clone(), inputmode: "numeric" }
                    FieldDescription { "An application enum with a string draft." }
                    FieldErrors { field }
                }
            }
        }
    }
}
