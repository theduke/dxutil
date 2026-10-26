//! Trigger matching and cancellation are model state; debounce belongs to the adapter.
use super::{InlineContainerRef, TextPoint, TextRange};
use crate::SuggestionItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerMatch {
    pub trigger: String,
    pub query: String,
    pub range: TextRange,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuggestionState {
    pub active: Option<TriggerMatch>,
    pub items: Vec<SuggestionItem>,
    pub active_index: usize,
    pub generation: u64,
    pub searching: bool,
    dismissed: Option<TriggerMatch>,
}
impl SuggestionState {
    pub fn update(&mut self, next: Option<TriggerMatch>) -> bool {
        if self.dismissed.as_ref() != next.as_ref() {
            self.dismissed = None;
        }
        let next = if self.dismissed.is_some() { None } else { next };
        if self.active == next {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.active = next;
        self.items.clear();
        self.active_index = 0;
        self.searching = self.active.is_some();
        true
    }
    pub fn dismiss(&mut self) {
        if let Some(active) = self.active.take() {
            self.dismissed = Some(active);
        }
        self.items.clear();
        self.searching = false;
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn close(&mut self) {
        self.active = None;
        self.items.clear();
        self.searching = false;
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn set_items(&mut self, generation: u64, items: Vec<SuggestionItem>) -> bool {
        if self.generation != generation || self.active.is_none() {
            return false;
        }
        self.items = items;
        self.active_index = self.items.iter().position(selectable).unwrap_or(0);
        self.searching = false;
        true
    }
    pub fn step(&mut self, delta: i32) {
        let indices = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| selectable(item).then_some(i))
            .collect::<Vec<_>>();
        if indices.is_empty() {
            return;
        }
        let current = indices
            .iter()
            .position(|i| *i == self.active_index)
            .unwrap_or(0) as i32;
        self.active_index = indices[(current + delta).rem_euclid(indices.len() as i32) as usize];
    }
    pub fn first(&mut self) {
        if let Some(i) = self.items.iter().position(selectable) {
            self.active_index = i;
        }
    }
    pub fn last(&mut self) {
        if let Some(i) = self.items.iter().rposition(selectable) {
            self.active_index = i;
        }
    }
    pub fn selected(&self) -> Option<&SuggestionItem> {
        self.items.get(self.active_index).filter(|i| selectable(i))
    }
}
fn selectable(item: &SuggestionItem) -> bool {
    item.data
        .get("disabled")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
}
pub fn detect(
    text: &str,
    container: InlineContainerRef,
    triggers: &[String],
    composer: bool,
    aliases: &[String],
) -> Option<TriggerMatch> {
    let mut occurrences = triggers
        .iter()
        .filter_map(|trigger| text.rfind(trigger).map(|i| (i, trigger)))
        .collect::<Vec<_>>();
    occurrences.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
    for (at, trigger) in occurrences {
        if trigger.is_empty() {
            continue;
        }
        if trigger != "[["
            && at > 0
            && !text[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let query = &text[at + trigger.len()..];
        if query.contains(['\n', '\r', '\u{fffc}']) {
            continue;
        }
        let accepted = if composer {
            trigger != "/" || !query.chars().any(|c| c.is_whitespace() || c == '/')
        } else {
            match trigger.as_str() {
                "[[" => !query.contains(']'),
                "@" => !query.chars().any(char::is_whitespace),
                "/" => {
                    !query.contains('/')
                        && !query.contains('\t')
                        && (!query.contains(' ')
                            || aliases.iter().any(|alias| {
                                alias.contains(' ')
                                    && alias.to_lowercase().starts_with(&query.to_lowercase())
                            }))
                }
                _ => !query.chars().any(char::is_whitespace),
            }
        };
        if accepted {
            return Some(TriggerMatch {
                trigger: trigger.clone(),
                query: query.into(),
                range: TextRange {
                    anchor: TextPoint::new(container.clone(), text[..at].chars().count()),
                    head: TextPoint::new(container, text.chars().count()),
                },
            });
        }
    }
    None
}
/// Lower scores rank higher; exact prefixes win, then compact subsequences.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let query = query.to_lowercase();
    let candidate = candidate.to_lowercase();
    if candidate.starts_with(&query) {
        return Some(0);
    }
    let mut cursor = 0;
    let chars = candidate.chars().collect::<Vec<_>>();
    let mut score = 0;
    for wanted in query.chars() {
        let offset = chars[cursor..].iter().position(|c| *c == wanted)?;
        score += offset + 1;
        cursor += offset + 1;
    }
    Some(score + candidate.chars().count())
}
