type PendingCallback = std::rc::Rc<std::cell::RefCell<Option<Box<dyn FnOnce()>>>>;

use std::{cell::RefCell, rc::Rc};

use dioxus::prelude::*;
use dxform::prelude::*;

#[derive(Clone, PartialEq, Debug, Default)]
struct Hobby {
    name: String,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Person {
    name: String,
    hobbies: Vec<Hobby>,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Profile {
    person: Person,
    active: bool,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct HobbyEntry {
    hobby: Hobby,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct HobbyBook {
    entries: Vec<HobbyEntry>,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Appointment {
    day: i32,
}

fn person_name_field<Root>(scope: &FormScope<Person, Root>) -> FieldHandle<String, Root>
where
    Root: Clone + PartialEq + 'static,
{
    scope.field(FieldSpec::new(
        "name",
        |person: &Person| person.name.clone(),
        |person, value| person.name = value,
    ))
}

fn hobby_name_field<Root>(scope: &FormScope<Hobby, Root>) -> FieldHandle<String, Root>
where
    Root: Clone + PartialEq + 'static,
{
    scope.field(FieldSpec::new(
        "name",
        |hobby: &Hobby| hobby.name.clone(),
        |hobby, value| hobby.name = value,
    ))
}

fn hobby_entry_hobby_scope<Root>(scope: &FormScope<HobbyEntry, Root>) -> FormScope<Hobby, Root>
where
    Root: Clone + PartialEq + 'static,
{
    scope.subform(SubformSpec::new(
        "hobby",
        |entry: &HobbyEntry| entry.hobby.clone(),
        |entry, value| entry.hobby = value,
    ))
}

fn run_in_runtime(f: impl FnOnce() + 'static) {
    #[derive(Clone)]
    struct TestProps {
        f: PendingCallback,
    }

    fn app(props: TestProps) -> Element {
        if let Some(f) = props.f.borrow_mut().take() {
            f();
        }
        rsx! {}
    }

    let f = Rc::new(RefCell::new(Some(Box::new(f) as Box<dyn FnOnce()>)));
    let mut dom = VirtualDom::new_with_props(app, TestProps { f });
    dom.rebuild_to_vec();
}

#[test]
fn dynamic_field_removal_discards_descendants_and_preserves_reset() {
    run_in_runtime(|| {
        let form = FormRoot::new(std::collections::BTreeMap::from([(
            "extra".to_string(),
            "initial".to_string(),
        )]));
        let scope = form.scope();
        let extra = scope.field(FieldSpec::new(
            "extra",
            |values: &std::collections::BTreeMap<String, String>| {
                values.get("extra").cloned().unwrap_or_default()
            },
            |values, value| {
                values.insert("extra".to_string(), value);
            },
        ));
        let leaf = extra.scope().field(FieldSpec::new(
            "value",
            |value: &String| value.clone(),
            |parent, value| *parent = value,
        ));
        leaf.set_value("edited".to_string());
        scope.unregister_field("extra");
        scope.update_value(|values| {
            values.remove("extra");
        });
        assert!(
            form.values().is_empty(),
            "unmounted child appliers must not resurrect removed values"
        );
        assert!(form.meta().dirty);

        scope.update_value(|values| {
            values.insert("new".to_string(), "added".to_string());
        });
        assert_eq!(form.values().get("new").map(String::as_str), Some("added"));
        form.reset();
        assert_eq!(
            form.values().get("extra").map(String::as_str),
            Some("initial")
        );
        assert!(!form.values().contains_key("new"));
        assert!(!form.meta().dirty);
    });
}

#[test]
fn field_change_marks_root_dirty_and_reset_clears_it() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let scope = form.scope();
        let name = person_name_field(&scope);

        name.set_value("Ada".to_string());

        assert_eq!(form.values().name, "Ada");
        assert!(name.meta().dirty);
        assert!(form.meta().dirty);

        name.reset();

        assert_eq!(form.values().name, "");
        assert!(!name.meta().dirty);
        assert!(!form.meta().dirty);
    });
}

#[test]
fn field_value_and_meta_signals_are_live_and_field_local() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let scope = form.scope();
        let name = person_name_field(&scope);
        let hobbies = scope.list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));
        let name_value = name.value_signal();
        let name_meta = name.meta_signal();
        let hobbies_meta = hobbies.meta_signal();

        name.set_value("Ada".to_string());

        assert_eq!(name_value.read().as_str(), "Ada");
        assert!(name_meta.read().dirty);
        assert!(!hobbies_meta.read().dirty);
    });
}

