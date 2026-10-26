use dioxus::prelude::*;

use crate::{FieldHandle, FormError, FormErrorSource};

pub trait InputValue: Clone + PartialEq + 'static {
    fn to_input_value(&self) -> String;
    fn from_input_value(value: &str) -> std::result::Result<Self, FormError>;
    fn is_empty_input(&self) -> bool;
}

impl InputValue for String {
    fn to_input_value(&self) -> String {
        self.clone()
    }

    fn from_input_value(value: &str) -> std::result::Result<Self, FormError> {
        Ok(value.to_string())
    }

    fn is_empty_input(&self) -> bool {
        self.trim().is_empty()
    }
}

impl InputValue for Option<String> {
    fn to_input_value(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn from_input_value(value: &str) -> std::result::Result<Self, FormError> {
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value.to_string()))
        }
    }

    fn is_empty_input(&self) -> bool {
        self.as_ref().is_none_or(|value| value.trim().is_empty())
    }
}

macro_rules! input_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl InputValue for $ty {
                fn to_input_value(&self) -> String {
                    self.to_string()
                }

                fn from_input_value(value: &str) -> std::result::Result<Self, FormError> {
                    value.parse::<$ty>().map_err(|err| {
                        FormError::new(format!("invalid number: {err}"))
                            .with_source(FormErrorSource::Parse)
                    })
                }

                fn is_empty_input(&self) -> bool {
                    false
                }
            }
        )*
    };
}

input_number!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

impl<Value, Root> FieldHandle<Value, Root>
where
    Root: Clone + PartialEq + 'static,
    Value: InputValue,
{
    pub fn input_value(&self) -> String {
        self.value().to_input_value()
    }

    pub fn on_input_text(&self) -> EventHandler<FormEvent> {
        let field = self.clone();
        EventHandler::new(
            move |event: FormEvent| match Value::from_input_value(&event.value()) {
                Ok(value) => field.set_value(value),
                Err(error) => {
                    field.root.with_registry(|registry| {
                        if let Some(node) = registry.nodes.get_mut(&field.path) {
                            node.errors = vec![error.at_path(&field.path)];
                        }
                    });
                    field.root.recompute_all_meta();
                }
            },
        )
    }

    pub fn on_blur(&self) -> EventHandler<FocusEvent> {
        let field = self.clone();
        EventHandler::new(move |_| field.set_focused(false))
    }

    pub fn on_focus(&self) -> EventHandler<FocusEvent> {
        let field = self.clone();
        EventHandler::new(move |_| field.set_focused(true))
    }
}

impl<Root> FieldHandle<bool, Root>
where
    Root: Clone + PartialEq + 'static,
{
    pub fn checked(&self) -> bool {
        self.value()
    }

    pub fn on_change_checked(&self) -> EventHandler<FormEvent> {
        let field = self.clone();
        EventHandler::new(move |event: FormEvent| field.set_value(event.value() == "true"))
    }
}
