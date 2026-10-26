use std::rc::Rc;

use dioxus::prelude::*;
use dxform::prelude::*;

#[derive(Clone, PartialEq, Default)]
struct Hobby {
    name: String,
}

#[derive(Clone, PartialEq, Default)]
struct Person {
    name: String,
    hobbies: Vec<Hobby>,
}

fn main() {
    dioxus::launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    let form = use_form(Person::default);
    let person = form.scope();
    rsx! {
        form { onsubmit: form.submit_handler(),
            PersonFields { scope: person }
            button { r#type: "submit", "Save" }
        }
    }
}

#[component]
fn PersonFields<Root>(scope: FormScope<Person, Root>) -> Element
where
    Root: Clone + PartialEq + 'static,
{
    let name = scope.field(FieldSpec::new(
        "name",
        |person: &Person| person.name.clone(),
        |person, value| person.name = value,
    ));
    let hobbies = scope.list(ListSpec::new(
        "hobbies",
        |person: &Person| person.hobbies.clone(),
        |person, value| person.hobbies = value,
    ));

    rsx! {
        input {
            value: "{name.input_value()}",
            oninput: name.on_input_text(),
        }
        {ListAddButton(hobbies.clone(), Rc::new(Hobby::default), None, rsx!("Add hobby"))}
        for item in hobbies.items() {
            HobbyItem { item }
        }
        {ListClearButton(hobbies, None, rsx!("Clear hobbies"))}
    }
}

#[component]
fn HobbyItem<Root>(item: ListItemHandle<Hobby, Root>) -> Element
where
    Root: Clone + PartialEq + 'static,
{
    let scope = item.scope();
    rsx! {
        HobbyFields { scope }
        {ListItemRemoveButton(item, None, rsx!("Remove"))}
    }
}

#[component]
fn HobbyFields<Root>(scope: FormScope<Hobby, Root>) -> Element
where
    Root: Clone + PartialEq + 'static,
{
    let name = scope.field(FieldSpec::new(
        "name",
        |hobby: &Hobby| hobby.name.clone(),
        |hobby, value| hobby.name = value,
    ));
    rsx! {
        input {
            value: "{name.input_value()}",
            oninput: name.on_input_text(),
        }
    }
}