#[test]
fn scope_value_signal_updates_for_descendant_changes() {
    run_in_runtime(|| {
        let form = FormRoot::new(Profile::default());
        let person = form.scope().subform(SubformSpec::new(
            "person",
            |profile: &Profile| profile.person.clone(),
            |profile, value| profile.person = value,
        ));
        let person_value = person.value_signal();
        let name = person_name_field(&person);

        name.set_value("Ada".to_string());

        assert_eq!(person_value.read().name, "Ada");
    });
}

#[test]
fn subform_reset_updates_root_but_preserves_sibling_dirty_state() {
    run_in_runtime(|| {
        let form = FormRoot::new(Profile::default());
        let root = form.scope();
        let active = root.field(FieldSpec::new(
            "active",
            |profile: &Profile| profile.active,
            |profile, value| profile.active = value,
        ));
        let person = root.subform(SubformSpec::new(
            "person",
            |profile: &Profile| profile.person.clone(),
            |profile, value| profile.person = value,
        ));
        let name = person_name_field(&person);

        active.set_value(true);
        name.set_value("Ada".to_string());
        assert!(form.meta().dirty);

        person.reset();

        assert_eq!(form.values().person.name, "");
        assert!(form.values().active);
        assert!(form.meta().dirty);
        assert!(!person.meta().dirty);
    });
}

#[test]
fn list_add_remove_clear_updates_root_values_and_meta() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let list = form.scope().list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));

        list.push(Hobby {
            name: "music".to_string(),
        });
        list.push(Hobby {
            name: "climbing".to_string(),
        });

        assert_eq!(form.values().hobbies.len(), 2);
        assert!(list.meta().dirty);
        assert!(form.meta().dirty);

        let keys = list
            .items()
            .into_iter()
            .map(|item| item.key())
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1]);

        let removed = list.remove(0).expect("item should exist");
        assert_eq!(removed.name, "music");
        assert_eq!(form.values().hobbies[0].name, "climbing");

        list.clear();
        assert!(form.values().hobbies.is_empty());
    });
}

#[test]
fn list_key_signal_tracks_list_structure() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let list = form.scope().list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));
        let keys = list.keys_signal();

        list.push(Hobby {
            name: "music".to_string(),
        });
        list.push(Hobby {
            name: "climbing".to_string(),
        });
        let before = keys.read().clone();
        list.swap(0, 1);
        let after_swap = keys.read().clone();
        list.remove(0);
        let after_remove = keys.read().clone();

        assert_eq!(before.len(), 2);
        assert_eq!(after_swap, vec![before[1], before[0]]);
        assert_eq!(after_remove, vec![before[0]]);
    });
}

#[test]
fn reusable_nested_hobby_fields_work_inside_person_hobbies_list() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let hobbies = form.scope().list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));

        hobbies.push(Hobby::default());
        let item = hobbies.items().remove(0);
        let hobby_scope = item.scope();
        let name = hobby_name_field(&hobby_scope);

        name.set_value("gardening".to_string());

        assert_eq!(form.values().hobbies[0].name, "gardening");
        assert!(hobby_scope.meta().dirty);
        assert!(form.meta().dirty);
    });
}

#[test]
fn list_item_scope_reads_after_adding_first_item() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let hobbies = form.scope().list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));

        hobbies.push(Hobby {
            name: "gardening".to_string(),
        });
        let item = hobbies.items().remove(0);
        let scope = item.scope();

        assert_eq!(scope.value().name, "gardening");
        assert_eq!(form.values().hobbies[0].name, "gardening");
    });
}

#[test]
fn nested_subform_inside_added_list_item_uses_list_item_signals() {
    run_in_runtime(|| {
        let form = FormRoot::new(HobbyBook::default());
        let entries = form.scope().list(ListSpec::new(
            "entries",
            |book: &HobbyBook| book.entries.clone(),
            |book, value| book.entries = value,
        ));

        entries.push(HobbyEntry {
            hobby: Hobby {
                name: "gardening".to_string(),
            },
        });
        let entry = entries.items().remove(0);
        let entry_scope = entry.scope();
        let hobby_scope = hobby_entry_hobby_scope(&entry_scope);
        let name = hobby_name_field(&hobby_scope);

        name.set_value("cooking".to_string());
        assert_eq!(form.values().entries[0].hobby.name, "cooking");
        assert_eq!(entry_scope.value().hobby.name, "cooking");
        assert_eq!(hobby_scope.value().name, "cooking");

        hobby_scope.reset();

        assert_eq!(form.values().entries[0].hobby.name, "gardening");
        assert_eq!(entry_scope.value().hobby.name, "gardening");
        assert_eq!(hobby_scope.value().name, "gardening");
    });
}

