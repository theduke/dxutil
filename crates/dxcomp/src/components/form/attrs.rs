use dioxus::core::{AttributeValue, IntoAttributeValue};
use dioxus::prelude::*;

/// Merge whitespace-separated IDREF lists, deduplicating in first-seen order.
pub fn merge_idrefs<I, S>(parts: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut ids = Vec::new();
    for part in parts {
        for id in part.as_ref().split_whitespace() {
            if !ids.iter().any(|existing| existing == id) {
                ids.push(id.to_string());
            }
        }
    }
    (!ids.is_empty()).then(|| ids.join(" "))
}

#[derive(Default)]
pub(crate) struct CallerControlAttrs {
    pub id: Option<String>,
    pub labelledby: Option<String>,
    pub describedby: Option<String>,
    pub invalid: Option<bool>,
    pub disabled: bool,
    pub required: bool,
    pub read_only: bool,
    pub name: Option<String>,
    pub rest: Vec<Attribute>,
}

pub(crate) fn take_control_attrs(attributes: Vec<Attribute>) -> CallerControlAttrs {
    let mut caller = CallerControlAttrs::default();
    for attribute in attributes {
        let boolean = match &attribute.value {
            AttributeValue::Bool(value) => Some(*value),
            AttributeValue::Text(value) => Some(!value.is_empty() && value != "false"),
            AttributeValue::None => Some(false),
            _ => None,
        };
        let text = match &attribute.value {
            AttributeValue::Text(value) => Some(value.clone()),
            AttributeValue::None | AttributeValue::Bool(_) => None,
            _ => {
                caller.rest.push(attribute);
                continue;
            }
        };
        if attribute.namespace.is_some() {
            caller.rest.push(attribute);
            continue;
        }
        let is_text = matches!(&attribute.value, AttributeValue::Text(_) | AttributeValue::None);
        match attribute.name {
            "id" if is_text => caller.id = text,
            "aria-labelledby" if is_text => caller.labelledby = text,
            "aria-describedby" if is_text => caller.describedby = text,
            "name" if is_text => caller.name = text,
            "aria-invalid" if boolean.is_some() => caller.invalid = boolean,
            "disabled" if boolean.is_some() => caller.disabled = boolean.unwrap_or(false),
            "required" if boolean.is_some() => caller.required = boolean.unwrap_or(false),
            "readonly" if boolean.is_some() => caller.read_only = boolean.unwrap_or(false),
            _ => caller.rest.push(attribute),
        }
    }
    caller
}

pub(crate) fn attr(name: &'static str, value: impl IntoAttributeValue) -> Attribute {
    Attribute::new(name, value, None, false)
}
