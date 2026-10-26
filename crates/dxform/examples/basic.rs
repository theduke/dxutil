use dioxus::prelude::*;
use dxform::prelude::*;
use futures::FutureExt;

#[derive(Clone, PartialEq, Default)]
struct NoteForm {
    title: String,
    body: String,
}

fn main() {
    dioxus::launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    let form = use_form_with_options(|| {
        FormOptions::new(NoteForm::default()).on_submit(SubmitHandler::async_(
            |ctx: SubmitContext<NoteForm>| {
                async move {
                    let _submitted_title = ctx.values.title;
                    Ok(())
                }
                .boxed_local()
            },
        ))
    });
    let scope = form.scope();
    let title = scope.field(
        FieldSpec::new(
            "title",
            |note: &NoteForm| note.title.clone(),
            |note, value| note.title = value,
        )
        .validator(FieldValidator::sync(
            |ctx: dxform::FieldValidationContext<NoteForm, String>| {
                if ctx.value.trim().is_empty() {
                    vec![FormError::field(ctx.path, "Title is required")]
                } else {
                    Vec::new()
                }
            },
        )),
    );
    let body = scope.field(FieldSpec::new(
        "body",
        |note: &NoteForm| note.body.clone(),
        |note, value| note.body = value,
    ));

    rsx! {
        form { onsubmit: form.submit_handler(),
            input {
                value: "{title.input_value()}",
                oninput: title.on_input_text(),
                onblur: title.on_blur(),
            }
            textarea {
                value: "{body.input_value()}",
                oninput: body.on_input_text(),
            }
            button { r#type: "submit", disabled: form.meta().submitting, "Save" }
            for error in form.meta().errors {
                p { "{error.message}" }
            }
        }
    }
}
