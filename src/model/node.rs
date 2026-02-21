use egui::{Color32, Pos2, Vec2};

pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeState {
    Default,
    Hovered,
    Editing,
}

#[derive(Clone, Debug)]
pub struct MindmapNode {
    pub id: NodeId,
    pub freemind_id: String,
    pub text: String,
    pub color: Option<Color32>,
    pub background_color: Option<Color32>,
    pub position: Option<Side>,
    pub folded: bool,
    pub created: Option<u64>,
    pub modified: Option<u64>,
    pub bold: bool,
    pub font_size: Option<f32>,
    pub font_name: Option<String>,
    pub notes: String,
    pub link: Option<String>,

    // Tree structure (arena indices)
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,

    // Layout results
    pub layout_pos: Pos2,
    pub layout_size: Vec2,
    /// Text with newlines inserted at wrap points determined during measurement.
    /// Used for rendering so line breaks are stable across zoom levels.
    pub display_text: String,

    // Runtime state (not persisted)
    pub state: NodeState,
}

impl MindmapNode {
    pub fn new(id: NodeId, freemind_id: String, text: String) -> Self {
        Self {
            id,
            freemind_id,
            text,
            color: None,
            background_color: None,
            position: None,
            folded: false,
            created: None,
            modified: None,
            bold: false,
            font_size: None,
            font_name: None,
            notes: String::new(),
            link: None,
            parent: None,
            children: Vec::new(),
            layout_pos: Pos2::ZERO,
            layout_size: Vec2::new(80.0, 36.0),
            display_text: String::new(),
            state: NodeState::Default,
        }
    }

    pub fn depth(&self, nodes: &[MindmapNode]) -> usize {
        let mut depth = 0;
        let mut current = self.parent;
        while let Some(parent_id) = current {
            depth += 1;
            current = nodes[parent_id].parent;
        }
        depth
    }
}
