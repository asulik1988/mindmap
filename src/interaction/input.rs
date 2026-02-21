use crate::canvas::renderer::NodeRects;
use crate::canvas::viewport::Viewport;
use crate::model::{Clipboard, MindmapTree, NodeId, NodeState, Selection};
use crate::history::{History, PasteEntry};
use crate::interaction::editing::EditingState;
use egui::{Key, Rect, Response, Ui};

/// Drag-and-drop state for reparenting nodes.
pub struct DragState {
    pub node_id: NodeId,
    pub drop_target: Option<NodeId>,
    pub grab_offset: egui::Vec2,  // cursor_pos - node_screen_center at drag start
    pub cursor_pos: egui::Pos2,   // updated each frame
}

/// Result from handle_input so the caller knows what extra work to do.
pub struct InputResult {
    pub needs_relayout: bool,
    /// If set, the viewport should auto-scroll to keep this node visible.
    pub ensure_visible: Option<NodeId>,
}

/// Handle all input: pan, zoom, selection, keyboard shortcuts.
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
    clipboard: &mut Clipboard,
    drag_state: &mut Option<DragState>,
    search_active: bool,
) -> InputResult {
    let mut result = InputResult {
        needs_relayout: false,
        ensure_visible: None,
    };

    // Don't process canvas shortcuts while editing text or search is active
    if editing.is_active() || search_active {
        return result;
    }

    // --- Zoom (Scroll wheel zooms toward cursor, Ctrl+Scroll or plain scroll) ---
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    let raw_scroll = ui.input(|i| {
        i.events.iter().find_map(|e| {
            if let egui::Event::MouseWheel { delta, .. } = e {
                Some(delta.y)
            } else {
                None
            }
        }).unwrap_or(0.0)
    });
    let scroll = if scroll_delta != 0.0 { scroll_delta } else { raw_scroll * 50.0 };
    let modifiers = ui.input(|i| i.modifiers);

    if scroll != 0.0 {
        if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
            if pointer.x >= screen_rect.min.x && pointer.x <= screen_rect.max.x
                && pointer.y >= screen_rect.min.y && pointer.y <= screen_rect.max.y
            {
                viewport.zoom_around(pointer, scroll * 0.002, screen_rect);
            }
        }
    }

    // --- Pan (left-drag on empty canvas, middle-mouse drag, or Space+drag) ---
    let space_held = ui.input(|i| i.key_down(Key::Space));
    let is_dragging_primary = response.dragged_by(egui::PointerButton::Primary);
    let is_dragging_middle = response.dragged_by(egui::PointerButton::Middle);

    if is_dragging_middle || (is_dragging_primary && space_held) {
        // Always pan with middle-mouse or Space+drag
        viewport.offset += response.drag_delta();
    } else if is_dragging_primary {
        // Left-drag: check if drag started on a node
        let drag_started_on_node_id = if response.drag_started_by(egui::PointerButton::Primary) {
            response.interact_pointer_pos()
                .and_then(|p| find_node_at(p, node_rects))
        } else {
            None
        };

        // Start drag-reparent if drag started on a non-root node
        if let Some(node_id) = drag_started_on_node_id {
            if node_id != tree.root && drag_state.is_none() {
                let pointer_pos = response.interact_pointer_pos().unwrap_or(egui::Pos2::ZERO);
                let node_screen_pos = viewport.canvas_to_screen(tree.nodes[node_id].layout_pos, screen_rect);
                *drag_state = Some(DragState {
                    node_id,
                    drop_target: None,
                    grab_offset: pointer_pos - node_screen_pos,
                    cursor_pos: pointer_pos,
                });
                selection.select_single(node_id);
            }
        }

        // While dragging: update drop target and cursor position
        if let Some(ref mut ds) = drag_state {
            if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                ds.cursor_pos = pointer;
                let hover_node = find_node_at(pointer, node_rects);
                ds.drop_target = hover_node.filter(|&target| {
                    target != ds.node_id
                        && target != tree.root  // don't drop onto root directly — use it only if needed
                        && !tree.is_ancestor(ds.node_id, target) // prevent cycles
                });
                // Also allow dropping on root
                if hover_node == Some(tree.root) {
                    ds.drop_target = Some(tree.root);
                }
            } else {
                ds.drop_target = None;
            }
        }

        // Pan if not dragging a node
        if drag_state.is_none() {
            viewport.offset += response.drag_delta();
        }
    }

    // Handle drag release
    if response.drag_stopped_by(egui::PointerButton::Primary) {
        if let Some(ds) = drag_state.take() {
            if let Some(target_id) = ds.drop_target {
                // Don't reparent if target is already the parent
                let current_parent = tree.nodes[ds.node_id].parent;
                if current_parent != Some(target_id) {
                    if let Some((old_parent, old_index, old_position)) =
                        tree.reparent_node(ds.node_id, target_id)
                    {
                        history.push(crate::history::Action::MoveNode {
                            node_id: ds.node_id,
                            old_parent,
                            old_child_index: old_index,
                            new_parent: target_id,
                            old_position,
                        });
                        result.needs_relayout = true;
                        result.ensure_visible = Some(ds.node_id);
                    }
                }
            }
        }
    }

    // --- Double-click to edit (check first so single-click doesn't also fire) ---
    if response.double_clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            if let Some(node_id) = find_node_at(pointer, node_rects) {
                selection.select_single(node_id);
                editing.start(node_id, tree.nodes[node_id].text.clone());
                tree.nodes[node_id].state = NodeState::Editing;
            }
        }
    }

    // --- Click to select + toggle fold/unfold ---
    if response.clicked() && !space_held {
        if let Some(pointer) = response.interact_pointer_pos() {
            let clicked_node = find_node_at(pointer, node_rects);
            if let Some(node_id) = clicked_node {
                if modifiers.ctrl {
                    selection.toggle(node_id);
                } else {
                    selection.select_single(node_id);
                    // Toggle fold if the node has children
                    if !tree.nodes[node_id].children.is_empty() {
                        tree.toggle_fold(node_id);
                        result.needs_relayout = true;
                    }
                }
            } else {
                selection.clear();
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

    // Track if any key event was consumed, to avoid also triggering any-key-starts-editing
    let mut key_consumed = false;

    // Handle egui's Copy/Cut/Paste events (Ctrl+C/X/V are intercepted by egui
    // and converted to these instead of Key events)
    for event in &keys {
        match event {
            egui::Event::Copy => {
                if !selection.selected.is_empty() {
                    let deduped = tree.deduplicate_selection(&selection.selected);
                    clipboard.clear();
                    for &id in &deduped {
                        clipboard.blueprints.push(tree.clone_subtree(id));
                    }
                }
                key_consumed = true;
            }
            egui::Event::Cut => {
                if !selection.selected.is_empty() {
                    let deduped = tree.deduplicate_selection(&selection.selected);
                    let has_root = deduped.iter().any(|&id| id == tree.root);
                    if !has_root && !deduped.is_empty() {
                        clipboard.clear();
                        for &id in &deduped {
                            clipboard.blueprints.push(tree.clone_subtree(id));
                        }
                        let mut batch_actions = Vec::new();
                        let mut select_after: Option<NodeId> = None;
                        for &id in &deduped {
                            let parent_id = tree.nodes[id].parent;
                            let child_index = tree.child_index(id).unwrap_or(0);
                            if select_after.is_none() {
                                select_after = parent_id;
                            }
                            if let Some(subtree) = tree.delete_subtree(id) {
                                batch_actions.push(crate::history::Action::DeleteSubtree {
                                    subtree,
                                    parent_id: parent_id.unwrap_or(tree.root),
                                    child_index,
                                });
                            }
                        }
                        if !batch_actions.is_empty() {
                            history.push(crate::history::Action::Batch(batch_actions));
                        }
                        if let Some(pid) = select_after {
                            selection.select_single(pid);
                            result.ensure_visible = Some(pid);
                        } else {
                            selection.clear();
                        }
                        result.needs_relayout = true;
                    }
                }
                key_consumed = true;
            }
            egui::Event::Paste(_) => {
                if !clipboard.is_empty() {
                    if let Some(parent_id) = selection.primary() {
                        let mut entries = Vec::new();
                        let mut first_root: Option<NodeId> = None;
                        for bp in &clipboard.blueprints {
                            let (new_root, all_ids) = tree.paste_subtree(bp, parent_id);
                            if first_root.is_none() {
                                first_root = Some(new_root);
                            }
                            let saved: Vec<_> = all_ids
                                .iter()
                                .map(|&id| tree.nodes[id].clone())
                                .collect();
                            entries.push(PasteEntry {
                                new_root_id: new_root,
                                parent_id,
                                all_new_ids: all_ids,
                                saved_nodes: saved,
                            });
                        }
                        history.push(crate::history::Action::PasteSubtrees { entries });
                        if let Some(root_id) = first_root {
                            selection.select_single(root_id);
                            result.ensure_visible = Some(root_id);
                        }
                        result.needs_relayout = true;
                    }
                }
                key_consumed = true;
            }
            _ => {}
        }
    }

    for event in &keys {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            match (key, modifiers) {
                // --- Arrow key navigation ---

                // Left: select parent
                (Key::ArrowLeft, m) if !m.ctrl && !m.alt => {
                    if let Some(node_id) = selection.primary() {
                        if let Some(parent_id) = tree.nodes[node_id].parent {
                            selection.select_single(parent_id);
                            result.ensure_visible = Some(parent_id);
                        }
                    }
                    key_consumed = true;
                }

                // Right: select first child (unfold first if folded)
                (Key::ArrowRight, m) if !m.ctrl && !m.alt => {
                    if let Some(node_id) = selection.primary() {
                        if !tree.nodes[node_id].children.is_empty() {
                            if tree.nodes[node_id].folded {
                                // Unfold, stay on current node
                                tree.toggle_fold(node_id);
                                result.needs_relayout = true;
                                result.ensure_visible = Some(node_id);
                            } else if let Some(child_id) = tree.first_visible_child(node_id) {
                                selection.select_single(child_id);
                                result.ensure_visible = Some(child_id);
                            }
                        }
                    }
                    key_consumed = true;
                }

                // Up: previous sibling (or Ctrl+Up: move sibling up)
                (Key::ArrowUp, m) if !m.alt => {
                    if m.ctrl {
                        // Move sibling up
                        if let Some(node_id) = selection.primary() {
                            if let Some((parent_id, old_idx, new_idx)) = tree.move_sibling_up(node_id) {
                                history.push(crate::history::Action::MoveSibling {
                                    node_id,
                                    parent_id,
                                    old_index: old_idx,
                                    new_index: new_idx,
                                });
                                result.needs_relayout = true;
                                result.ensure_visible = Some(node_id);
                            }
                        }
                    } else {
                        // Previous sibling
                        if let Some(node_id) = selection.primary() {
                            if let Some(prev_id) = tree.prev_sibling(node_id) {
                                selection.select_single(prev_id);
                                result.ensure_visible = Some(prev_id);
                            }
                        }
                    }
                    key_consumed = true;
                }

                // Down: next sibling (or Ctrl+Down: move sibling down)
                (Key::ArrowDown, m) if !m.alt => {
                    if m.ctrl {
                        // Move sibling down
                        if let Some(node_id) = selection.primary() {
                            if let Some((parent_id, old_idx, new_idx)) = tree.move_sibling_down(node_id) {
                                history.push(crate::history::Action::MoveSibling {
                                    node_id,
                                    parent_id,
                                    old_index: old_idx,
                                    new_index: new_idx,
                                });
                                result.needs_relayout = true;
                                result.ensure_visible = Some(node_id);
                            }
                        }
                    } else {
                        // Next sibling
                        if let Some(node_id) = selection.primary() {
                            if let Some(next_id) = tree.next_sibling(node_id) {
                                selection.select_single(next_id);
                                result.ensure_visible = Some(next_id);
                            }
                        }
                    }
                    key_consumed = true;
                }

                // Home: select root
                (Key::Home, _) => {
                    selection.select_single(tree.root);
                    result.ensure_visible = Some(tree.root);
                    key_consumed = true;
                }

                // Escape: deselect all
                (Key::Escape, _) => {
                    selection.clear();
                    key_consumed = true;
                }

                // Tab: add child
                (Key::Tab, _) => {
                    if let Some(parent_id) = selection.primary() {
                        let new_id = tree.add_child(parent_id, "");
                        history.push(crate::history::Action::AddNode {
                            node_id: new_id,
                            parent_id,
                        });
                        selection.select_single(new_id);
                        editing.start(new_id, String::new());
                        tree.nodes[new_id].state = NodeState::Editing;
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // Shift+Enter: add sibling ABOVE
                (Key::Enter, m) if m.shift && !m.ctrl => {
                    if let Some(node_id) = selection.primary() {
                        let new_id = tree.add_sibling_before(node_id, "");
                        history.push(crate::history::Action::AddNode {
                            node_id: new_id,
                            parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
                        });
                        selection.select_single(new_id);
                        editing.start(new_id, String::new());
                        tree.nodes[new_id].state = NodeState::Editing;
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // Enter: add sibling below
                (Key::Enter, m) if !m.ctrl && !m.shift => {
                    if let Some(node_id) = selection.primary() {
                        let new_id = tree.add_sibling(node_id, "");
                        history.push(crate::history::Action::AddNode {
                            node_id: new_id,
                            parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
                        });
                        selection.select_single(new_id);
                        editing.start(new_id, String::new());
                        tree.nodes[new_id].state = NodeState::Editing;
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // F2: edit selected node (preserves text)
                (Key::F2, _) => {
                    if let Some(node_id) = selection.primary() {
                        editing.start(node_id, tree.nodes[node_id].text.clone());
                        tree.nodes[node_id].state = NodeState::Editing;
                    }
                    key_consumed = true;
                }

                // Delete/Backspace: delete selected node, select parent
                (Key::Delete, _) | (Key::Backspace, _) => {
                    if let Some(node_id) = selection.primary() {
                        if node_id != tree.root {
                            let parent_id = tree.nodes[node_id].parent;
                            let child_index = tree.child_index(node_id).unwrap_or(0);
                            if let Some(subtree) = tree.delete_subtree(node_id) {
                                history.push(crate::history::Action::DeleteSubtree {
                                    subtree,
                                    parent_id: parent_id.unwrap_or(tree.root),
                                    child_index,
                                });
                                // Select parent after deletion
                                if let Some(pid) = parent_id {
                                    selection.select_single(pid);
                                    result.ensure_visible = Some(pid);
                                } else {
                                    selection.clear();
                                }
                                result.needs_relayout = true;
                            }
                        }
                    }
                    key_consumed = true;
                }

                // Ctrl+C: copy
                (Key::C, m) if m.ctrl && !m.shift => {
                    if !selection.selected.is_empty() {
                        let deduped = tree.deduplicate_selection(&selection.selected);
                        clipboard.clear();
                        for &id in &deduped {
                            clipboard.blueprints.push(tree.clone_subtree(id));
                        }
                    }
                    key_consumed = true;
                }

                // Ctrl+X: cut (copy + delete)
                (Key::X, m) if m.ctrl && !m.shift => {
                    if !selection.selected.is_empty() {
                        let deduped = tree.deduplicate_selection(&selection.selected);
                        // Skip if any selected node is root
                        let has_root = deduped.iter().any(|&id| id == tree.root);
                        if !has_root && !deduped.is_empty() {
                            // Copy first
                            clipboard.clear();
                            for &id in &deduped {
                                clipboard.blueprints.push(tree.clone_subtree(id));
                            }
                            // Delete each, collecting actions for Batch
                            let mut batch_actions = Vec::new();
                            let mut select_after: Option<NodeId> = None;
                            for &id in &deduped {
                                let parent_id = tree.nodes[id].parent;
                                let child_index = tree.child_index(id).unwrap_or(0);
                                if select_after.is_none() {
                                    select_after = parent_id;
                                }
                                if let Some(subtree) = tree.delete_subtree(id) {
                                    batch_actions.push(crate::history::Action::DeleteSubtree {
                                        subtree,
                                        parent_id: parent_id.unwrap_or(tree.root),
                                        child_index,
                                    });
                                }
                            }
                            if !batch_actions.is_empty() {
                                history.push(crate::history::Action::Batch(batch_actions));
                            }
                            if let Some(pid) = select_after {
                                selection.select_single(pid);
                                result.ensure_visible = Some(pid);
                            } else {
                                selection.clear();
                            }
                            result.needs_relayout = true;
                        }
                    }
                    key_consumed = true;
                }

                // Ctrl+V: paste as child of selected node
                (Key::V, m) if m.ctrl && !m.shift => {
                    if !clipboard.is_empty() {
                        if let Some(parent_id) = selection.primary() {
                            let mut entries = Vec::new();
                            let mut first_root: Option<NodeId> = None;
                            for bp in &clipboard.blueprints {
                                let (new_root, all_ids) = tree.paste_subtree(bp, parent_id);
                                if first_root.is_none() {
                                    first_root = Some(new_root);
                                }
                                // Save the pasted nodes for redo
                                let saved: Vec<_> = all_ids
                                    .iter()
                                    .map(|&id| tree.nodes[id].clone())
                                    .collect();
                                entries.push(PasteEntry {
                                    new_root_id: new_root,
                                    parent_id,
                                    all_new_ids: all_ids,
                                    saved_nodes: saved,
                                });
                            }
                            history.push(crate::history::Action::PasteSubtrees { entries });
                            if let Some(root_id) = first_root {
                                selection.select_single(root_id);
                                result.ensure_visible = Some(root_id);
                            }
                            result.needs_relayout = true;
                        }
                    }
                    key_consumed = true;
                }

                // Ctrl+.: toggle fold
                (Key::Period, m) if m.ctrl => {
                    if let Some(node_id) = selection.primary() {
                        tree.toggle_fold(node_id);
                        history.push(crate::history::Action::ToggleFold { node_id });
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // Ctrl+Z: undo
                (Key::Z, m) if m.ctrl && !m.shift => {
                    if history.undo(tree) {
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // Ctrl+Shift+Z: redo
                (Key::Z, m) if m.ctrl && m.shift => {
                    if history.redo(tree) {
                        result.needs_relayout = true;
                    }
                    key_consumed = true;
                }

                // Ctrl+S: save
                (Key::S, m) if m.ctrl => {
                    save_file(tree, file_path);
                    key_consumed = true;
                }

                // Ctrl+0: fit to screen
                (Key::Num0, m) if m.ctrl => {
                    let bounds = compute_bounds(tree);
                    viewport.fit_to_bounds(bounds, screen_rect, 80.0);
                    key_consumed = true;
                }

                _ => {}
            }
        }
    }

    // --- Any-key-starts-editing (Excel model) ---
    // If a node is selected, not editing, and a printable character was typed
    // (without Ctrl/Alt modifiers), clear text and start editing.
    if !key_consumed {
        if let Some(node_id) = selection.primary() {
            for event in &keys {
                if let egui::Event::Text(text) = event {
                    // Skip if Ctrl or Alt held (those are shortcuts)
                    if !modifiers.ctrl && !modifiers.alt && !text.is_empty() {
                        let original_text = tree.nodes[node_id].text.clone();
                        editing.start_replace(node_id, original_text, text.clone());
                        tree.nodes[node_id].state = NodeState::Editing;
                        break;
                    }
                }
            }
        }
    }

    result
}

pub fn find_node_at(pos: egui::Pos2, node_rects: &NodeRects) -> Option<NodeId> {
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

pub fn save_file(tree: &MindmapTree, file_path: &mut Option<std::path::PathBuf>) {
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
