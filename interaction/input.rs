use crate::canvas::renderer::NodeRects;
use crate::canvas::viewport::Viewport;
use crate::model::{MindmapTree, NodeId, NodeState, Selection};
use crate::history::History;
use crate::interaction::editing::EditingState;
use egui::{Key, Modifiers, Rect, Response, Ui};

/// Handle all input: pan, zoom, selection, keyboard shortcuts.
/// Returns true if layout needs to be re-run.
pub fn handle_input(
    ui: &Ui,
    response: &Response,
    viewport: &mut Viewport,
    tree: &mut MindmapTree,
    selection: &mut Selection,
    node_rects: &NodeRects,
    screen_rect: Rect,
    history: &mut History,
    editing: &mut EditingState,
    file_path: &mut Option<std::path::PathBuf>,
) -> bool {
    let mut needs_relayout = false;

    // Don't process canvas shortcuts while editing text
    if editing.is_active() {
        return false;
    }

    // --- Zoom (Ctrl+Scroll) ---
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    let modifiers = ui.input(|i| i.modifiers);
    if scroll != 0.0 && modifiers.ctrl {
        if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
            viewport.zoom_around(pointer, scroll * 0.002, screen_rect);
        }
    }

    // --- Pan (Middle-mouse drag or Space+drag) ---
    let space_held = ui.input(|i| i.key_down(Key::Space));
    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary) && space_held)
    {
        viewport.offset += response.drag_delta();
    }

    // --- Click to select ---
    if response.clicked() && !space_held {
        if let Some(pointer) = response.interact_pointer_pos() {
            let clicked_node = find_node_at(pointer, node_rects);
            if let Some(node_id) = clicked_node {
                if modifiers.ctrl {
                    selection.toggle(node_id);
                } else {
                    selection.select_single(node_id);
                }
            } else {
                selection.clear();
            }
        }
    }

    // --- Double-click to edit ---
    if response.double_clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            if let Some(node_id) = find_node_at(pointer, node_rects) {
                selection.select_single(node_id);
                editing.start(node_id, tree.nodes[node_id].text.clone());
                tree.nodes[node_id].state = NodeState::Editing;
            }
        }
    }

    // --- Hover ---
    selection.hovered = None;
    if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
        selection.hovered = find_node_at(pointer, node_rects);
    }

    // Update node hover states
    for node in &mut tree.nodes {
        if node.state != NodeState::Editing {
            if selection.hovered == Some(node.id) {
                node.state = NodeState::Hovered;
            } else {
                node.state = NodeState::Default;
            }
        }
    }

    // --- Keyboard shortcuts ---
    let keys = ui.input(|i| i.events.clone());
    for event in &keys {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            match (key, modifiers) {
                // Tab: add child
                (Key::Tab, _) => {
                    if let Some(parent_id) = selection.primary() {
                        let old_text = String::new();
                        let new_id = tree.add_child(parent_id, "New node");
                        history.push(crate::history::Action::AddNode {
                            node_id: new_id,
                            parent_id,
                        });
                        selection.select_single(new_id);
                        editing.start(new_id, "New node".to_string());
                        tree.nodes[new_id].state = NodeState::Editing;
                        needs_relayout = true;
                    }
                }
                // Enter: add sibling
                (Key::Enter, m) if !m.ctrl && !m.shift => {
                    if let Some(node_id) = selection.primary() {
                        let new_id = tree.add_sibling(node_id, "New node");
                        history.push(crate::history::Action::AddNode {
                            node_id: new_id,
                            parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
                        });
                        selection.select_single(new_id);
                        editing.start(new_id, "New node".to_string());
                        tree.nodes[new_id].state = NodeState::Editing;
                        needs_relayout = true;
                    }
                }
                // F2: edit selected node
                (Key::F2, _) => {
                    if let Some(node_id) = selection.primary() {
                        editing.start(node_id, tree.nodes[node_id].text.clone());
                        tree.nodes[node_id].state = NodeState::Editing;
                    }
                }
                // Delete: delete selected node
                (Key::Delete, _) | (Key::Backspace, _) => {
                    if let Some(node_id) = selection.primary() {
                        if node_id != tree.root {
                            let parent_id = tree.nodes[node_id].parent;
                            if let Some(subtree) = tree.delete_subtree(node_id) {
                                history.push(crate::history::Action::DeleteSubtree {
                                    subtree,
                                    parent_id: parent_id.unwrap_or(tree.root),
                                    child_index: 0,
                                });
                                selection.clear();
                                needs_relayout = true;
                            }
                        }
                    }
                }
                // Ctrl+.: toggle fold
                (Key::Period, m) if m.ctrl => {
                    if let Some(node_id) = selection.primary() {
                        tree.toggle_fold(node_id);
                        history.push(crate::history::Action::ToggleFold { node_id });
                        needs_relayout = true;
                    }
                }
                // Ctrl+Z: undo
                (Key::Z, m) if m.ctrl && !m.shift => {
                    if history.undo(tree) {
                        needs_relayout = true;
                    }
                }
                // Ctrl+Shift+Z: redo
                (Key::Z, m) if m.ctrl && m.shift => {
                    if history.redo(tree) {
                        needs_relayout = true;
                    }
                }
                // Ctrl+S: save
                (Key::S, m) if m.ctrl => {
                    save_file(tree, file_path);
                }
                // Ctrl+0: fit to screen
                (Key::Num0, m) if m.ctrl => {
                    let bounds = compute_bounds(tree);
                    viewport.fit_to_bounds(bounds, screen_rect, 80.0);
                }
                _ => {}
            }
        }
    }

    needs_relayout
}

fn find_node_at(pos: egui::Pos2, node_rects: &NodeRects) -> Option<NodeId> {
    // Return the topmost (last drawn) node that contains the point.
    // Iterate in reverse since later-drawn nodes are on top.
    node_rects
        .iter()
        .filter(|(_, rect)| rect.contains(pos))
        .max_by_key(|(id, _)| *id) // pick highest id (deepest node drawn last)
        .map(|(&id, _)| id)
}

fn compute_bounds(tree: &MindmapTree) -> Rect {
    let visible = tree.visible_nodes();
    if visible.is_empty() {
        return Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &id in &visible {
        let pos = tree.nodes[id].layout_pos;
        min_x = min_x.min(pos.x - 150.0);
        max_x = max_x.max(pos.x + 150.0);
        min_y = min_y.min(pos.y - 20.0);
        max_y = max_y.max(pos.y + 20.0);
    }
    Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

fn save_file(tree: &MindmapTree, file_path: &mut Option<std::path::PathBuf>) {
    let path = if let Some(ref p) = file_path {
        Some(p.clone())
    } else {
        rfd::FileDialog::new()
            .add_filter("FreeMind", &["mm"])
            .save_file()
    };

    if let Some(path) = path {
        match crate::io::freemind_write::save_mm_file(tree, &path) {
            Ok(_) => {
                *file_path = Some(path);
                log::info!("File saved successfully");
            }
            Err(e) => {
                log::error!("Failed to save: {}", e);
            }
        }
    }
}
