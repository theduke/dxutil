#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    BlockDocument,
    GraphNode,
    MarkdownNotes,
    Composer,
}
impl Profile {
    pub fn has_blocks(self) -> bool {
        self != Self::Composer
    }
    pub fn allows_colors(self) -> bool {
        matches!(self, Self::BlockDocument | Self::GraphNode)
    }
    pub fn allows_marks(self) -> bool {
        self != Self::Composer
    }
    pub fn allows_islands(self) -> bool {
        matches!(self, Self::BlockDocument | Self::GraphNode)
    }
    pub fn allows_mark(self, kind: &str) -> bool {
        self.allows_marks()
            && (self.allows_colors()
                || matches!(
                    kind,
                    "bold" | "italic" | "strike" | "code" | "link" | "entity_link"
                ))
    }
}
#[derive(Clone, Debug)]
pub struct EngineOptions {
    pub read_only: bool,
    pub history_depth: usize,
    pub history_group_ms: u64,
    pub host_entity_picker: bool,
    pub mac: bool,
    pub origin: Option<String>,
}
impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            read_only: false,
            history_depth: 200,
            history_group_ms: 500,
            host_entity_picker: false,
            mac: false,
            origin: None,
        }
    }
}
