use crate::model::{MindmapNode, MindmapTree, NodeId, Side};

/// One entry per pasted subtree root.
#[derive(Clone, Debug)]
pub struct PasteEntry {
    pub new_root_id: NodeId,
    pub parent_id: NodeId,
    pub all_new_ids: Vec<NodeId>,
    /// Snapshot of every pasted node (for redo restore).
    pub saved_nodes: Vec<MindmapNode>,
}

#[derive(Clone, Debug)]
pub enum Action {
    AddNode {
        node_id: NodeId,
        parent_id: NodeId,
    },
    DeleteSubtree {
        subtree: Vec<MindmapNode>,
        parent_id: NodeId,
        child_index: usize,
    },
    EditText {
        node_id: NodeId,
        old_text: String,
        new_text: String,
    },
    ToggleFold {
        node_id: NodeId,
    },
    MoveSibling {
        node_id: NodeId,
        parent_id: NodeId,
        old_index: usize,
        new_index: usize,
    },
    MoveNode {
        node_id: NodeId,
        old_parent: NodeId,
        old_child_index: usize,
        new_parent: NodeId,
        old_position: Option<Side>,
    },
    PasteSubtrees {
        entries: Vec<PasteEntry>,
    },
    Batch(Vec<Action>),
    SetBold {
        node_id: NodeId,
        old_bold: bool,
        new_bold: bool,
    },
    SetLink {
        node_id: NodeId,
        old_link: Option<String>,
        new_link: Option<String>,
    },
}

pub struct History {
    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,
    dirty: bool,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
        }
    }
}

