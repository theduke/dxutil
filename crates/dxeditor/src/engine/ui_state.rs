use super::{
    EditorEngine, EditorSelection, InlineContainerRef, Uid, suggestion::TriggerMatch, text,
};
use crate::{AttributeMap, ComponentMark, SuggestionItem};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub selection: Option<Rect>,
    pub caret: Option<Rect>,
    pub block: Option<Rect>,
    pub table: Option<Rect>,
    pub table_rows: Vec<Rect>,
    pub table_columns: Vec<Rect>,
    pub cell: Option<Rect>,
    pub viewport: Rect,
    pub scale_x: f64,
    pub scale_y: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionSummary {
    pub selection: EditorSelection,
    pub collapsed: bool,
    pub container: Option<InlineContainerRef>,
    pub uid: Option<Uid>,
    pub kind: Option<String>,
    pub attrs: AttributeMap,
    pub active_marks: Vec<ComponentMark>,
    pub link: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionUi {
    pub manual: bool,
    pub matched: TriggerMatch,
    pub items: Vec<SuggestionItem>,
    pub active_index: usize,
    pub generation: u64,
    pub searching: bool,
    pub listbox_id: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TableUi {
    pub uid: Uid,
    pub rows: usize,
    pub cols: usize,
    pub anchor: (usize, usize),
    pub head: (usize, usize),
    pub header_row: bool,
    pub header_column: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImageUi {
    pub uid: Uid,
    pub attrs: AttributeMap,
    pub transient: AttributeMap,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EntityPreviewUi {
    pub entity_id: String,
    pub preview: Option<crate::EntityLinkPreview>,
    pub loading: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EditorUiState {
    pub profile: super::Profile,
    pub entity_preview: Option<EntityPreviewUi>,
    pub read_only: bool,
    pub composing: bool,
    pub focused: bool,
    pub selection: SelectionSummary,
    pub suggestion: Option<SuggestionUi>,
    pub table: Option<TableUi>,
    pub image: Option<ImageUi>,
    pub hover_block: Option<Uid>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub geometry: Geometry,
    pub open_menu: Option<String>,
}
impl EditorUiState {
    pub fn derive(engine: &EditorEngine) -> Self {
        let uid = super::commands::blocks::current_uid(engine);
        let node = uid.as_ref().and_then(|id| engine.content(id));
        let range = engine.selection.text_range();
        let container = range.map(|range| range.head.container.clone());
        let active_marks = if engine.stored_marks_explicit {
            engine.stored_marks.clone()
        } else {
            range
                .and_then(|r| {
                    engine
                        .inline(&r.head.container)
                        .map(|n| text::marks_at(n, r.head.offset))
                })
                .unwrap_or_default()
        };
        let link = active_marks
            .iter()
            .find(|m| m.kind.0 == "link")
            .and_then(|m| m.attrs.get("href"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let table = node.filter(|n| n.kind.0 == "table").and_then(|n| {
            let uid = uid.clone()?;
            let (anchor, head) = match &engine.selection {
                EditorSelection::Cells { anchor, head, .. } => (*anchor, *head),
                EditorSelection::Text(range) => {
                    if let InlineContainerRef::Cell { row, col } = &range.head.container {
                        let r = n
                            .content
                            .iter()
                            .position(|r| r.id.as_ref().is_some_and(|id| id.0 == *row))?;
                        ((r, *col), (r, *col))
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };
            Some(TableUi {
                uid,
                rows: n.content.len(),
                cols: n.content.first().map_or(0, |r| r.content.len()),
                anchor,
                head,
                header_row: n
                    .attrs
                    .get("header_row")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                header_column: n
                    .attrs
                    .get("header_column")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            })
        });
        let image = node.filter(|n| n.kind.0 == "image").and_then(|n| {
            Some(ImageUi {
                uid: uid.clone()?,
                attrs: n.attrs.clone(),
                transient: uid
                    .as_ref()
                    .and_then(|id| engine.transient.get(id))
                    .cloned()
                    .unwrap_or_default(),
            })
        });
        Self {
            profile: engine.profile,
            entity_preview: engine.entity_preview.clone(),
            read_only: engine.options.read_only,
            composing: engine.composing,
            focused: engine.focused,
            selection: SelectionSummary {
                selection: engine.selection.clone(),
                collapsed: engine.selection.is_collapsed(),
                container,
                uid,
                kind: node.map(|n| n.kind.0.clone()),
                attrs: node.map(|n| n.attrs.clone()).unwrap_or_default(),
                active_marks,
                link,
            },
            suggestion: engine
                .suggestion
                .active
                .as_ref()
                .map(|matched| SuggestionUi {
                    manual: engine.manual_suggestion,
                    matched: matched.clone(),
                    items: engine.suggestion.items.clone(),
                    active_index: engine.suggestion.active_index,
                    generation: engine.suggestion.generation,
                    searching: engine.suggestion.searching,
                    listbox_id: engine.listbox_id.clone(),
                }),
            table,
            image,
            hover_block: engine.hover_block.clone(),
            can_undo: engine.can_undo(),
            can_redo: engine.can_redo(),
            geometry: Geometry::default(),
            open_menu: engine.open_menu.clone(),
        }
    }
}
