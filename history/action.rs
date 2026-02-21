use crate::model::{MindmapNode, MindmapTree, NodeId};

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
}

pub struct History {
    undo_stack: Vec<Action>,
    redo_stack: Vec<Action>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl History {
    pub fn push(&mut self, action: Action) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, tree: &mut MindmapTree) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            let reverse = apply_reverse(tree, &action);
            self.redo_stack.push(action);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, tree: &mut MindmapTree) -> bool {
        if let Some(action) = self.redo_stack.pop() {
            apply_forward(tree, &action);
            self.undo_stack.push(action);
            true
        } else {
            false
        }
    }
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
            new_text,
        } => {
            tree.nodes[*node_id].text = old_text.clone();
        }
        Action::ToggleFold { node_id } => {
            tree.nodes[*node_id].folded = !tree.nodes[*node_id].folded;
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
            old_text,
            new_text,
        } => {
            tree.nodes[*node_id].text = new_text.clone();
        }
        Action::ToggleFold { node_id } => {
            tree.nodes[*node_id].folded = !tree.nodes[*node_id].folded;
        }
    }
}
