use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};

/// A searchable entity that can be inserted as an internal editor link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityLinkCandidate {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A small, presentation-neutral field shown in an entity-link hover preview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityPreviewField {
    pub label: String,
    pub value: String,
}

/// Presentation data for the hover preview of an internal entity link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityLinkPreview {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EntityPreviewField>,
}

/// Application-owned entity lookup used by the editor integration.
///
/// `dxeditor` deliberately knows nothing about entity storage, schemas, or display
/// conventions. Applications opt in by providing this interface.
pub trait EntityLinkProvider {
    fn search(&self, query: String) -> LocalBoxFuture<'static, Vec<EntityLinkCandidate>>;

    /// Applications can narrow `[[` searches to documents.
    fn search_for_trigger(
        &self,
        query: String,
        _trigger: String,
    ) -> LocalBoxFuture<'static, Vec<EntityLinkCandidate>> {
        self.search(query)
    }

    fn preview(&self, entity_id: String) -> LocalBoxFuture<'static, Option<EntityLinkPreview>>;

    fn open(&self, _entity_id: String) {}
}

/// Optional application extension for internal entity links.
#[derive(Clone)]
pub struct EntityLinkExtension {
    provider: Rc<dyn EntityLinkProvider>,
    accent_color: String,
}

impl EntityLinkExtension {
    pub fn new(provider: Rc<dyn EntityLinkProvider>) -> Self {
        Self {
            provider,
            accent_color: "#176b87".to_string(),
        }
    }

    pub fn with_accent_color(mut self, color: impl Into<String>) -> Self {
        let color = color.into();
        if color.len() <= 128
            && !color.is_empty()
            && !color.chars().any(|character| {
                character.is_control() || matches!(character, ';' | '{' | '}' | '<' | '>')
            })
        {
            self.accent_color = color;
        }
        self
    }

    pub fn provider(&self) -> Rc<dyn EntityLinkProvider> {
        self.provider.clone()
    }

    pub fn accent_color(&self) -> &str {
        &self.accent_color
    }
}

impl PartialEq for EntityLinkExtension {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.provider, &other.provider) && self.accent_color == other.accent_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyProvider;

    impl EntityLinkProvider for EmptyProvider {
        fn search(&self, _query: String) -> LocalBoxFuture<'static, Vec<EntityLinkCandidate>> {
            Box::pin(async { Vec::new() })
        }

        fn preview(
            &self,
            _entity_id: String,
        ) -> LocalBoxFuture<'static, Option<EntityLinkPreview>> {
            Box::pin(async { None })
        }
    }

    #[test]
    fn accent_color_rejects_style_injection() {
        let extension = EntityLinkExtension::new(Rc::new(EmptyProvider))
            .with_accent_color("red; display: none");
        assert_eq!(extension.accent_color(), "#176b87");
        let extension = extension.with_accent_color("var(--color-primary, #176b87)");
        assert_eq!(extension.accent_color(), "var(--color-primary, #176b87)");
    }
}