#[test]
fn submit_receives_values_from_nested_leaf_field_signal() {
    run_in_runtime(|| {
        let submitted = Rc::new(RefCell::new(None));
        let submitted_for_handler = submitted.clone();
        let form = FormRoot::with_options(FormOptions::new(Profile::default()).on_submit(
            SubmitHandler::sync(move |ctx| {
                *submitted_for_handler.borrow_mut() = Some(ctx.values);
                Ok(())
            }),
        ));
        let person = form.scope().subform(SubformSpec::new(
            "person",
            |profile: &Profile| profile.person.clone(),
            |profile, value| profile.person = value,
        ));
        let name_scope = person.subform(SubformSpec::new(
            "name",
            |person: &Person| person.name.clone(),
            |person, value| person.name = value,
        ));
        let value = name_scope.field(FieldSpec::new(
            "value",
            |value: &String| value.clone(),
            |parent, value| *parent = value,
        ));

        value.set_value("Ada".to_string());
        futures::executor::block_on(form.submit()).expect("submit should succeed");

        assert_eq!(
            submitted
                .borrow()
                .as_ref()
                .map(|profile| profile.person.name.as_str()),
            Some("Ada")
        );
    });
}

#[test]
fn field_validation_updates_root_validity() {
    run_in_runtime(|| {
        let form = FormRoot::new(Person::default());
        let name = form.scope().field(
            FieldSpec::new(
                "name",
                |person: &Person| person.name.clone(),
                |person, value| person.name = value,
            )
            .validator(FieldValidator::sync(
                |ctx: dxform::FieldValidationContext<Person, String>| {
                    if ctx.value.trim().is_empty() {
                        vec![FormError::field(ctx.path, "name is required")]
                    } else {
                        Vec::new()
                    }
                },
            )),
        );

        let result = futures::executor::block_on(name.validate());

        assert!(result.is_err());
        assert_eq!(name.meta().errors.len(), 1);
        assert_eq!(form.meta().validity, Validity::Invalid);
    });
}

#[test]
fn submit_error_maps_to_nested_list_path() {
    run_in_runtime(|| {
        let form = FormRoot::with_options(
            FormOptions::new(Person {
                hobbies: vec![Hobby::default()],
                ..Person::default()
            })
            .on_submit(SubmitHandler::sync(|_ctx| {
                Err(SubmitError::errors(vec![FormError::field(
                    "hobbies[0].name",
                    "invalid hobby",
                )]))
            })),
        );
        let hobbies = form.scope().list(ListSpec::new(
            "hobbies",
            |person: &Person| person.hobbies.clone(),
            |person, value| person.hobbies = value,
        ));
        let hobby_scope = hobbies.items().remove(0).scope();
        let name = hobby_name_field(&hobby_scope);

        let result = futures::executor::block_on(form.submit());

        assert!(result.is_err());
        assert_eq!(name.meta().submit_errors.len(), 1);
        assert!(form.meta().submit_failed);
    });
}

#[test]
fn transformed_field_retains_invalid_draft_without_changing_root_value() {
    run_in_runtime(|| {
        let submitted = Rc::new(RefCell::new(false));
        let submitted_for_handler = submitted.clone();
        let form = FormRoot::with_options(FormOptions::new(Appointment { day: 1 }).on_submit(
            SubmitHandler::sync(move |_ctx| {
                *submitted_for_handler.borrow_mut() = true;
                Ok(())
            }),
        ));
        let day = form.scope().field(
            FieldSpec::new(
                "day",
                |appointment: &Appointment| appointment.day,
                |appointment, value| appointment.day = value,
            )
            .with_draft(
                |value| value.to_string(),
                |draft: &String| {
                    draft
                        .parse::<i32>()
                        .map_err(|err| FormError::new(format!("invalid day: {err}")))
                },
                |draft| draft.trim().is_empty(),
            ),
        );

        day.set_draft("2".to_string());

        assert_eq!(day.draft(), "2");
        assert_eq!(day.value(), 2);
        assert_eq!(form.values().day, 2);
        assert!(day.meta().errors.is_empty());

        day.set_draft("not-a-day".to_string());

        assert_eq!(day.draft(), "not-a-day");
        assert_eq!(day.value(), 2);
        assert_eq!(form.values().day, 2);
        assert_eq!(day.meta().errors.len(), 1);
        assert_eq!(form.meta().errors.len(), 1);

        let result = futures::executor::block_on(form.submit());

        assert!(result.is_err());
        assert!(!*submitted.borrow());
    });
}