impl History {
    pub fn push(&mut self, action: Action) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self, tree: &mut MindmapTree) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            apply_reverse(tree, &action);
            self.redo_stack.push(action);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, tree: &mut MindmapTree) -> bool {
        if let Some(action) = self.redo_stack.pop() {
            apply_forward(tree, &action);
            self.undo_stack.push(action);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn is_dirty(&self) -> bool { self.dirty }
    pub fn mark_clean(&mut self) { self.dirty = false; }
}

fn apply_reverse(tree: &mut MindmapTree, action: &Action) {
    match action {
        Action::AddNode { node_id, parent_id } => {
            // Undo add: delete the node
            tree.nodes[*parent_id].children.retain(|&c| c != *node_id);
            tree.nodes[*node_id].parent = None;
            tree.nodes[*node_id].text.clear();
        }
        Action::DeleteSubtree {
            subtree,
            parent_id,
            child_index,
        } => {
            // Undo delete: restore the subtree
            if let Some(first) = subtree.first() {
                let root_id = first.id;
                // Restore all nodes
                for saved_node in subtree {
                    if saved_node.id < tree.nodes.len() {
                        tree.nodes[saved_node.id] = saved_node.clone();
                    }
                }
                // Re-attach to parent
                tree.nodes[*parent_id].children.push(root_id);
            }
        }
        Action::EditText {
            node_id,
            old_text,
            new_text: _,
        } => {
            tree.nodes[*node_id].text = old_text.clone();
        }
        Action::ToggleFold { node_id } => {
            tree.nodes[*node_id].folded = !tree.nodes[*node_id].folded;
        }
        Action::MoveSibling {
            parent_id,
            old_index,
            new_index,
            ..
        } => {
            // Undo: swap back from new_index to old_index
            tree.nodes[*parent_id].children.swap(*old_index, *new_index);
        }
        Action::MoveNode {
            node_id,
            old_parent,
            old_child_index,
            new_parent,
            old_position,
        } => {
            // Undo move: detach from new_parent, re-insert into old_parent at old_child_index
            tree.nodes[*new_parent].children.retain(|&c| c != *node_id);
            tree.nodes[*old_parent].children.insert(*old_child_index, *node_id);
            tree.nodes[*node_id].parent = Some(*old_parent);
            tree.nodes[*node_id].position = old_position.clone();
        }
        Action::PasteSubtrees { entries } => {
            // Undo paste: detach roots from parents, clear all pasted node slots
            for entry in entries {
                tree.nodes[entry.parent_id]
                    .children
                    .retain(|&c| c != entry.new_root_id);
                for &id in &entry.all_new_ids {
                    if id < tree.nodes.len() {
                        tree.nodes[id].children.clear();
                        tree.nodes[id].parent = None;
                        tree.nodes[id].text = String::new();
                    }
                }
            }
        }
        Action::Batch(actions) => {
            // Undo in reverse order
            for action in actions.iter().rev() {
                apply_reverse(tree, action);
            }
        }
        Action::SetBold { node_id, old_bold, .. } => {
            tree.nodes[*node_id].bold = *old_bold;
        }
        Action::SetLink { node_id, old_link, .. } => {
            tree.nodes[*node_id].link = old_link.clone();
        }
    }
}

fn apply_forward(tree: &mut MindmapTree, action: &Action) {
    match action {
        Action::AddNode { node_id, parent_id } => {
            // Re-add the node (it's still in the arena, just disconnected)
            tree.nodes[*parent_id].children.push(*node_id);
            tree.nodes[*node_id].parent = Some(*parent_id);
        }
        Action::DeleteSubtree {
            subtree,
            parent_id,
            ..
        } => {
            if let Some(first) = subtree.first() {
                tree.delete_subtree(first.id);
            }
        }
        Action::EditText {
            node_id,
            old_text: _,
            new_text,
        } => {
            tree.nodes[*node_id].text = new_text.clone();
        }
        Action::ToggleFold { node_id } => {
            tree.nodes[*node_id].folded = !tree.nodes[*node_id].folded;
        }
        Action::MoveSibling {
            parent_id,
            old_index,
            new_index,
            ..
        } => {
            // Redo: swap from old_index to new_index
            tree.nodes[*parent_id].children.swap(*old_index, *new_index);
        }
        Action::MoveNode {
            node_id,
            old_parent,
            new_parent,
            ..
        } => {
            // Redo move: detach from old_parent, append to new_parent
            tree.nodes[*old_parent].children.retain(|&c| c != *node_id);
            tree.nodes[*new_parent].children.push(*node_id);
            tree.nodes[*node_id].parent = Some(*new_parent);
            // Assign side if new parent is root, else clear
            if *new_parent == tree.root {
                let right_count = tree.nodes[*new_parent]
                    .children
                    .iter()
                    .filter(|&&c| tree.nodes[c].position == Some(crate::model::Side::Right))
                    .count();
                let left_count = tree.nodes[*new_parent]
                    .children
                    .iter()
                    .filter(|&&c| tree.nodes[c].position == Some(crate::model::Side::Left))
                    .count();
                tree.nodes[*node_id].position = if right_count <= left_count {
                    Some(crate::model::Side::Right)
                } else {
                    Some(crate::model::Side::Left)
                };
            } else {
                tree.nodes[*node_id].position = None;
            }
            tree.nodes[*new_parent].folded = false;
        }
        Action::PasteSubtrees { entries } => {
            // Redo paste: restore saved_nodes into arena, re-attach roots
            for entry in entries {
                // Ensure arena is large enough
                while tree.nodes.len() <= entry.all_new_ids.iter().copied().max().unwrap_or(0) {
                    let placeholder_id = tree.nodes.len();
                    tree.nodes.push(MindmapNode::new(
                        placeholder_id,
                        String::new(),
                        String::new(),
                    ));
                }
                for saved_node in &entry.saved_nodes {
                    if saved_node.id < tree.nodes.len() {
                        tree.nodes[saved_node.id] = saved_node.clone();
                    }
                }
                tree.nodes[entry.parent_id].children.push(entry.new_root_id);
            }
        }
        Action::Batch(actions) => {
            // Redo in forward order
            for action in actions {
                apply_forward(tree, action);
            }
        }
        Action::SetBold { node_id, new_bold, .. } => {
            tree.nodes[*node_id].bold = *new_bold;
        }
        Action::SetLink { node_id, new_link, .. } => {
            tree.nodes[*node_id].link = new_link.clone();
        }
    }
}
