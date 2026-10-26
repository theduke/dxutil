use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};
use dxform::FieldValidationContext;
use dxform::prelude::*;

#[derive(Clone, Debug, Default, PartialEq)]
struct Profile {
    name: String,
    email: String,
}

#[component]
pub fn ProfileForm() -> Element {
    let mut saved = use_signal(|| None::<Profile>);
    let form = use_form_with_options(move || {
        FormOptions::new(Profile::default())
            .validation(ValidationStrategy::blur())
            .on_submit(SubmitHandler::sync(move |ctx: SubmitContext<Profile>| {
                let mut saved = saved;
                saved.set(Some(ctx.values));
                Ok(())
            }))
    });
    let name = form.scope().field(
        FieldSpec::new(
            "name",
            |p: &Profile| p.name.clone(),
            |p, value| p.name = value,
        )
        .validator(FieldValidator::sync(
            |ctx: FieldValidationContext<Profile, String>| {
                if ctx.value.trim().chars().count() < 2 {
                    vec![FormError::field(
                        ctx.path,
                        "Enter a name with at least 2 characters.",
                    )]
                } else {
                    vec![]
                }
            },
        )),
    );
    let email = form.scope().field(
        FieldSpec::new(
            "email",
            |p: &Profile| p.email.clone(),
            |p, value| p.email = value,
        )
        .validator(FieldValidator::sync(
            |ctx: FieldValidationContext<Profile, String>| {
                if !ctx.value.contains('@') {
                    vec![FormError::field(
                        ctx.path,
                        "Enter an email address containing @.",
                    )]
                } else {
                    vec![]
                }
            },
        )),
    );
    let meta = form.meta();
    let reset = form.clone();
    let blur_name = name.clone();
    let blur_email = email.clone();
    rsx! {
        div { class: "demo-columns",
            form { class: "settings-card", onsubmit: form.submit_handler(), novalidate: true,
                h3 { "Your profile" }
                label { r#for: "profile-name", "Display name" }
                input { class: "demo-input", id: "profile-name", autocomplete: "name", value: name.input_value(),
                    oninput: name.on_input_text(), onfocus: name.on_focus(), onblur: move |_| blur_name.handle_blur(),
                    "aria-invalid": !name.meta().errors.is_empty(), "aria-describedby": "name-errors",
                }
                div { id: "name-errors", class: "field-errors", "aria-live": "polite",
                    for error in name.meta().errors { p { "{error.message}" } }
                }
                label { r#for: "profile-email", "Email address" }
                input { class: "demo-input", id: "profile-email", r#type: "email", autocomplete: "email", value: email.input_value(),
                    oninput: email.on_input_text(), onfocus: email.on_focus(), onblur: move |_| blur_email.handle_blur(),
                    "aria-invalid": !email.meta().errors.is_empty(), "aria-describedby": "email-errors",
                }
                div { id: "email-errors", class: "field-errors", "aria-live": "polite",
                    for error in email.meta().errors { p { "{error.message}" } }
                }
                div { class: "toolbar",
                    Button { r#type: "submit", disabled: meta.submitting, "Save profile" }
                    Button { r#type: "button", variant: ButtonVariant::Outline, onclick: move |_| { reset.reset(); saved.set(None); }, "Reset form" }
                }
            }
            div { class: "state-card", h3 { "Form state" }
                dl {
                    dt { "Dirty" } dd { "{meta.dirty}" }
                    dt { "Touched" } dd { "{meta.touched}" }
                    dt { "Validity" } dd { "{meta.validity:?}" }
                    dt { "Submit attempts" } dd { "{meta.submit_count}" }
                }
                div { role: "status", class: "saved-profile",
                    if let Some(profile) = saved() { strong { "Saved locally" } p { "{profile.name}" } p { "{profile.email}" } }
                    else { p { class: "muted", "No profile saved yet." } }
                }
            }
        }
    }
}