#[derive(Clone, Debug, Default, PartialEq)]
struct DraftNumbers {
    count: u32,
    optional: Option<u32>,
}

fn count_draft(
    scope: &FormScope<DraftNumbers, DraftNumbers>,
) -> FieldHandle<u32, DraftNumbers, String> {
    scope.field(
        FieldSpec::new(
            "count",
            |value: &DraftNumbers| value.count,
            |value, count| value.count = count,
        )
        .input_draft(),
    )
}

#[test]
fn input_draft_keeps_invalid_text_and_commits_valid_numbers() {
    run_in_runtime(|| {
        let form = FormRoot::new(DraftNumbers::default());
        let field = count_draft(&form.scope());
        field.set_draft("12".into());
        assert_eq!(field.value(), 12);
        field.set_draft("1x".into());
        assert_eq!(field.draft(), "1x");
        assert_eq!(field.value(), 12);
        assert_eq!(form.values().count, 12);
        let errors = field.meta().errors;
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].source, FormErrorSource::Parse);
        // Focus helpers are also available with a non-default draft type.
        let _: EventHandler<FocusEvent> = field.on_focus();
        let _: EventHandler<FocusEvent> = field.on_blur();
        let _: EventHandler<FormEvent> = field.on_input_draft();
        field.set_draft("13".into());
        assert_eq!(field.value(), 13);
        assert!(field.meta().errors.is_empty());
    });
}

#[test]
fn optional_number_input_draft_maps_empty_to_none() {
    run_in_runtime(|| {
        let form = FormRoot::new(DraftNumbers::default());
        let field = form.scope().field(
            FieldSpec::new(
                "optional",
                |value: &DraftNumbers| value.optional,
                |value, n| value.optional = n,
            )
            .input_draft(),
        );
        assert_eq!(field.draft(), "");
        field.set_draft(" 7 ".into());
        assert_eq!(field.value(), Some(7));
        assert_eq!(field.draft(), " 7 ");
        field.set_draft(" \t ".into());
        assert_eq!(field.value(), None);
        assert!(field.meta().empty);
        assert!(field.meta().errors.is_empty());
    });
}

#[test]
fn text_draft_supports_foreign_values() {
    #[derive(Clone, Debug, PartialEq)]
    enum AppValue {
        Null,
        Int(i64),
    }
    run_in_runtime(|| {
        let form = FormRoot::new(AppValue::Null);
        // The application type has neither Default nor InputValue.
        let spec = FieldSpec {
            name: "value".into(),
            get: Rc::new(|value: &AppValue| value.clone()),
            set: Rc::new(|value, next| *value = next),
            format: Rc::new(|value: &AppValue| value.clone()),
            parse: Rc::new(|value: &AppValue| Ok(value.clone())),
            is_empty: Rc::new(|value: &AppValue| *value == AppValue::Null),
            validators: Vec::new(),
            validation: ValidationStrategy::default(),
        };
        let field = form.scope().field(spec.text_draft(
            |value| match value {
                AppValue::Null => String::new(),
                AppValue::Int(value) => value.to_string(),
            },
            |text| {
                if text.trim().is_empty() {
                    Ok(AppValue::Null)
                } else {
                    text.parse()
                        .map(AppValue::Int)
                        .map_err(|_| FormError::new("Expected an integer"))
                }
            },
        ));
        field.set_draft("42".into());
        assert_eq!(form.values(), AppValue::Int(42));
        field.set_draft("no".into());
        assert_eq!(field.draft(), "no");
        assert_eq!(field.value(), AppValue::Int(42));
        assert_eq!(field.meta().errors[0].source, FormErrorSource::Parse);
    });
}

