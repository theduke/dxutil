use std::{collections::BTreeMap, rc::Rc};

use futures::future::LocalBoxFuture;
use serde_json::Value;

use crate::catalog::EditorCatalog;

#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub data: Value,
    pub priority: i32,
}

#[derive(Clone)]
pub struct SuggestionQueryContext {
    pub query: String,
    pub catalog: EditorCatalog,
}

pub trait SuggestionProvider {
    fn trigger(&self) -> char;

    fn query(&self, ctx: SuggestionQueryContext) -> LocalBoxFuture<'static, Vec<SuggestionItem>>;
}

#[derive(Clone, Default)]
pub struct SuggestionRegistry {
    providers: BTreeMap<char, Rc<dyn SuggestionProvider>>,
}

impl SuggestionRegistry {
    pub fn register(&mut self, provider: Rc<dyn SuggestionProvider>) {
        self.providers.insert(provider.trigger(), provider);
    }

    pub fn triggers(&self) -> Vec<char> {
        self.providers.keys().copied().collect()
    }

    pub fn provider(&self, trigger: char) -> Option<Rc<dyn SuggestionProvider>> {
        self.providers.get(&trigger).cloned()
    }
}

pub struct EntityMentionSuggestionProvider;

impl SuggestionProvider for EntityMentionSuggestionProvider {
    fn trigger(&self) -> char {
        '@'
    }

    fn query(&self, _ctx: SuggestionQueryContext) -> LocalBoxFuture<'static, Vec<SuggestionItem>> {
        Box::pin(async { Vec::new() })
    }
}

impl PartialEq for SuggestionRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.providers.len() == other.providers.len()
            && self.providers.iter().all(|(key, value)| {
                other
                    .providers
                    .get(key)
                    .is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }
}
