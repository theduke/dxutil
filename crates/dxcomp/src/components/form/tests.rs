use std::cell::Cell;

use dioxus::prelude::*;

use super::*;
use super::attrs::{attr, take_control_attrs};

fn render(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn ids_have_suffixes_and_injective_token_encoding() {
    let ids = FieldIds::new("email");
    assert_eq!(ids.control, "email");
    assert_eq!(ids.label, "email-label");
    assert_eq!(ids.description, "email-description");
    assert_eq!(ids.errors, "email-errors");
    let paths = ["a.b", "a_2eb", "a_b", "people[0].name", "é \t"];
    let ids: Vec<_> = paths
        .iter()
        .map(|path| FieldIds::from_path("profile", path))
        .collect();
    for (index, id) in ids.iter().enumerate() {
        assert!(
            id.control
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        );
        assert!(!ids[..index].contains(id));
    }
    assert_eq!(ids[3].control, "profile-people_5b0_5d_2ename");
}

#[test]
fn idrefs_merge_in_order_without_duplicates() {
    assert_eq!(
        merge_idrefs(["  help\t errors", "help extra", " "]),
        Some("help errors extra".into())
    );
    assert_eq!(merge_idrefs(["", " \n\t"]), None);
    assert_eq!(merge_idrefs(Vec::<String>::new()), None);
}

#[test]
fn caller_attributes_are_extracted_and_other_attributes_preserved() {
    let caller = take_control_attrs(vec![
        attr("id", "custom"),
        attr("aria-describedby", "extra"),
        attr("disabled", true),
        attr("required", "false"),
        attr("readonly", "true"),
        attr("aria-invalid", "false"),
        attr("placeholder", "Type here"),
        Attribute::new("id", "namespaced", Some("custom"), false),
    ]);
    assert_eq!(caller.id.as_deref(), Some("custom"));
    assert_eq!(caller.describedby.as_deref(), Some("extra"));
    assert!(caller.disabled);
    assert!(!caller.required);
    assert!(caller.read_only);
    assert_eq!(caller.invalid, Some(false));
    assert_eq!(caller.rest.len(), 2);
    assert_eq!(caller.rest[0].name, "placeholder");
    assert_eq!(caller.rest[1].namespace, Some("custom"));
}

#[component]
fn CustomControl(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
    let (control, rest) = use_field_control(attributes);
    rsx! { input { ..control.aria_attributes(), ..rest } }
}

#[test]
fn field_wires_label_description_visible_errors_and_caller_idrefs() {
    fn app() -> Element {
        rsx! {
            Field { id: "email", label: "Email", description: "Work address", errors: vec!["Invalid address".into()],
                CustomControl { aria_describedby: "extra email-description" }
            }
        }
    }
    let html = render(app);
    for attribute in [
        "for=\"email\"",
        "id=\"email\"",
        "id=\"email-errors\"",
        "aria-live=\"polite\"",
        "aria-describedby=\"email-description email-errors extra\"",
        "aria-invalid=\"true\"",
    ] {
        assert!(html.contains(attribute), "missing {attribute}: {html}");
    }
}

#[test]
fn automatic_ids_are_distinct_and_standalone_controls_get_a_fallback() {
    fn siblings() -> Element {
        rsx! {
            Field { label: "One", CustomControl {} }
            Field { label: "Two", CustomControl {} }
        }
    }
    let html = render(siblings);
    let ids: Vec<_> = html
        .split("for=\"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    for id in ids {
        assert!(html.contains(&format!("id=\"{id}\"")));
    }
    fn standalone() -> Element {
        rsx! { CustomControl {} }
    }
    let html = render(standalone);
    assert!(html.contains("id=\"dx-control-"));
    assert!(!html.contains("aria-describedby"));
}

#[test]
fn custom_layout_uses_labelledby_and_explicit_caller_overrides() {
    fn app() -> Element {
        let state = FieldState {
            labelling: FieldLabelling::LabelledBy,
            has_errors: true,
            disabled: true,
            ..FieldState::new(FieldIds::new("table-control"))
        };
        rsx! {
            FieldProvider { state,
                CustomControl { aria_invalid: "false", aria_describedby: "hint" }
            }
        }
    }
    let html = render(app);
    assert!(html.contains("aria-labelledby=\"table-control-label\""));
    assert!(html.contains("aria-describedby=\"table-control-errors hint\""));
    assert!(html.contains("aria-disabled=\"true\""));
    assert!(!html.contains("aria-invalid=\"true\""));
}

#[test]
fn password_toggle_is_a_button_linked_to_the_hidden_password() {
    fn app() -> Element {
        rsx! { Field { id: "secret", label: "Password",
            PasswordInput { value: "kept", oninput: |_| {} }
        } }
    }
    let html = render(app);
    for attribute in [
        "type=\"password\"",
        "type=\"button\"",
        "aria-controls=\"secret\"",
        "aria-pressed=\"false\"",
        "value=\"kept\"",
    ] {
        assert!(html.contains(attribute), "missing {attribute}: {html}");
    }
}

#[test]
fn pending_submit_uses_shared_spinner_and_pending_label() {
    fn app() -> Element {
        rsx! { SubmitButton { pending: true, pending_label: "Saving", "Save" } }
    }
    let html = render(app);
    assert!(html.contains("disabled"));
    assert!(html.contains("aria-busy=\"true\""));
    assert!(html.contains("dx-spinner"));
    assert!(html.contains("aria-hidden=\"true\""));
    assert!(html.contains("Saving"));
}

#[cfg(feature = "dxform")]
mod bound {
    use super::*;
    use dxform::prelude::*;

    #[test]
    fn visibility_covers_all_modes_and_auto_conditions() {
        let validation = FormError::new("validation");
        let submit = FormError::new("submit").with_source(FormErrorSource::Submission);
        for visibility in [
            ErrorVisibility::Auto,
            ErrorVisibility::Touched,
            ErrorVisibility::AfterSubmit,
            ErrorVisibility::Always,
            ErrorVisibility::Never,
        ] {
            for touched in [false, true] {
                for focused in [false, true] {
                    for attempted in [false, true] {
                        for change in [false, true] {
                            let mut meta = FieldMeta::new(FieldPath::from("value"));
                            meta.touched = touched;
                            meta.focused = focused;
                            meta.errors = vec![validation.clone()];
                            // Repeated submit errors are announced only once.
                            meta.submit_errors = vec![submit.clone(), submit.clone()];
                            let form = FormMeta {
                                submit_attempted: attempted,
                                ..FormMeta::default()
                            };
                            let strategy = if change {
                                ValidationStrategy::change()
                            } else {
                                ValidationStrategy::submit()
                            };
                            let errors = visible_field_errors(&meta, &form, strategy, visibility);
                            let show = match visibility {
                                ErrorVisibility::Auto => {
                                    touched && (!focused || attempted || change)
                                }
                                ErrorVisibility::Touched => touched,
                                ErrorVisibility::AfterSubmit => attempted,
                                ErrorVisibility::Always => true,
                                ErrorVisibility::Never => false,
                            };
                            assert_eq!(errors.contains(&validation), show);
                            assert_eq!(
                                errors.iter().filter(|e| **e == submit).count(),
                                usize::from(visibility != ErrorVisibility::Never)
                            );
                            if show {
                                assert_eq!(errors[0], validation);
                            }
                        }
                    }
                }
            }
        }
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    struct Profile {
        age: u32,
        active: bool,
    }

    #[test]
    fn bound_input_renders_the_draft_path_and_accessible_shell() {
        fn app() -> Element {
            let form = use_form(Profile::default);
            let field = form.scope().field(
                FieldSpec::new("age", |p: &Profile| p.age, |p, value| p.age = value).input_draft(),
            );
            use_hook(|| field.set_draft("1x".into()));
            rsx! {
                FormField { field: field.clone(), id: "age", label: "Age", error_visibility: ErrorVisibility::Always,
                    FieldInput { field, value: "ignored caller value" }
                }
            }
        }
        let html = render(app);
        assert!(!html.contains("ignored caller value"));
        for attribute in [
            "value=\"1x\"",
            "name=\"age\"",
            "for=\"age\"",
            "id=\"age\"",
            "aria-invalid=\"true\"",
            "aria-describedby=\"age-errors\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}: {html}");
        }
    }

    #[test]
    fn password_binding_preserves_explicit_false_invalid_through_its_wrapper() {
        fn app() -> Element {
            let form = use_form(String::new);
            let field = form.scope().field(FieldSpec::new(
                "secret",
                |value: &String| value.clone(),
                |value, next| *value = next,
            ));
            let state = FieldState {
                has_errors: true,
                ..FieldState::new(FieldIds::new("secret"))
            };
            rsx! {
                FieldProvider { state,
                    FieldPasswordInput { field, aria_invalid: "false" }
                    FieldErrorList { messages: vec!["Server message".into()] }
                }
            }
        }
        let html = render(app);
        assert!(!html.contains("aria-invalid=\"true\""));
        assert!(html.contains("aria-describedby=\"secret-errors\""));
        assert!(html.contains("Server message"));
    }

    #[test]
    fn checkbox_label_targets_the_controlled_primitive_button() {
        fn app() -> Element {
            let form = use_form(|| Profile {
                active: true,
                ..Profile::default()
            });
            let field = form.scope().field(FieldSpec::new(
                "active",
                |p: &Profile| p.active,
                |p, value| p.active = value,
            ));
            rsx! { FormField { field: field.clone(), id: "active", label: "Active", FieldCheckbox { field } } }
        }
        let html = render(app);
        for attribute in [
            "role=\"checkbox\"",
            "aria-checked=\"true\"",
            "for=\"active\"",
            "id=\"active\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}: {html}");
        }
    }

    #[test]
    fn native_select_matches_typed_values_by_index_and_handles_absent_values() {
        fn app() -> Element {
            let form = use_form(|| 7u32);
            let field = form.scope().field(FieldSpec::new(
                "value",
                |n: &u32| *n,
                |n, value| *n = value,
            ));
            rsx! {
                FieldNativeSelect { field: field.clone(), options: vec![SelectChoice::new(5u32, "Five"), SelectChoice::new(7u32, "Seven")] }
                FieldNativeSelect { field, placeholder: "Choose", options: vec![SelectChoice::new(5u32, "Five")] }
            }
        }
        let html = render(app);
        let selected = html
            .split("<option")
            .skip(1)
            .filter(|part| part.split('>').next().unwrap().contains("selected"))
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 2);
        assert!(selected[0].contains("value=\"1\""));
        assert!(selected[0].contains("Seven"));
        assert!(selected[1].contains("value=\"\""));
        assert!(selected[1].contains("disabled"));
        assert!(selected[1].contains("Choose"));
    }

    #[test]
    fn click_submit_uses_button_type_and_reset_disables_when_clean() {
        fn app() -> Element {
            let form = use_form(Profile::default);
            rsx! {
                FormSubmitButton { form: form.clone(), submit_on_click: true, "Save" }
                FormResetButton { form, disabled_when_clean: true, "Reset" }
            }
        }
        let html = render(app);
        assert!(!html.contains("type=\"submit\""));
        assert!(html.contains("type=\"button\""));
        assert!(html.split("<button").nth(2).unwrap().contains("disabled"));
    }

    #[test]
    fn foreign_value_editor_works_in_a_table_without_a_field_wrapper() {
        #[derive(Clone, Default, PartialEq)]
        enum AppValue {
            #[default]
            Null,
            Count(u32),
        }
        fn app() -> Element {
            let form = use_form(|| AppValue::Count(8));
            let field = form.scope().field(
                FieldSpec::new("count", |v: &AppValue| v.clone(), |v, next| *v = next)
                    .text_draft(
                        |value| match value {
                            AppValue::Null => String::new(),
                            AppValue::Count(value) => value.to_string(),
                        },
                        |text| {
                            text.parse()
                                .map(AppValue::Count)
                                .map_err(|_| FormError::new("Expected a number"))
                        },
                    ),
            );
            use_hook(|| field.set_draft("bad".into()));
            let ids = FieldIds::from_path("table", field.path().as_str());
            let base = FieldState {
                labelling: FieldLabelling::LabelledBy,
                has_description: true,
                ..FieldState::new(ids.clone())
            };
            let (state, errors) = bound_field_state(&field, base, ErrorVisibility::Auto);
            assert_eq!(errors.len(), 1);
            rsx! {
                table { tbody {
                    FieldProvider { state,
                        tr {
                            th { id: ids.label, scope: "row", "Count" }
                            td {
                                FieldInput { field: field.clone(), name: "custom-count" }
                                FieldDescription { "Application value" }
                                FieldErrors { field, format_error: |error: FormError| format!("Localized: {}", error.message) }
                            }
                        }
                    }
                } }
            }
        }
        let html = render(app);
        for attribute in [
            "id=\"table-count\"",
            "name=\"custom-count\"",
            "value=\"bad\"",
            "aria-labelledby=\"table-count-label\"",
            "aria-describedby=\"table-count-description table-count-errors\"",
            "aria-invalid=\"true\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}: {html}");
        }
        assert!(html.contains("Localized: Expected a number"));
        assert!(!html.contains("class=\"dx-field\""));
    }

    #[test]
    fn nested_list_relative_fields_have_distinct_ids() {
        fn app() -> Element {
            let form = use_form(|| vec![Profile::default(), Profile::default()]);
            let list = form.scope().list(ListSpec::new(
                "people",
                |p: &Vec<Profile>| p.clone(),
                |p, value| *p = value,
            ));
            let fields: Vec<_> = list
                .items()
                .into_iter()
                .map(|item| {
                    item.scope().field(
                        FieldSpec::new("age", |p: &Profile| p.age, |p, age| p.age = age)
                            .input_draft(),
                    )
                })
                .collect();
            assert_ne!(
                FieldIds::from_path("form", fields[0].path().as_str()),
                FieldIds::from_path("form", fields[1].path().as_str())
            );
            rsx! { for field in fields { FormField { field: field.clone(), label: "Age", FieldInput { field } } } }
        }
        let html = render(app);
        let ids: Vec<_> = html
            .split("for=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap())
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        for id in ids {
            assert!(html.contains(&format!("id=\"{id}\"")));
        }
    }
}

#[test]
fn context_updates_reach_controls_with_unchanged_props() {
    thread_local! {
        static INVALID: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
    }
    fn app() -> Element {
        let invalid = use_signal(|| false);
        use_hook(|| INVALID.with(|slot| slot.set(Some(invalid))));
        let state = FieldState {
            has_errors: invalid(),
            ..FieldState::new(FieldIds::new("reactive"))
        };
        rsx! { FieldProvider { state, CustomControl {} } }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    assert!(!dioxus_ssr::render(&dom).contains("aria-invalid"));
    dom.in_runtime(|| {
        INVALID.with(|slot| slot.get().unwrap().set(true));
    });
    dom.render_immediate_to_vec();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("aria-invalid=\"true\""));
    assert!(html.contains("aria-describedby=\"reactive-errors\""));
    dom.in_runtime(|| {
        INVALID.with(|slot| slot.get().unwrap().set(false));
    });
    dom.render_immediate_to_vec();
    assert!(!dioxus_ssr::render(&dom).contains("aria-invalid"));
    INVALID.with(|slot| slot.set(None));
}
