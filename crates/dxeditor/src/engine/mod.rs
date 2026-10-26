//! Platform-neutral editing state. Browser APIs are confined to the DOM adapter.
pub mod commands;
pub mod history;
pub mod identity;
pub mod input_rules;
pub mod input_types;
pub mod keymap;
pub mod model;
pub mod profile;
pub mod schema;
pub mod selection;
pub mod suggestion;
pub mod text;
pub mod transaction;
pub mod ui_state;
// The transform and clipboard modules are independent of the browser adapter.
pub mod clipboard;
pub mod html;
pub mod markdown_blocks;

use crate::{
    AttributeMap, ComponentDocumentV2, ComponentMark, EditorCatalog, EngineEvent, HistoryPolicy,
    UnknownComponentPolicy, ValidationLimits,
};
pub use commands::{Command, CommandAvailability};
use history::History;
pub use keymap::KeyChord;
pub use profile::{EngineOptions, Profile};
use schema::EditingSchema;
pub use selection::{EditorSelection, InlineContainerRef, TextPoint, TextRange, Uid};
use std::collections::{BTreeMap, BTreeSet};
use suggestion::SuggestionState;
pub use transaction::{HistoryMode, Op, RenderPlan, Transaction, TxKind};
pub use ui_state::EditorUiState;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0}")]
    InvalidOperation(String),
    #[error("{0}")]
    InvalidCommand(String),
    #[error("{0}")]
    InvalidDocument(#[from] crate::DocumentValidationError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineInput {
    InsertText {
        text: String,
    },
    ReplaceRange {
        range: TextRange,
        text: String,
    },
    InsertParagraph,
    InsertLineBreak,
    Delete {
        direction: text::Direction,
        unit: text::DeleteUnit,
    },
    CompositionStart,
    CompositionEnd {
        container: InlineContainerRef,
        dom_text: String,
        dom_caret: Option<usize>,
    },
    SelectionChanged(EditorSelection),
    Paste(clipboard::PasteContent),
    Command(Command),
    Key(KeyChord),
    HostResponse(HostResponse),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusTarget {
    Editor,
    Block(Uid),
    AfterBlock(Uid),
    Island(Uid),
}
#[derive(Clone, Debug, PartialEq)]
pub enum HostResponse {
    EntityPicker {
        request_id: u64,
        entity: Option<crate::EntityLinkCandidate>,
    },
    EntityCreate {
        request_id: u64,
        entity: Option<crate::EntityLinkCandidate>,
    },
    Markdown {
        request_id: u64,
        nodes: Option<Vec<crate::ComponentNode>>,
    },
    MediaUrl {
        request_id: u64,
        url: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerAction {
    Submit { alternate: bool },
    BackgroundSubmit,
    HistoryPrevious,
    HistoryNext,
    Stash,
    PlainPaste,
    SuggestionAccepted(crate::SuggestionItem),
}
#[derive(Clone, Debug)]
struct PendingRequest {
    selection: EditorSelection,
    kind: PendingKind,
}
struct BatchState {
    ops: Vec<Op>,
    selection_before: EditorSelection,
    kind: TxKind,
    dirty: BTreeSet<Uid>,
    structure_changed: bool,
}
#[derive(Clone, Debug)]
enum PendingKind {
    Picker(String),
    Create {
        trigger: String,
        uid: Option<Uid>,
        content: Option<crate::ComponentNode>,
    },
    Markdown(String),
}
#[derive(Clone, Debug, PartialEq)]
pub enum EngineOutput {
    Render(RenderPlan),
    ApplySelection(EditorSelection),
    Request(EngineEvent),
    Changed {
        revision: u64,
        dirty_blocks: Vec<Uid>,
    },
    /// Rare intermediate state when an input rule adds its own undo entry.
    ChangedSnapshot {
        revision: u64,
        dirty_blocks: Vec<Uid>,
        document: ComponentDocumentV2,
    },
    Scroll {
        uid: Uid,
        highlight: bool,
    },
    Focus {
        target: FocusTarget,
    },
    UiState(EditorUiState),
    QuerySuggestions {
        generation: u64,
        trigger: String,
        query: String,
    },
    Composer(ComposerAction),
    QueryEntityPreview {
        entity_id: String,
    },
    EntityOpen {
        entity_id: String,
    },
    RememberSuggestion {
        trigger: String,
        item: crate::SuggestionItem,
    },
}

pub struct EditorEngine {
    pub(crate) doc: ComponentDocumentV2,
    pub(crate) selection: EditorSelection,
    pub(crate) index: model::BlockIndex,
    pub(crate) schema: EditingSchema,
    pub(crate) profile: Profile,
    pub(crate) options: EngineOptions,
    pub(crate) history: History,
    pub(crate) revision: u64,
    pub(crate) stored_marks: Vec<ComponentMark>,
    pub(crate) stored_marks_explicit: bool,
    pub(crate) composing: bool,
    pub(crate) collapsed: BTreeSet<Uid>,
    pub(crate) transient: BTreeMap<Uid, AttributeMap>,
    pub(crate) focused: bool,
    pub(crate) hover_block: Option<Uid>,
    pub(crate) open_menu: Option<String>,
    pub(crate) suggestion: SuggestionState,
    pub(crate) composer_submit_on_enter: bool,
    pub(crate) last_highlight: String,
    pub(crate) last_input_rule: bool,
    pub(crate) listbox_id: String,
    pub(crate) entity_preview: Option<ui_state::EntityPreviewUi>,
    recent_blocks: Vec<String>,
    recent_mentions: Vec<crate::EntityLinkCandidate>,
    typed_mention_trigger: bool,
    pub(crate) manual_suggestion: bool,
    suggestion_mode: String,
    triggers: Vec<String>,
    pending: BTreeMap<u64, PendingRequest>,
    next_request_id: u64,
    last_ui: Option<EditorUiState>,
    batch: Option<BatchState>,
    time_ms: Option<u64>,
    queued_replace: Option<(ComponentDocumentV2, HistoryPolicy)>,
}
impl EditorEngine {
    pub fn new(
        mut doc: ComponentDocumentV2,
        catalog: &EditorCatalog,
        profile: Profile,
        options: EngineOptions,
    ) -> Result<Self, EngineError> {
        // The editable tree always has a root group and at least one caret position.
        if doc.root.content.is_empty() {
            doc.root.content = model::document(vec![model::paragraph()]).root.content;
            if doc.schema.0 == crate::COMPONENT_DOCUMENT_SCHEMA {
                doc.schema = crate::profiles::block_document::BLOCK_DOCUMENT_SCHEMA.into();
            }
        }
        for op in identity::repairs(&doc, &BTreeMap::new()) {
            op.apply(&mut doc)?;
        }
        normalize_initial(&mut doc)?;
        crate::validate_component_document(
            &doc,
            catalog.component_specs(),
            &ValidationLimits::default(),
            UnknownComponentPolicy::Reject,
        )?;
        let selection = model::first_caret(&doc).ok_or_else(|| {
            EngineError::InvalidCommand("Document contains no caret position".into())
        })?;
        Ok(Self {
            index: model::BlockIndex::new(&doc),
            doc,
            selection,
            schema: EditingSchema::new(catalog),
            profile,
            history: History::new(options.history_depth, options.history_group_ms),
            options,
            revision: 0,
            stored_marks: Vec::new(),
            stored_marks_explicit: false,
            composing: false,
            collapsed: BTreeSet::new(),
            transient: BTreeMap::new(),
            focused: false,
            hover_block: None,
            open_menu: None,
            suggestion: SuggestionState::default(),
            composer_submit_on_enter: true,
            last_highlight: "yellow".into(),
            last_input_rule: false,
            entity_preview: None,
            recent_blocks: Vec::new(),
            recent_mentions: Vec::new(),
            typed_mention_trigger: false,
            manual_suggestion: false,
            suggestion_mode: "mention".into(),
            listbox_id: format!("dxeditor-suggestions-{}", identity::new_uid()),
            triggers: if profile == Profile::Composer {
                Vec::new()
            } else {
                vec!["/".into(), "@".into(), "[[".into()]
            },
            pending: BTreeMap::new(),
            next_request_id: 1,
            last_ui: None,
            batch: None,
            time_ms: None,
            queued_replace: None,
        })
    }
    pub fn block(&self, uid: &str) -> Option<&crate::ComponentNode> {
        self.index.get(&self.doc, uid)
    }
    pub fn content(&self, uid: &str) -> Option<&crate::ComponentNode> {
        self.block(uid)?.content.first()
    }
    pub fn inline(&self, container: &InlineContainerRef) -> Option<&Vec<crate::ComponentNode>> {
        model::inline_indexed(&self.doc, &self.index, container)
    }
    pub fn container_owner(&self, container: &InlineContainerRef) -> Option<Uid> {
        match container {
            InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => Some(uid.clone()),
            InlineContainerRef::Cell { row, .. } => {
                self.index.rows.get(row).map(|(owner, _)| owner.clone())
            }
        }
    }
    pub fn location(&self, uid: &str) -> Option<(Option<Uid>, usize)> {
        let path = self.index.paths.get(uid)?;
        let index = *path.last()?;
        let parent = if path.len() > 2 {
            model::node_at(&self.doc.root, &path[..path.len() - 2])?
                .id
                .as_ref()
                .map(|id| id.0.clone())
        } else {
            None
        };
        Some((parent, index))
    }
    fn refresh_index(&mut self, tx: &Transaction, plan: &RenderPlan) {
        if plan.structure_changed {
            self.index = model::BlockIndex::new(&self.doc);
        } else {
            for op in &tx.ops {
                if let Op::SetContent { uid, before, after } = op
                    && (before.kind.0 == "table" || after.kind.0 == "table")
                {
                    self.index.refresh_table(uid, after);
                }
            }
        }
    }
    pub fn document(&self) -> &ComponentDocumentV2 {
        &self.doc
    }
    pub fn selection(&self) -> &EditorSelection {
        &self.selection
    }
    pub fn profile(&self) -> Profile {
        self.profile
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn read_only(&self) -> bool {
        self.options.read_only
    }
    pub fn is_composing(&self) -> bool {
        self.composing
    }
    pub fn schema(&self) -> &EditingSchema {
        &self.schema
    }
    pub fn transient(&self, uid: &str) -> Option<&AttributeMap> {
        self.transient.get(uid)
    }
    pub fn is_collapsed(&self, uid: &str) -> bool {
        self.collapsed.contains(uid)
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    /// The adapter supplies its monotonic browser clock before dispatching input.
    pub fn set_time_ms(&mut self, time_ms: u64) {
        self.time_ms = Some(time_ms);
    }
    pub fn set_read_only(&mut self, read_only: bool) -> Vec<EngineOutput> {
        self.options.read_only = read_only;
        self.finish_outputs(Vec::new())
    }
    pub fn ui_state(&self) -> EditorUiState {
        EditorUiState::derive(self)
    }
    pub fn suggestion(&self) -> &SuggestionState {
        &self.suggestion
    }
    pub fn set_triggers(&mut self, triggers: Vec<String>) -> Vec<EngineOutput> {
        self.triggers = triggers;
        self.finish_outputs(Vec::new())
    }
    pub fn set_suggestion_triggers(&mut self, triggers: Vec<char>) -> Vec<EngineOutput> {
        self.set_triggers(triggers.into_iter().map(|c| c.to_string()).collect())
    }
    pub fn set_composer_submit_on_enter(&mut self, value: bool) {
        self.composer_submit_on_enter = value;
    }
    pub fn set_focused(&mut self, focused: bool) -> Vec<EngineOutput> {
        self.focused = focused;
        self.finish_outputs(Vec::new())
    }
    pub fn set_hover_block(&mut self, uid: Option<Uid>) -> Vec<EngineOutput> {
        self.hover_block = uid;
        self.publish_ui(Vec::new())
    }
    pub fn set_last_highlight(&mut self, color: String) {
        self.last_highlight = color;
    }
    pub fn set_entity_preview(
        &mut self,
        entity_id: String,
        preview: Option<crate::EntityLinkPreview>,
        loading: bool,
    ) -> Vec<EngineOutput> {
        self.entity_preview = Some(ui_state::EntityPreviewUi {
            entity_id,
            preview,
            loading,
        });
        self.publish_ui(Vec::new())
    }
    pub fn clear_entity_preview(&mut self) -> Vec<EngineOutput> {
        self.entity_preview = None;
        self.publish_ui(Vec::new())
    }
    /// Dismiss contextual menus and entity previews when interacting elsewhere.
    /// Unlike `Command::CloseMenu`, this never requests focus, so the host can
    /// leave focus and selection handling to the interaction's target.
    pub fn dismiss_overlays(&mut self) -> Vec<EngineOutput> {
        self.open_menu = None;
        self.entity_preview = None;
        self.publish_ui(Vec::new())
    }
    pub fn set_recent_blocks(&mut self, ids: Vec<String>) {
        self.recent_blocks = ids.into_iter().take(5).collect();
    }
    pub fn set_recent_mentions(&mut self, candidates: Vec<crate::EntityLinkCandidate>) {
        self.recent_mentions = candidates.into_iter().take(5).collect();
    }
    pub fn set_suggestion_items(
        &mut self,
        generation: u64,
        mut items: Vec<crate::SuggestionItem>,
    ) -> Vec<EngineOutput> {
        if self.profile != Profile::Composer
            && let Some(matched) = &self.suggestion.active
        {
            if matched.query.is_empty() {
                let mut recent = self
                    .recent_mentions
                    .iter()
                    .map(|entity| crate::SuggestionItem {
                        id: entity.id.clone(),
                        label: entity.label.clone(),
                        description: entity.detail.clone(),
                        icon: None,
                        data: serde_json::json!({"entity":entity,"group":"Recent"}),
                        priority: 0,
                    })
                    .collect::<Vec<_>>();
                recent.extend(
                    items
                        .into_iter()
                        .filter(|item| !self.recent_mentions.iter().any(|e| e.id == item.id)),
                );
                items = recent;
            }
            items.push(crate::SuggestionItem {
                id: "create-entity".into(),
                label: if matched.trigger == "[[" {
                    format!("New sub-document “{}”", matched.query)
                } else {
                    format!("Create “{}” as…", matched.query)
                },
                description: None,
                icon: Some("+".into()),
                data: serde_json::json!({"create":true,"group":"Create"}),
                priority: 0,
            });
        }
        self.suggestion.set_items(generation, items);
        self.publish_ui(Vec::new())
    }
    pub fn availability(&self, command: &Command) -> CommandAvailability {
        if self.options.read_only && command.mutates() {
            return CommandAvailability::Disabled("Read-only document".into());
        }
        match command {
            Command::ToggleMark(mark) | Command::SetMark(mark)
                if !self.profile.allows_mark(&mark.kind.0)
                    || !self.schema.marks.contains(&mark.kind.0) =>
            {
                CommandAvailability::Hidden
            }
            Command::TurnInto { kind, .. } if !self.schema.has_kind(kind) => {
                CommandAvailability::Hidden
            }
            Command::SetColor { .. } if !self.profile.allows_colors() => {
                CommandAvailability::Hidden
            }
            Command::Image { attrs, .. }
                if self.profile == Profile::MarkdownNotes
                    && (attrs.contains_key("width")
                        || attrs.contains_key("alignment")
                        || attrs.get("media").is_some_and(|media| {
                            media.get("type").and_then(serde_json::Value::as_str) != Some("url")
                        })) =>
            {
                CommandAvailability::Hidden
            }
            Command::Undo if !self.can_undo() => {
                CommandAvailability::Disabled("Nothing to undo".into())
            }
            Command::Redo if !self.can_redo() => {
                CommandAvailability::Disabled("Nothing to redo".into())
            }
            _ => CommandAvailability::Enabled,
        }
    }
    pub fn run(&mut self, command: Command) -> Vec<EngineOutput> {
        if self.availability(&command) != CommandAvailability::Enabled {
            return Vec::new();
        }
        if let Command::SetMark(mark) | Command::ToggleMark(mark) = &command
            && mark.kind.0 == "highlight"
            && let Some(color) = mark.attrs.get("color").and_then(serde_json::Value::as_str)
        {
            self.last_highlight = color.into();
        }
        let outputs = match command {
            Command::ClearMarks => {
                if self.selection.is_collapsed() {
                    self.stored_marks.clear();
                    self.stored_marks_explicit = true;
                    Vec::new()
                } else {
                    let marks = self
                        .schema
                        .marks
                        .iter()
                        .cloned()
                        .map(Command::UnsetMark)
                        .collect();
                    self.batch_commands(marks, TxKind::Command)
                }
            }
            Command::OpenEntity(entity_id) => vec![EngineOutput::EntityOpen { entity_id }],
            Command::OpenMenu(menu) => {
                self.open_menu = Some(menu);
                Vec::new()
            }
            Command::CloseMenu => {
                self.open_menu = None;
                vec![EngineOutput::Focus {
                    target: FocusTarget::Editor,
                }]
            }
            Command::SuggestionNext(delta) => {
                self.suggestion.step(delta);
                Vec::new()
            }
            Command::SuggestionFirst => {
                self.suggestion.first();
                Vec::new()
            }
            Command::SuggestionLast => {
                self.suggestion.last();
                Vec::new()
            }
            Command::DismissSuggestion => {
                self.manual_suggestion = false;
                self.suggestion.dismiss();
                Vec::new()
            }
            Command::SetSuggestionQuery(query) => {
                if let Some(mut matched) = self.suggestion.active.clone() {
                    matched.query = query;
                    self.suggestion.update(Some(matched));
                    if let Some(matched) = &self.suggestion.active {
                        vec![EngineOutput::QuerySuggestions {
                            generation: self.suggestion.generation,
                            trigger: matched.trigger.clone(),
                            query: matched.query.clone(),
                        }]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }
            Command::AcceptSuggestion => self.accept_suggestion(),
            Command::Request(mut event) => {
                if let EngineEvent::EntityPickerRequest { mode, .. } = &event
                    && !self.options.host_entity_picker
                {
                    self.suggestion_mode = mode.clone();
                    self.manual_suggestion = true;
                    let range = self.selection.text_range().cloned().or_else(|| {
                        model::first_caret(&self.doc).and_then(|s| s.text_range().cloned())
                    });
                    if let Some(range) = range {
                        self.suggestion.update(Some(suggestion::TriggerMatch {
                            trigger: "@".into(),
                            query: String::new(),
                            range,
                        }));
                        vec![EngineOutput::QuerySuggestions {
                            generation: self.suggestion.generation,
                            trigger: "@".into(),
                            query: String::new(),
                        }]
                    } else {
                        Vec::new()
                    }
                } else {
                    self.prepare_request(&mut event);
                    self.capture_request(&event);
                    vec![EngineOutput::Request(event)]
                }
            }
            other => commands::run(self, other),
        };
        self.finish_outputs(outputs)
    }
    pub fn handle(&mut self, input: EngineInput) -> Vec<EngineOutput> {
        match input {
            EngineInput::InsertText { text } => {
                self.last_input_rule = false;
                self.typed_mention_trigger = text.ends_with('@');
                let mut out = self.run(Command::InsertText(text));
                if let Some(tx) = input_rules::transaction(self, false) {
                    let mut snapshot = Some(self.doc.clone());
                    for output in &mut out {
                        if let EngineOutput::Changed {
                            revision,
                            dirty_blocks,
                        } = output
                        {
                            if let Some(document) = snapshot.take() {
                                *output = EngineOutput::ChangedSnapshot {
                                    revision: *revision,
                                    dirty_blocks: std::mem::take(dirty_blocks),
                                    document,
                                };
                            }
                        }
                    }
                    out.extend(input_rules::apply_transaction(self, tx));
                }
                self.typed_mention_trigger = false;
                self.finish_outputs(out)
            }
            EngineInput::ReplaceRange { range, text } => {
                self.run(Command::ReplaceRange { range, text })
            }
            EngineInput::InsertParagraph => {
                self.last_input_rule = false;
                let out = input_rules::apply(self, true);
                if out.is_empty() {
                    self.run(Command::SplitBlock)
                } else {
                    self.finish_outputs(out)
                }
            }
            EngineInput::InsertLineBreak => self.run(Command::HardBreak),
            EngineInput::Delete { direction, unit } => {
                if self.last_input_rule
                    && direction == text::Direction::Backward
                    && unit == text::DeleteUnit::Grapheme
                {
                    self.last_input_rule = false;
                    let out = self.undo();
                    self.finish_outputs(out)
                } else {
                    self.last_input_rule = false;
                    self.run(Command::Delete { direction, unit })
                }
            }
            EngineInput::Command(command) => self.run(command),
            EngineInput::Key(key) => self.handle_key(key),
            EngineInput::HostResponse(response) => self.host_response(response),
            EngineInput::SelectionChanged(selection) => {
                if selection != self.selection {
                    self.history.close_group();
                    self.stored_marks.clear();
                    self.stored_marks_explicit = false;
                    self.selection = selection;
                    self.clamp_selection();
                }
                self.finish_outputs(Vec::new())
            }
            EngineInput::CompositionStart => {
                self.composing = true;
                self.history.close_group();
                self.publish_ui(Vec::new())
            }
            EngineInput::CompositionEnd {
                container,
                dom_text,
                dom_caret,
            } => {
                self.composing = false;
                let mut outputs = Vec::new();
                if let Some(nodes) = self.inline(&container) {
                    let before = text::plain(nodes);
                    if before != dom_text && !self.options.read_only {
                        let a: Vec<_> = before.chars().collect();
                        let b: Vec<_> = dom_text.chars().collect();
                        let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
                        let suffix = a[prefix..]
                            .iter()
                            .rev()
                            .zip(b[prefix..].iter().rev())
                            .take_while(|(a, b)| a == b)
                            .count();
                        let range = TextRange {
                            anchor: TextPoint::new(container.clone(), prefix),
                            head: TextPoint::new(container.clone(), a.len() - suffix),
                        };
                        // Replace only the changed run so marks and inline atoms outside it survive.
                        outputs = self.batch_commands(
                            vec![Command::ReplaceRange {
                                range,
                                text: b[prefix..b.len() - suffix].iter().collect(),
                            }],
                            TxKind::Composition,
                        );
                        self.history.close_group();
                    }
                    if let Some(caret) = dom_caret {
                        self.selection = EditorSelection::caret(container, caret);
                        self.clamp_selection();
                        outputs.push(EngineOutput::ApplySelection(self.selection.clone()));
                    }
                }
                if let Some((doc, policy)) = self.queued_replace.take() {
                    outputs.extend(self.replace_document(doc, policy));
                }
                outputs
            }
            EngineInput::Paste(content) => self.paste(content),
        }
    }
    pub fn transact(&mut self, tx: Transaction) -> Result<Vec<EngineOutput>, EngineError> {
        // Without an adapter clock, separate commands form separate history entries.
        let time = self.time_ms.unwrap_or_else(|| {
            self.revision
                .saturating_mul(self.options.history_group_ms + 1)
        });
        self.transact_at(tx, time)
    }
    pub fn transact_at(
        &mut self,
        mut tx: Transaction,
        time_ms: u64,
    ) -> Result<Vec<EngineOutput>, EngineError> {
        if self.options.read_only || tx.ops.is_empty() {
            return Ok(Vec::new());
        }
        let inserted_ids = identity::prepare_insertions(&self.doc, &mut tx.ops);
        if tx.selection_after != tx.selection_before {
            tx.selection_after.remap_ids(&inserted_ids);
        }
        // Ordinary typing changes only one inline container. Avoid scanning the
        // entire document for identities or rebuilding paths in that hot path.
        let repair_tree = tx.ops.iter().any(|op| match op {
            Op::SetContent { before, after, .. } => !same_identities(before, after),
            Op::SetBlockAttrs { .. } | Op::SetMetadata { .. } => false,
            _ => true,
        });
        let existing = if repair_tree {
            existing_islands(&self.doc)
        } else {
            BTreeMap::new()
        };
        let mut plan = tx.apply_indexed(&mut self.doc, &self.index)?;
        self.map_pending(&tx);
        let mut additions = if repair_tree {
            identity::repairs(&self.doc, &existing)
        } else {
            Vec::new()
        };
        // Identity repair is an invertible part of the same transaction.
        for op in additions.drain(..) {
            if let Err(error) = op.apply(&mut self.doc) {
                let _ = tx.invert().apply(&mut self.doc);
                return Err(error);
            }
            tx.ops.push(op);
            plan.structure_changed = true;
        }
        let normalized = if self.batch.is_some() {
            Vec::new()
        } else if repair_tree {
            normalization_ops(&self.doc)
        } else {
            trailing_paragraph_ops(&self.doc)
        };
        for op in normalized {
            op.apply(&mut self.doc)?;
            tx.ops.push(op);
            plan.structure_changed = true;
        }
        self.selection = tx.selection_after.clone();
        self.refresh_index(&tx, &plan);
        self.clamp_selection();
        tx.selection_after = self.selection.clone();
        if let Some(batch) = &mut self.batch {
            batch.ops.extend(tx.ops);
            batch.dirty.extend(plan.dirty);
            batch.structure_changed |= plan.structure_changed;
            return Ok(Vec::new());
        }
        self.history.record(tx, time_ms);
        self.revision = self.revision.saturating_add(1);
        Ok(vec![
            EngineOutput::Render(plan.clone()),
            EngineOutput::ApplySelection(self.selection.clone()),
            EngineOutput::Changed {
                revision: self.revision,
                dirty_blocks: plan.dirty,
            },
        ])
    }
    pub fn undo(&mut self) -> Vec<EngineOutput> {
        if self.options.read_only {
            return Vec::new();
        }
        let Some(tx) = self.history.undo_transaction() else {
            return Vec::new();
        };
        match self.apply_history(&tx) {
            Ok(output) => {
                self.history.did_undo();
                output
            }
            Err(error) => vec![error_output(error)],
        }
    }
    pub fn redo(&mut self) -> Vec<EngineOutput> {
        if self.options.read_only {
            return Vec::new();
        }
        let Some(tx) = self.history.redo_transaction() else {
            return Vec::new();
        };
        match self.apply_history(&tx) {
            Ok(output) => {
                self.history.did_redo();
                output
            }
            Err(error) => vec![error_output(error)],
        }
    }
    fn apply_history(&mut self, tx: &Transaction) -> Result<Vec<EngineOutput>, EngineError> {
        let plan = tx.apply_indexed(&mut self.doc, &self.index)?;
        self.selection = tx.selection_after.clone();
        self.refresh_index(tx, &plan);
        self.clamp_selection();
        self.revision = self.revision.saturating_add(1);
        Ok(vec![
            EngineOutput::Render(plan.clone()),
            EngineOutput::ApplySelection(self.selection.clone()),
            EngineOutput::Changed {
                revision: self.revision,
                dirty_blocks: plan.dirty,
            },
        ])
    }
    pub fn replace_document(
        &mut self,
        mut doc: ComponentDocumentV2,
        policy: HistoryPolicy,
    ) -> Vec<EngineOutput> {
        for op in identity::repairs(&doc, &BTreeMap::new()) {
            if let Err(error) = op.apply(&mut doc) {
                return vec![error_output(error)];
            }
        }
        if let Err(error) = normalize_initial(&mut doc) {
            return vec![error_output(error)];
        }
        if let Err(error) = crate::validate_component_document(
            &doc,
            &self.schema.catalog,
            &ValidationLimits::default(),
            UnknownComponentPolicy::Reject,
        ) {
            return vec![error_output(error.into())];
        }
        if self.composing {
            self.queued_replace = Some((doc, policy));
            return Vec::new();
        }
        if policy == HistoryPolicy::Preserve && self.doc != doc {
            if self.history.can_undo() {
                let mut tx = Transaction::new(
                    replacement_ops(&self.doc, &doc),
                    self.selection.clone(),
                    self.selection.clone(),
                    TxKind::Host,
                );
                tx.history = HistoryMode::Merge;
                self.history.record(tx, self.time_ms.unwrap_or_default());
            } else {
                // Redo entries based on a replaced document cannot be replayed safely.
                self.history.clear();
            }
        }
        self.doc = doc;
        self.index = model::BlockIndex::new(&self.doc);
        self.clamp_selection();
        self.stored_marks.clear();
        self.stored_marks_explicit = false;
        self.pending.clear();
        self.manual_suggestion = false;
        self.suggestion.close();
        self.open_menu = None;
        if policy == HistoryPolicy::Reset {
            self.history.clear();
        } else {
            self.history.close_group();
        }
        vec![
            EngineOutput::Render(RenderPlan {
                full: true,
                structure_changed: true,
                dirty: self.index.order.clone(),
            }),
            EngineOutput::ApplySelection(self.selection.clone()),
        ]
    }
    pub(crate) fn clamp_selection(&mut self) {
        fn clamp(
            doc: &ComponentDocumentV2,
            index: &model::BlockIndex,
            point: &mut TextPoint,
        ) -> bool {
            if let Some(nodes) = model::inline_indexed(doc, index, &point.container) {
                point.offset = point.offset.min(text::len(nodes));
                true
            } else {
                false
            }
        }
        let valid = match &mut self.selection {
            EditorSelection::Text(range) => {
                clamp(&self.doc, &self.index, &mut range.anchor)
                    && clamp(&self.doc, &self.index, &mut range.head)
            }
            EditorSelection::Node(uid) => self.index.paths.contains_key(uid),
            EditorSelection::Blocks { anchor, head } => {
                self.index.paths.contains_key(anchor) && self.index.paths.contains_key(head)
            }
            EditorSelection::Cells {
                table,
                anchor,
                head,
            } => {
                if let Some(content) = self
                    .index
                    .get(&self.doc, table)
                    .and_then(|b| b.content.first())
                    .filter(|n| n.kind.0 == "table")
                {
                    let rows = content.content.len();
                    let cols = content.content.first().map_or(0, |r| r.content.len());
                    if rows > 0 && cols > 0 {
                        anchor.0 = anchor.0.min(rows - 1);
                        head.0 = head.0.min(rows - 1);
                        anchor.1 = anchor.1.min(cols - 1);
                        head.1 = head.1.min(cols - 1);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        };
        if !valid && let Some(selection) = model::first_caret(&self.doc) {
            self.selection = selection;
        }
    }
}
fn existing_islands(doc: &ComponentDocumentV2) -> BTreeMap<String, AttributeMap> {
    fn visit(node: &crate::ComponentNode, result: &mut BTreeMap<String, AttributeMap>) {
        if node.kind.0 == "block"
            && let Some(uid) = &node.id
            && let Some(content) = node.content.first()
            && identity::is_island_kind(&content.kind.0)
        {
            result.insert(uid.0.clone(), content.attrs.clone());
        }
        for child in &node.content {
            visit(child, result);
        }
    }
    let mut result = BTreeMap::new();
    visit(&doc.root, &mut result);
    result
}
fn normalize_initial(doc: &mut ComponentDocumentV2) -> Result<(), EngineError> {
    for op in normalization_ops(doc) {
        op.apply(doc)?;
    }
    Ok(())
}
fn normalization_ops(doc: &ComponentDocumentV2) -> Vec<Op> {
    let mut ops = Vec::new();
    if let Some(list) = model::children(doc, None) {
        if list.is_empty()
            || list
                .last()
                .and_then(|b| b.content.first())
                .is_some_and(|n| !model::is_text_kind(&n.kind.0))
        {
            ops.push(Op::InsertBlocks {
                parent: None,
                index: list.len(),
                blocks: vec![model::paragraph()],
            });
        }
    }
    fn visit(node: &crate::ComponentNode, ops: &mut Vec<Op>) {
        if node.kind.0 == "block"
            && let Some(uid) = &node.id
            && let Some(group) = node.content.iter().find(|n| n.kind.0 == "block_children")
            && group.content.is_empty()
        {
            ops.push(Op::SetChildren {
                uid: uid.0.clone(),
                before: Some(group.clone()),
                after: None,
            });
        }
        for child in &node.content {
            visit(child, ops);
        }
    }
    visit(&doc.root, &mut ops);
    ops
}
fn error_output(error: EngineError) -> EngineOutput {
    EngineOutput::Request(EngineEvent::Error {
        code: "editor_engine".into(),
        message: error.to_string(),
        recoverable: true,
    })
}

impl EditorEngine {
    fn publish_ui(&mut self, mut outputs: Vec<EngineOutput>) -> Vec<EngineOutput> {
        let ui = self.ui_state();
        if self.last_ui.as_ref() != Some(&ui) {
            self.last_ui = Some(ui.clone());
            outputs.push(EngineOutput::UiState(ui));
        }
        outputs
    }
    fn finish_outputs(&mut self, mut outputs: Vec<EngineOutput>) -> Vec<EngineOutput> {
        if self.composing {
            return outputs;
        }
        let next = if self.manual_suggestion && !self.options.read_only {
            self.suggestion.active.clone()
        } else if !self.options.read_only && self.selection.is_collapsed() {
            self.selection.text_range().and_then(|range| {
                let uid = commands::blocks::owner(self, &range.head.container)?;
                let kind = &self.content(&uid)?.kind.0;
                if matches!(kind.as_str(), "code_block" | "opaque_markdown") {
                    return None;
                }
                let before = text::plain(&text::slice(
                    self.inline(&range.head.container)?,
                    0,
                    range.head.offset,
                ));
                let aliases = self
                    .schema
                    .kinds
                    .values()
                    .filter_map(|s| s.insert.as_ref())
                    .flat_map(|s| std::iter::once(s.label.clone()).chain(s.keywords.clone()))
                    .collect::<Vec<_>>();
                suggestion::detect(
                    &before,
                    range.head.container.clone(),
                    &self.triggers,
                    self.profile == Profile::Composer,
                    &aliases,
                )
            })
        } else {
            None
        };
        if self.suggestion.update(next)
            && let Some(matched) = self.suggestion.active.clone()
        {
            if matched.trigger == "/" && self.profile != Profile::Composer {
                let items = self.slash_items(&matched.query);
                self.suggestion.set_items(self.suggestion.generation, items);
            } else if self.options.host_entity_picker
                && matched.trigger == "@"
                && matched.query.is_empty()
                && self.typed_mention_trigger
            {
                self.typed_mention_trigger = false;
                let request_id = self.next_request();
                let event = EngineEvent::EntityPickerRequest {
                    request_id,
                    mode: "mention".into(),
                };
                self.pending.insert(
                    request_id,
                    PendingRequest {
                        selection: EditorSelection::Text(matched.range),
                        kind: PendingKind::Picker("mention".into()),
                    },
                );
                self.suggestion.dismiss();
                outputs.push(EngineOutput::Request(event));
            } else {
                outputs.push(EngineOutput::QuerySuggestions {
                    generation: self.suggestion.generation,
                    trigger: matched.trigger,
                    query: matched.query,
                });
            }
        }
        self.publish_ui(outputs)
    }
    pub fn slash_items(&self, query: &str) -> Vec<crate::SuggestionItem> {
        let mut items = Vec::new();
        for spec in self.schema.kinds.values() {
            let Some(insert) = &spec.insert else {
                continue;
            };
            if self.profile == Profile::Composer || !self.schema.has_kind(&spec.id.0) {
                continue;
            }
            let levels = if spec.id.0 == "heading" {
                vec![1, 2, 3]
            } else {
                vec![0]
            };
            for level in levels {
                let label = if level > 0 {
                    format!("Heading {level}")
                } else {
                    insert.label.clone()
                };
                let attrs = match spec.id.0.as_str() {
                    "heading" => serde_json::json!({"level":level}),
                    "todo" => serde_json::json!({"checked":false}),
                    "callout" => serde_json::json!({"icon":"💡"}),
                    "code_block" => serde_json::json!({"wrap":false}),
                    _ => serde_json::json!({}),
                };
                let terms = format!("{} {}", label, insert.keywords.join(" "));
                let Some(score) = suggestion::fuzzy_score(query, &terms) else {
                    continue;
                };
                items.push(crate::SuggestionItem{id:if level>0{format!("{}:{level}",spec.id.0)}else{spec.id.0.clone()},label,description:Some(insert.description.clone()),icon:Some(insert.icon.clone()),data:serde_json::json!({"kind":spec.id.0,"attrs":attrs,"group":format!("{:?}",insert.group).to_lowercase(),"shortcut":insert.shortcut}),priority:-(score as i32)});
            }
        }
        if self.triggers.iter().any(|trigger| trigger == "@") {
            for (kind, label, mode) in [
                ("mention", "Mention", "mention"),
                ("entity_embed", "Embed entity", "embed"),
                ("page", "Sub-document", "page"),
            ] {
                if kind == "entity_embed" && !self.schema.has_kind(kind)
                    || kind == "page" && !self.schema.has_kind("entity_embed")
                {
                    continue;
                }
                if let Some(score) =
                    suggestion::fuzzy_score(query, &format!("{kind} {label} {mode}"))
                {
                    items.push(crate::SuggestionItem {
                        id: kind.into(),
                        label: label.into(),
                        description: Some("Link your content to an entity".into()),
                        icon: Some("@".into()),
                        data: serde_json::json!({"kind":kind,"mode":mode,"group":"semantic"}),
                        priority: -(score as i32),
                    });
                }
            }
        }
        items.retain(|item| {
            item.data.get("kind").and_then(serde_json::Value::as_str) != Some("entity_embed")
                || item.data.get("mode").is_some()
        });
        items.sort_by_key(|i| {
            (
                -i.priority,
                match i
                    .data
                    .get("group")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                {
                    "basic" => 0,
                    "media" => 1,
                    "semantic" => 2,
                    "advanced" => 3,
                    _ => 4,
                },
                i.label.clone(),
            )
        });
        if query.is_empty() {
            let mut recent = self
                .recent_blocks
                .iter()
                .filter_map(|id| items.iter().find(|item| &item.id == id).cloned())
                .map(|mut item| {
                    item.data["group"] = serde_json::json!("Recent");
                    item
                })
                .collect::<Vec<_>>();
            recent.extend(items);
            items = recent;
        }
        items
    }
    fn handle_key(&mut self, key: KeyChord) -> Vec<EngineOutput> {
        let Some(action) = keymap::resolve(self, &key) else {
            return Vec::new();
        };
        match action {
            keymap::KeyAction::Command(command) => self.run(command),
            keymap::KeyAction::MoveToTextEdge { end } => {
                if let Some(range) = self.selection.text_range() {
                    let container = range.head.container.clone();
                    let offset = if end {
                        self.inline(&container).map_or(0, |n| text::len(n))
                    } else {
                        0
                    };
                    self.run(Command::SetSelection(EditorSelection::caret(
                        container, offset,
                    )))
                } else {
                    Vec::new()
                }
            }
            action => {
                let event = match action {
                    keymap::KeyAction::Submit { alternate } => ComposerAction::Submit { alternate },
                    keymap::KeyAction::BackgroundSubmit => ComposerAction::BackgroundSubmit,
                    keymap::KeyAction::HistoryPrevious => ComposerAction::HistoryPrevious,
                    keymap::KeyAction::HistoryNext => ComposerAction::HistoryNext,
                    keymap::KeyAction::Stash => ComposerAction::Stash,
                    keymap::KeyAction::PlainPaste => ComposerAction::PlainPaste,
                    _ => return Vec::new(),
                };
                self.suggestion.dismiss();
                self.publish_ui(vec![EngineOutput::Composer(event)])
            }
        }
    }
    fn accept_suggestion(&mut self) -> Vec<EngineOutput> {
        self.batch_edit(TxKind::Command, |engine| engine.accept_suggestion_inner())
    }
    fn accept_suggestion_inner(&mut self) -> Vec<EngineOutput> {
        let (Some(matched), Some(item)) = (
            self.suggestion.active.clone(),
            self.suggestion.selected().cloned(),
        ) else {
            return vec![];
        };
        self.manual_suggestion = false;
        self.suggestion.dismiss();
        self.selection = EditorSelection::Text(matched.range.clone());
        let mut outputs = Vec::new();
        if self.profile == Profile::Composer {
            if let Some(chip) = item.data.get("chip") {
                let mut node = crate::ComponentNode::new("chip");
                for key in ["kind", "label"] {
                    if let Some(value) = chip.get(key) {
                        node.attrs.insert(key.into(), value.clone());
                    }
                }
                node.attrs.insert(
                    "payload".into(),
                    serde_json::Value::String(
                        chip.get("payload")
                            .unwrap_or(&serde_json::Value::Null)
                            .to_string(),
                    ),
                );
                outputs.extend(commands::run(self, Command::InsertInline(vec![node])));
            } else if let Some(value) = item.data.get("text").and_then(serde_json::Value::as_str) {
                if item
                    .data
                    .get("at_start")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    outputs.extend(commands::run(
                        self,
                        Command::Delete {
                            direction: text::Direction::Backward,
                            unit: text::DeleteUnit::Selection,
                        },
                    ));
                    if let Some(uid) = self.index.order.first() {
                        self.selection =
                            EditorSelection::caret(InlineContainerRef::Block(uid.clone()), 0);
                    }
                }
                outputs.extend(commands::run(self, Command::InsertText(value.into())));
            } else {
                outputs.extend(commands::run(
                    self,
                    Command::Delete {
                        direction: text::Direction::Backward,
                        unit: text::DeleteUnit::Selection,
                    },
                ));
            }
            outputs.push(EngineOutput::Composer(ComposerAction::SuggestionAccepted(
                item,
            )));
            return outputs;
        }
        let remembered = item.clone();
        let trigger = matched.trigger.clone();
        if matched.trigger == "/" {
            self.recent_blocks.retain(|id| id != &item.id);
            self.recent_blocks.insert(0, item.id.clone());
            self.recent_blocks.truncate(5);
            outputs.extend(commands::run(
                self,
                Command::Delete {
                    direction: text::Direction::Backward,
                    unit: text::DeleteUnit::Selection,
                },
            ));
            let Some(kind) = item.data.get("kind").and_then(serde_json::Value::as_str) else {
                return outputs;
            };
            let attrs = item
                .data
                .get("attrs")
                .and_then(serde_json::Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(mode) = item.data.get("mode").and_then(serde_json::Value::as_str) {
                if mode == "page" {
                    outputs.extend(self.request_entity_create("page".into(), String::new()));
                } else {
                    outputs.extend(self.request_entity_picker(mode.into()));
                }
            } else if kind == "image" && self.profile == Profile::MarkdownNotes {
                self.open_menu = Some("image-url".into());
            } else if kind == "bookmark" {
                self.open_menu = Some("bookmark".into());
            } else if model::is_text_kind(kind) {
                outputs.extend(commands::run(
                    self,
                    Command::TurnInto {
                        kind: kind.into(),
                        attrs,
                    },
                ));
            } else if kind == "table" {
                outputs.extend(commands::run(
                    self,
                    Command::Table(commands::TableCommand::Insert {
                        rows: 3,
                        columns: 3,
                        header_row: true,
                    }),
                ));
            } else {
                let mut content = crate::ComponentNode::new(kind);
                content.attrs = attrs;
                let block = model::block_node(content);
                let uid = block.id.as_ref().unwrap().0.clone();
                outputs.extend(commands::run(
                    self,
                    Command::InsertBlocks {
                        at: crate::InsertAt::Selection,
                        nodes: vec![block],
                    },
                ));
                if matches!(kind, "image" | "file") {
                    outputs.push(EngineOutput::Request(EngineEvent::FilePickerRequest {
                        uid,
                        accept: if kind == "image" { "image/*" } else { "*" }.into(),
                    }));
                }
            }
        } else if let Some(entity) = item
            .data
            .get("entity")
            .and_then(|v| serde_json::from_value::<crate::EntityLinkCandidate>(v.clone()).ok())
        {
            let mode = self.suggestion_mode.clone();
            outputs.extend(self.insert_entity(entity, &mode));
            self.suggestion_mode = "mention".into();
        } else if item.data.get("create").and_then(serde_json::Value::as_bool) == Some(true) {
            outputs.extend(self.request_entity_create(matched.trigger, matched.query));
        }
        outputs.push(EngineOutput::RememberSuggestion {
            trigger,
            item: remembered,
        });
        outputs
    }
    pub fn request_entity_picker(&mut self, mode: String) -> Vec<EngineOutput> {
        let request_id = self.next_request();
        self.run(Command::Request(EngineEvent::EntityPickerRequest {
            request_id,
            mode,
        }))
    }
    pub fn request_entity_create(&mut self, trigger: String, query: String) -> Vec<EngineOutput> {
        let request_id = self.next_request();
        let uids = commands::blocks::selected_uids(self);
        let checked = commands::blocks::current_uid(self)
            .and_then(|uid| self.content(&uid))
            .and_then(|n| n.attrs.get("checked"))
            .and_then(serde_json::Value::as_bool);
        self.run(Command::Request(EngineEvent::EntityCreateRequest {
            request_id,
            query,
            trigger,
            uids,
            checked,
        }))
    }
    fn next_request(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }
    fn prepare_request(&self, event: &mut EngineEvent) {
        if let EngineEvent::EntityCreateRequest {
            query,
            trigger,
            uids,
            checked,
            ..
        } = event
        {
            *uids = commands::blocks::selected_uids(self);
            if query.is_empty() {
                if let Some(range) = self.selection.text_range()
                    && range.anchor.container == range.head.container
                    && !range.is_collapsed()
                {
                    if let Some(nodes) = self.inline(&range.head.container) {
                        *query = text::plain(&text::slice(
                            nodes,
                            range.anchor.offset.min(range.head.offset),
                            range.anchor.offset.max(range.head.offset),
                        ));
                    }
                }
                if query.is_empty() {
                    *query = commands::blocks::current_uid(self)
                        .and_then(|uid| self.content(&uid))
                        .map(|n| text::plain(&n.content))
                        .unwrap_or_default();
                }
            }
            if trigger == "task" {
                *checked = commands::blocks::current_uid(self)
                    .and_then(|uid| self.content(&uid))
                    .and_then(|n| n.attrs.get("checked"))
                    .and_then(serde_json::Value::as_bool);
            }
        }
    }
    fn capture_request(&mut self, event: &EngineEvent) {
        let request = match event {
            EngineEvent::EntityPickerRequest { request_id, mode } => {
                Some((*request_id, PendingKind::Picker(mode.clone())))
            }
            EngineEvent::EntityCreateRequest {
                request_id,
                trigger,
                ..
            } => {
                let uid = commands::blocks::current_uid(self);
                let content = uid.as_ref().and_then(|uid| self.content(uid)).cloned();
                Some((
                    *request_id,
                    PendingKind::Create {
                        trigger: trigger.clone(),
                        uid,
                        content,
                    },
                ))
            }
            EngineEvent::ConvertMarkdownRequest {
                request_id,
                markdown,
            } => Some((*request_id, PendingKind::Markdown(markdown.clone()))),
            _ => None,
        };
        if let Some((id, kind)) = request {
            self.pending.insert(
                id,
                PendingRequest {
                    selection: self.selection.clone(),
                    kind,
                },
            );
        }
    }
    fn host_response(&mut self, response: HostResponse) -> Vec<EngineOutput> {
        if self.options.read_only {
            return Vec::new();
        }
        let (request_id, entity) = match response {
            HostResponse::EntityPicker { request_id, entity }
            | HostResponse::EntityCreate { request_id, entity } => (request_id, entity),
            HostResponse::Markdown { request_id, nodes } => {
                let Some(request) = self.pending.remove(&request_id) else {
                    return vec![];
                };
                if !selection_exists(&self.doc, &request.selection) {
                    return vec![];
                }
                self.selection = request.selection;
                let nodes = nodes.unwrap_or_else(|| {
                    if let PendingKind::Markdown(text) = request.kind {
                        clipboard::plain_blocks(&text)
                    } else {
                        Vec::new()
                    }
                });
                return self.insert_pasted_blocks(nodes);
            }
            HostResponse::MediaUrl { .. } => return Vec::new(),
        };
        let Some(request) = self.pending.remove(&request_id) else {
            return vec![];
        };
        let Some(entity) = entity else {
            self.suggestion.dismiss();
            return self.publish_ui(vec![EngineOutput::Focus {
                target: FocusTarget::Editor,
            }]);
        };
        if !selection_exists(&self.doc, &request.selection) {
            return vec![];
        }
        self.selection = request.selection;
        let output = match request.kind {
            PendingKind::Picker(mode) => self.insert_entity(entity, &mode),
            PendingKind::Create {
                trigger,
                uid,
                content,
            } => {
                if trigger == "selection" || trigger == "@" {
                    self.insert_entity(entity, "mention")
                } else {
                    if matches!(trigger.as_str(), "task" | "page")
                        && uid.as_ref().and_then(|uid| self.content(uid)) != content.as_ref()
                    {
                        return vec![];
                    }
                    self.complete_embed(uid, entity, &trigger)
                }
            }
            PendingKind::Markdown(_) => Vec::new(),
        };
        self.finish_outputs(output)
    }
    fn complete_embed(
        &mut self,
        uid: Option<Uid>,
        entity: crate::EntityLinkCandidate,
        trigger: &str,
    ) -> Vec<EngineOutput> {
        let Some(uid) = uid else {
            return Vec::new();
        };
        let Some(before) = self.content(&uid).cloned() else {
            return Vec::new();
        };
        let Some((parent, index)) = self.location(&uid) else {
            return Vec::new();
        };
        let mut inline_after = before.clone();
        if trigger == "[["
            && let Some(range) = self.selection.text_range()
            && range.anchor.container == range.head.container
        {
            inline_after.content = text::replace(
                &before.content,
                range.anchor.offset.min(range.head.offset),
                range.anchor.offset.max(range.head.offset),
                Vec::new(),
            );
        }
        let mut embed = crate::ComponentNode::new("entity_embed");
        embed.attrs = serde_json::from_value(serde_json::json!({"entity":entity.id,"view":"link"}))
            .unwrap_or_default();
        let mut ops = Vec::new();
        let selection;
        if trigger == "[[" && !inline_after.content.is_empty() {
            if inline_after != before {
                ops.push(Op::SetContent {
                    uid: uid.clone(),
                    before,
                    after: inline_after,
                });
            }
            let block = model::block_node(embed);
            let next = block.id.as_ref().unwrap().0.clone();
            ops.push(Op::InsertBlocks {
                parent,
                index: index + 1,
                blocks: vec![block],
            });
            selection = EditorSelection::Node(next);
        } else {
            ops.push(Op::SetContent {
                uid: uid.clone(),
                before,
                after: embed,
            });
            if let Some(group) = self
                .block(&uid)
                .and_then(|block| block.content.iter().find(|n| n.kind.0 == "block_children"))
                .cloned()
            {
                ops.push(Op::SetChildren {
                    uid: uid.clone(),
                    before: Some(group.clone()),
                    after: None,
                });
                if !group.content.is_empty() {
                    ops.push(Op::InsertBlocks {
                        parent,
                        index: index + 1,
                        blocks: group.content,
                    });
                }
            }
            selection = EditorSelection::Node(uid);
        }
        self.transact(Transaction::new(
            ops,
            self.selection.clone(),
            selection,
            TxKind::Host,
        ))
        .unwrap_or_else(|error| vec![error_output(error)])
    }
    fn insert_entity(
        &mut self,
        entity: crate::EntityLinkCandidate,
        mode: &str,
    ) -> Vec<EngineOutput> {
        if mode == "link" && !self.selection.is_collapsed() {
            let mut mark = ComponentMark::new("entity_link");
            mark.attrs
                .insert("entity_id".into(), serde_json::json!(entity.id));
            return commands::run(self, Command::SetMark(mark));
        }
        if mode == "embed" {
            let mut node = crate::ComponentNode::new("entity_embed");
            node.attrs =
                serde_json::from_value(serde_json::json!({"entity":entity.id,"view":"card"}))
                    .unwrap_or_default();
            return commands::run(
                self,
                Command::InsertBlocks {
                    at: crate::InsertAt::Selection,
                    nodes: vec![model::block_node(node)],
                },
            );
        }
        self.recent_mentions
            .retain(|candidate| candidate.id != entity.id);
        self.recent_mentions.insert(0, entity.clone());
        self.recent_mentions.truncate(5);
        let remembered = crate::SuggestionItem {
            id: entity.id.clone(),
            label: entity.label.clone(),
            description: entity.detail.clone(),
            icon: None,
            data: serde_json::json!({"entity":entity}),
            priority: 0,
        };
        let mut node = crate::ComponentNode::new("mention");
        node.attrs =
            serde_json::from_value(serde_json::json!({"entity_id":entity.id,"label":entity.label}))
                .unwrap_or_default();
        let mut outputs = commands::run(self, Command::InsertInline(vec![node]));
        outputs.push(EngineOutput::RememberSuggestion {
            trigger: "@".into(),
            item: remembered,
        });
        outputs
    }
    fn map_pending(&mut self, tx: &Transaction) {
        for pending in self.pending.values_mut() {
            if let EditorSelection::Text(range) = &mut pending.selection {
                map_point(&mut range.anchor, &tx.ops, false);
                map_point(&mut range.head, &tx.ops, true);
            }
        }
    }
    fn paste(&mut self, content: clipboard::PasteContent) -> Vec<EngineOutput> {
        if self.options.read_only {
            return Vec::new();
        }
        let text = content
            .text
            .as_deref()
            .map(clipboard::normalize_line_endings)
            .unwrap_or_default();
        if let Some(uid) = commands::blocks::current_uid(self)
            && self.content(&uid).is_some_and(|n| n.kind.0 == "code_block")
        {
            return self.run(Command::InsertText(text));
        }
        if self.profile == Profile::Composer {
            if !content.plain_mode
                && let Some(slice) = content.internal
            {
                let blocks =
                    html::restrict_to_catalog(slice.content, &self.schema.catalog, &|_| false);
                return self.insert_pasted_blocks(blocks);
            }
            let text = if text.is_empty() && !content.plain_mode {
                content
                    .html
                    .as_ref()
                    .map(|html| html::blocks_to_plain_text(&html::html_to_blocks(html)))
                    .unwrap_or_default()
            } else {
                text
            };
            return if text.contains('\n') {
                self.insert_pasted_blocks(clipboard::plain_blocks(&text))
            } else {
                self.run(Command::InsertText(text))
            };
        }
        if matches!(&self.selection, EditorSelection::Cells { .. })
            || matches!(&self.selection,EditorSelection::Text(range) if matches!(range.head.container,InlineContainerRef::Cell{..}))
        {
            if !content.plain_mode {
                let imported = content
                    .internal
                    .as_ref()
                    .map(|slice| slice.content.clone())
                    .or_else(|| content.html.as_ref().map(html::html_to_blocks))
                    .map(|blocks| {
                        html::restrict_to_catalog(blocks, &self.schema.catalog, &|mark| {
                            self.profile.allows_mark(mark)
                        })
                    });
                if let Some(blocks) = imported
                    && let Some(table) = blocks
                        .iter()
                        .filter_map(|b| b.content.first())
                        .find(|n| n.kind.0 == "table")
                {
                    let grid = table
                        .content
                        .iter()
                        .map(|row| {
                            row.content
                                .iter()
                                .map(|cell| cell.content.clone())
                                .collect()
                        })
                        .collect();
                    return self.run(Command::Table(commands::TableCommand::PasteCells(grid)));
                }
            }
            if let Some(grid) = clipboard::parse_tsv_grid(&text) {
                return self.run(Command::Table(commands::TableCommand::PasteGrid(grid)));
            }
            let nodes = text
                .split('\n')
                .enumerate()
                .flat_map(|(i, line)| {
                    std::iter::once((i > 0).then(|| crate::ComponentNode::new("hard_break")))
                        .flatten()
                        .chain(std::iter::once(crate::ComponentNode::text(line)))
                })
                .collect();
            return self.run(Command::InsertInline(nodes));
        }
        if !content.plain_mode {
            if let Some(slice) = content.internal {
                let blocks =
                    html::restrict_to_catalog(slice.content, &self.schema.catalog, &|mark| {
                        self.profile.allows_mark(mark)
                    });
                return if slice.open_start == 0 && slice.open_end == 0 {
                    self.insert_whole_blocks_mode(blocks, true)
                } else {
                    self.insert_pasted_blocks(blocks)
                };
            }
            let explicit_url = text.contains(':')
                || text.starts_with('/')
                || text.starts_with('#')
                || text.starts_with("./")
                || text.starts_with("../");
            if explicit_url
                && let Some(url) = clipboard::classify_url(&text, self.options.origin.as_deref())
            {
                let mut mark = match url {
                    clipboard::ClassifiedUrl::Entity { id, .. } => {
                        let mut mark = ComponentMark::new("entity_link");
                        mark.attrs.insert("entity_id".into(), serde_json::json!(id));
                        mark
                    }
                    clipboard::ClassifiedUrl::External(href) => {
                        let mut mark = ComponentMark::new("link");
                        mark.attrs.insert("href".into(), serde_json::json!(href));
                        mark
                    }
                };
                // The catalog can omit semantic links even in a block profile.
                if !self.schema.marks.contains(&mark.kind.0) {
                    if !crate::is_safe_url(&text) {
                        return self.run(Command::InsertText(text));
                    }
                    mark = ComponentMark::new("link");
                    mark.attrs.insert("href".into(), serde_json::json!(text));
                }
                if self.schema.marks.contains(&mark.kind.0) {
                    if !self.selection.is_collapsed() {
                        return self.run(Command::SetMark(mark));
                    }
                    let mut node = crate::ComponentNode::text(text);
                    node.marks = vec![mark];
                    return self.run(Command::InsertInline(vec![node]));
                }
            }
            if let Some(html) = content.html {
                let blocks = html::restrict_to_catalog(
                    html::html_to_blocks(&html),
                    &self.schema.catalog,
                    &|mark| self.profile.allows_mark(mark),
                );
                if !blocks.is_empty() {
                    return self.insert_pasted_blocks(blocks);
                }
            }
            if clipboard::looks_like_markdown(&text) && self.profile != Profile::Composer {
                #[cfg(feature = "markdown")]
                if self.profile == Profile::MarkdownNotes {
                    use crate::DocumentFormat;
                    let standard = crate::ComponentCatalog::standard();
                    if let Ok(catalog) = standard {
                        if let Ok(decoded) = crate::MarkdownDocumentFormat::new().decode(
                            &crate::EditorPayload::new("markdown", serde_json::json!(text)),
                            &catalog,
                            crate::DecodeOptions {
                                unknown_components: UnknownComponentPolicy::PreserveOpaque,
                                ..Default::default()
                            },
                        ) {
                            let blocks = markdown_blocks::to_blocks(&decoded.document);
                            return self.insert_pasted_blocks(
                                model::children(&blocks, None).cloned().unwrap_or_default(),
                            );
                        }
                    }
                }
                let request_id = self.next_request();
                return self.run(Command::Request(EngineEvent::ConvertMarkdownRequest {
                    request_id,
                    markdown: text,
                }));
            }
        }
        if text.contains('\n') {
            self.insert_pasted_blocks(clipboard::plain_blocks(&text))
        } else {
            self.run(Command::InsertText(text))
        }
    }
    fn insert_whole_blocks(&mut self, blocks: Vec<crate::ComponentNode>) -> Vec<EngineOutput> {
        self.insert_whole_blocks_mode(blocks, false)
    }
    fn insert_whole_blocks_mode(
        &mut self,
        mut blocks: Vec<crate::ComponentNode>,
        keep_block_selection: bool,
    ) -> Vec<EngineOutput> {
        for block in &mut blocks {
            identity::rekey(block);
        }
        let mut commands = Vec::new();
        if matches!(&self.selection,EditorSelection::Text(range) if !range.is_collapsed()) {
            commands.push(Command::Delete {
                direction: text::Direction::Backward,
                unit: text::DeleteUnit::Selection,
            });
        }
        let final_caret = blocks.last().and_then(|block| {
            let uid = block.id.as_ref()?.0.clone();
            let content = block.content.first()?;
            if model::is_text_kind(&content.kind.0) {
                Some(EditorSelection::caret(
                    InlineContainerRef::Block(uid),
                    text::len(&content.content),
                ))
            } else if content.kind.0 == "image" {
                Some(EditorSelection::caret(
                    InlineContainerRef::Caption(uid),
                    text::len(&content.content),
                ))
            } else {
                None
            }
        });
        commands.push(Command::InsertBlocks {
            at: crate::InsertAt::Selection,
            nodes: blocks,
        });
        if (!keep_block_selection || self.profile == Profile::Composer)
            && let Some(selection) = final_caret
        {
            commands.push(Command::SetSelection(selection));
        }
        self.batch_commands(commands, TxKind::Paste)
    }
    fn insert_pasted_blocks(&mut self, mut blocks: Vec<crate::ComponentNode>) -> Vec<EngineOutput> {
        if blocks.is_empty() {
            return Vec::new();
        }
        for block in &mut blocks {
            identity::rekey(block);
        }
        // Plain/HTML paragraph slices merge at the caret. Whole internal blocks retain boundaries.
        if let Some(range) = self
            .selection
            .text_range()
            .filter(|r| r.anchor.container == r.head.container)
            .cloned()
        {
            if let InlineContainerRef::Block(uid) = &range.head.container {
                if blocks
                    .iter()
                    .all(|b| b.content.first().is_some_and(|n| n.kind.0 == "paragraph"))
                    && self
                        .content(uid)
                        .is_some_and(|n| model::is_text_kind(&n.kind.0) && !n.content.is_empty())
                {
                    let Some(before) = self.content(uid).cloned() else {
                        return vec![];
                    };
                    let start = range.anchor.offset.min(range.head.offset);
                    let end = range.anchor.offset.max(range.head.offset);
                    let (mut head, _) = text::split(&before.content, start);
                    let (_, tail) = text::split(&before.content, end);
                    head.extend(blocks[0].content[0].content.clone());
                    let mut after = before.clone();
                    if blocks.len() == 1 {
                        head.extend(tail);
                        after.content = text::compact(head);
                        let point = EditorSelection::caret(
                            range.head.container.clone(),
                            start + text::len(&blocks[0].content[0].content),
                        );
                        let tx = Transaction::new(
                            vec![Op::SetContent {
                                uid: uid.clone(),
                                before,
                                after,
                            }],
                            self.selection.clone(),
                            point,
                            TxKind::Paste,
                        );
                        return self.transact(tx).unwrap_or_else(|e| vec![error_output(e)]);
                    }
                    after.content = text::compact(head);
                    blocks.remove(0);
                    let last = blocks.last_mut().unwrap();
                    let offset = text::len(&last.content[0].content);
                    last.content[0].content.extend(tail);
                    let last_uid = last.id.as_ref().unwrap().0.clone();
                    let Some((parent, index)) = self.location(uid) else {
                        return vec![];
                    };
                    let tx = Transaction::new(
                        vec![
                            Op::SetContent {
                                uid: uid.clone(),
                                before,
                                after,
                            },
                            Op::InsertBlocks {
                                parent,
                                index: index + 1,
                                blocks,
                            },
                        ],
                        self.selection.clone(),
                        EditorSelection::caret(InlineContainerRef::Block(last_uid), offset),
                        TxKind::Paste,
                    );
                    return self.transact(tx).unwrap_or_else(|e| vec![error_output(e)]);
                }
            }
        }
        self.insert_whole_blocks(blocks)
    }
}
fn selection_exists(doc: &ComponentDocumentV2, selection: &EditorSelection) -> bool {
    match selection {
        EditorSelection::Text(range) => {
            model::inline(doc, &range.anchor.container).is_some()
                && model::inline(doc, &range.head.container).is_some()
        }
        EditorSelection::Node(uid) => model::block(doc, uid).is_some(),
        EditorSelection::Blocks { anchor, head } => {
            model::block(doc, anchor).is_some() && model::block(doc, head).is_some()
        }
        EditorSelection::Cells { table, .. } => model::content(doc, table).is_some(),
    }
}
fn map_point(point: &mut TextPoint, ops: &[Op], after_affinity: bool) {
    for op in ops {
        if let Op::SetContent { uid, before, after } = op {
            let containers = match &point.container {
                InlineContainerRef::Block(id) | InlineContainerRef::Caption(id) if id == uid => {
                    Some((&before.content, &after.content))
                }
                InlineContainerRef::Cell { row, col } => model::find(before, row).and_then(|a| {
                    model::find(after, row).and_then(|b| {
                        Some((&a.content.get(*col)?.content, &b.content.get(*col)?.content))
                    })
                }),
                _ => None,
            };
            if let Some((before, after)) = containers {
                let a = text::plain(before).chars().collect::<Vec<_>>();
                let b = text::plain(after).chars().collect::<Vec<_>>();
                let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
                let suffix = a[prefix..]
                    .iter()
                    .rev()
                    .zip(b[prefix..].iter().rev())
                    .take_while(|(a, b)| a == b)
                    .count();
                if point.offset > prefix || point.offset == prefix && after_affinity {
                    if point.offset >= a.len() - suffix {
                        point.offset = (b.len() - suffix)
                            .saturating_add(point.offset.saturating_sub(a.len() - suffix));
                    } else {
                        point.offset = if after_affinity {
                            b.len() - suffix
                        } else {
                            prefix
                        };
                    }
                }
            }
        }
    }
}
fn same_identities(a: &crate::ComponentNode, b: &crate::ComponentNode) -> bool {
    fn collect(node: &crate::ComponentNode, ids: &mut Vec<String>) {
        if let Some(id) = &node.id {
            ids.push(id.0.clone());
        }
        if identity::is_island_kind(&node.kind.0) {
            ids.extend(identity::owned_uids(&serde_json::Value::Object(
                node.attrs.clone(),
            )));
        }
        for child in &node.content {
            collect(child, ids);
        }
    }
    let mut before = Vec::new();
    let mut after = Vec::new();
    collect(a, &mut before);
    collect(b, &mut after);
    before == after
}
fn trailing_paragraph_ops(doc: &ComponentDocumentV2) -> Vec<Op> {
    if let Some(list) = model::children(doc, None)
        && (list.is_empty()
            || list
                .last()
                .and_then(|b| b.content.first())
                .is_some_and(|n| !model::is_text_kind(&n.kind.0)))
    {
        return vec![Op::InsertBlocks {
            parent: None,
            index: list.len(),
            blocks: vec![model::paragraph()],
        }];
    }
    Vec::new()
}
impl EditorEngine {
    /// Run a compound gesture as one undo/revision/change, retaining block-level ops.
    pub fn batch_commands(&mut self, commands: Vec<Command>, kind: TxKind) -> Vec<EngineOutput> {
        self.batch_edit(kind, move |engine| {
            commands
                .into_iter()
                .flat_map(|command| engine.run(command))
                .collect()
        })
    }
    fn batch_edit(
        &mut self,
        kind: TxKind,
        edit: impl FnOnce(&mut Self) -> Vec<EngineOutput>,
    ) -> Vec<EngineOutput> {
        if self.options.read_only {
            return Vec::new();
        }
        if self.batch.is_some() {
            return edit(self);
        }
        self.batch = Some(BatchState {
            ops: Vec::new(),
            selection_before: self.selection.clone(),
            kind,
            dirty: BTreeSet::new(),
            structure_changed: false,
        });
        let mut requests = edit(self)
            .into_iter()
            .filter(|output| {
                matches!(
                    output,
                    EngineOutput::Request(_)
                        | EngineOutput::Composer(_)
                        | EngineOutput::Focus { .. }
                        | EngineOutput::Scroll { .. }
                        | EngineOutput::EntityOpen { .. }
                        | EngineOutput::RememberSuggestion { .. }
                        | EngineOutput::QuerySuggestions { .. }
                )
            })
            .collect::<Vec<_>>();
        let mut batch = self.batch.take().unwrap();
        if requests
            .iter()
            .any(|output| matches!(output, EngineOutput::Request(EngineEvent::Error { .. })))
        {
            let tx = Transaction::new(
                batch.ops,
                self.selection.clone(),
                batch.selection_before.clone(),
                kind,
            )
            .invert();
            let _ = tx.apply(&mut self.doc);
            self.selection = batch.selection_before;
            self.index = model::BlockIndex::new(&self.doc);
            return self.finish_outputs(requests);
        }
        for op in normalization_ops(&self.doc) {
            if let Err(error) = op.apply(&mut self.doc) {
                requests.push(error_output(error));
            } else {
                batch.ops.push(op);
                batch.structure_changed = true;
            }
        }
        if batch.ops.is_empty() {
            return self.finish_outputs(requests);
        }
        if batch.structure_changed {
            self.index = model::BlockIndex::new(&self.doc);
        }
        self.clamp_selection();
        let tx = Transaction::new(
            batch.ops,
            batch.selection_before,
            self.selection.clone(),
            batch.kind,
        );
        self.history.record(
            tx,
            self.time_ms.unwrap_or_else(|| {
                self.revision
                    .saturating_mul(self.options.history_group_ms + 1)
            }),
        );
        self.revision = self.revision.saturating_add(1);
        let dirty = batch.dirty.into_iter().collect::<Vec<_>>();
        requests.splice(
            0..0,
            [
                EngineOutput::Render(RenderPlan {
                    dirty: dirty.clone(),
                    structure_changed: batch.structure_changed,
                    full: false,
                }),
                EngineOutput::ApplySelection(self.selection.clone()),
                EngineOutput::Changed {
                    revision: self.revision,
                    dirty_blocks: dirty,
                },
            ],
        );
        self.finish_outputs(requests)
    }
}
/// Fold an external replacement into existing history using the same block ops
/// as local edits. Its inverse restores the exact state expected by older ops.
fn replacement_ops(before: &ComponentDocumentV2, after: &ComponentDocumentV2) -> Vec<Op> {
    fn groups(
        before: &[crate::ComponentNode],
        after: &[crate::ComponentNode],
        parent: Option<Uid>,
        ops: &mut Vec<Op>,
    ) {
        let same_order =
            before.len() == after.len() && before.iter().zip(after).all(|(a, b)| a.id == b.id);
        if !same_order {
            if !before.is_empty() {
                ops.push(Op::RemoveBlocks {
                    parent: parent.clone(),
                    index: 0,
                    blocks: before.to_vec(),
                });
            }
            if !after.is_empty() {
                ops.push(Op::InsertBlocks {
                    parent,
                    index: 0,
                    blocks: after.to_vec(),
                });
            }
            return;
        }
        for (a, b) in before.iter().zip(after) {
            let Some(uid) = a.id.as_ref().map(|id| id.0.clone()) else {
                continue;
            };
            if a.attrs != b.attrs {
                ops.push(Op::SetBlockAttrs {
                    uid: uid.clone(),
                    before: a.attrs.clone(),
                    after: b.attrs.clone(),
                });
            }
            if let (Some(ac), Some(bc)) = (a.content.first(), b.content.first())
                && ac != bc
            {
                ops.push(Op::SetContent {
                    uid: uid.clone(),
                    before: ac.clone(),
                    after: bc.clone(),
                });
            }
            let ac = a.content.iter().find(|n| n.kind.0 == "block_children");
            let bc = b.content.iter().find(|n| n.kind.0 == "block_children");
            match (ac, bc) {
                (Some(ac), Some(bc)) => groups(&ac.content, &bc.content, Some(uid), ops),
                (None, None) => {}
                _ => ops.push(Op::SetChildren {
                    uid,
                    before: ac.cloned(),
                    after: bc.cloned(),
                }),
            }
        }
    }
    let mut ops = Vec::new();
    groups(
        model::children(before, None)
            .map(Vec::as_slice)
            .unwrap_or_default(),
        model::children(after, None)
            .map(Vec::as_slice)
            .unwrap_or_default(),
        None,
        &mut ops,
    );
    if before.metadata != after.metadata {
        ops.push(Op::SetMetadata {
            before: before.metadata.clone(),
            after: after.metadata.clone(),
        });
    }
    ops
}
