use dioxus::prelude::*;
use dxform::prelude::*;

#[derive(Clone, PartialEq, Default)]
struct Address {
    city: String,
}

#[derive(Clone, PartialEq, Default)]
struct Profile {
    name: String,
    address: Address,
}

fn main() {
    dioxus::launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    let form = use_form(Profile::default);
    let profile = form.scope();
    let name = profile.field(FieldSpec::new(
        "name",
        |profile: &Profile| profile.name.clone(),
        |profile, value| profile.name = value,
    ));
    let address = profile.subform(SubformSpec::new(
        "address",
        |profile: &Profile| profile.address.clone(),
        |profile, value| profile.address = value,
    ));

    rsx! {
        form { onsubmit: form.submit_handler(),
            input {
                value: "{name.input_value()}",
                oninput: name.on_input_text(),
            }
            AddressFields { scope: address }
            button { r#type: "button", onclick: move |_| profile.reset(), "Reset profile" }
            button { r#type: "submit", "Save" }
        }
    }
}

#[component]
fn AddressFields<Root>(scope: FormScope<Address, Root>) -> Element
where
    Root: Clone + PartialEq + 'static,
{
    let city = scope.field(FieldSpec::new(
        "city",
        |address: &Address| address.city.clone(),
        |address, value| address.city = value,
    ));
    rsx! {
        input {
            value: "{city.input_value()}",
            oninput: city.on_input_text(),
        }
        button { r#type: "button", onclick: move |_| scope.reset(), "Reset address" }
    }
}