#[test]
fn effective_validation_is_union_of_field_and_form() {
    run_in_runtime(|| {
        let form = FormRoot::with_options(
            FormOptions::new(String::new()).validation(ValidationStrategy::blur()),
        );
        let field = form.scope().field(
            FieldSpec::new(
                "text",
                |value: &String| value.clone(),
                |value, next| *value = next,
            )
            .validation(ValidationStrategy::change()),
        );
        let strategy = field.validation();
        assert!(strategy.on_change);
        assert!(strategy.on_blur);
        assert!(strategy.on_submit);
        assert!(!strategy.on_mount);
        assert!(field.form() == form);
        let inheriting = form.scope().field(
            FieldSpec::new(
                "other",
                |value: &String| value.clone(),
                |value, next| *value = next,
            )
            .validation(ValidationStrategy::never()),
        );
        assert!(inheriting.validation().on_blur);
        assert!(inheriting.validation().on_submit);
    });
}

#[test]
fn validate_if_runs_only_for_enabled_phases() {
    run_in_runtime(|| {
        let calls = Rc::new(std::cell::Cell::new(0));
        let validator_calls = calls.clone();
        let form = FormRoot::with_options(
            FormOptions::new(String::new()).validation(ValidationStrategy::blur()),
        );
        let field = form.scope().field(
            FieldSpec::new(
                "text",
                |value: &String| value.clone(),
                |value, next| *value = next,
            )
            .validator(FieldValidator::sync(move |ctx| {
                validator_calls.set(validator_calls.get() + 1);
                vec![FormError::field(ctx.path, "Required")]
            })),
        );
        assert!(futures::executor::block_on(field.validate_if(ValidationPhase::Change)).is_none());
        assert_eq!(calls.get(), 0);
        assert!(matches!(
            futures::executor::block_on(field.validate_if(ValidationPhase::Blur)),
            Some(Err(_))
        ));
        assert_eq!(calls.get(), 1);
    });
}

#[test]
fn validate_if_skips_validators_while_parse_error() {
    run_in_runtime(|| {
        let calls = Rc::new(std::cell::Cell::new(0));
        let validator_calls = calls.clone();
        let form =
            FormRoot::with_options(FormOptions::new(0u32).validation(ValidationStrategy::change()));
        let field = form.scope().field(
            FieldSpec::new("value", |value: &u32| *value, |value, next| *value = next)
                .validator(FieldValidator::sync(move |_| {
                    validator_calls.set(validator_calls.get() + 1);
                    Vec::new()
                }))
                .input_draft(),
        );
        field.set_draft("bad".into());
        assert!(futures::executor::block_on(field.validate_if(ValidationPhase::Change)).is_none());
        assert_eq!(calls.get(), 0);
        field.set_draft("2".into());
        assert_eq!(
            futures::executor::block_on(field.validate_if(ValidationPhase::Change)),
            Some(Ok(()))
        );
        assert_eq!(calls.get(), 1);
    });
}

#[test]
fn handle_blur_marks_touched_and_unfocused() {
    run_in_runtime(|| {
        let form = FormRoot::new(DraftNumbers::default());
        let field = count_draft(&form.scope());
        field.handle_focus();
        assert!(field.meta().focused);
        assert!(!field.meta().touched);
        field.handle_blur();
        assert!(!field.meta().focused);
        assert!(field.meta().touched);
    });
}

#[test]
fn form_reset_clears_parse_errors() {
    run_in_runtime(|| {
        let form = FormRoot::new(DraftNumbers {
            count: 5,
            optional: None,
        });
        let field = count_draft(&form.scope());
        field.set_draft("bad".into());
        assert!(!field.meta().errors.is_empty());
        form.reset();
        assert_eq!(field.value(), 5);
        assert_eq!(field.draft(), "5");
        assert!(field.meta().errors.is_empty());
        assert!(form.meta().errors.is_empty());
        assert!(!field.meta().touched);
        assert!(!form.meta().dirty);
    });
}

#[test]
fn scope_reset_clears_parse_errors() {
    run_in_runtime(|| {
        let form = FormRoot::new(Appointment { day: 10 });
        let day = form.scope().field(FieldSpec::new(
            "day",
            |value: &Appointment| value.day,
            |value, day| value.day = day,
        ));
        let leaf = day.scope().field(
            FieldSpec::new("number", |value: &i32| *value, |value, next| *value = next)
                .input_draft(),
        );
        leaf.set_draft("bad".into());
        assert!(!leaf.meta().errors.is_empty());
        day.scope().reset();
        assert_eq!(leaf.draft(), "10");
        assert!(leaf.meta().errors.is_empty());
        assert!(form.meta().errors.is_empty());
        assert!(!leaf.meta().touched);
    });
}
