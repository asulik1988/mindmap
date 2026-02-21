use crate::canvas::node_renderer;
use crate::canvas::renderer::{self, NodeRects};
use crate::canvas::viewport::Viewport;
use crate::export;
use crate::history::{History, PasteEntry};
use crate::interaction::editing::{EditingState, EditResult};
use crate::interaction::input::{self, DragState};
use crate::interaction::search::SearchState;
use crate::layout::reingold_tilford;
use crate::model::{Clipboard, MindmapNode, MindmapTree, NodeId, NodeState, Selection};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};
use std::collections::HashSet;
use std::path::PathBuf;

struct ContextMenuState {
    pos: egui::Pos2,
    target_node: Option<NodeId>,
    color_picker_open: bool,
    color_picker_depth: Option<usize>,
    preview_color: Option<(usize, usize)>, // (depth % 8, palette_index)
}

pub struct MindmapApp {
    tree: Option<MindmapTree>,
    viewport: Viewport,
    selection: Selection,
    history: History,
    editing: EditingState,
    node_rects: NodeRects,
    file_path: Option<PathBuf>,
    needs_initial_fit: bool,
    menu_open: bool,
    clipboard: Clipboard,
    context_menu: Option<ContextMenuState>,
    drag_state: Option<DragState>,
    style_panel_open: bool,
    style_selected_depth: Option<usize>,
    depth_color_config: DepthColorConfig,
    search: SearchState,
    help_open: bool,
    help_suppress_close: bool,
    recent_files: Vec<PathBuf>,
    notes_panel_open: bool,
    notes_focused: bool,
    notes_suppress_close: bool, // skip click-outside for one frame when panel just opened
    notes_edit_node: Option<NodeId>,
    notes_saved_at: Option<f64>, // egui time when last autosave happened
    dark_mode: bool,
    minimap_dragging: bool,
    link_edit: Option<(NodeId, String)>,
    link_edit_suppress_close: bool,
}

impl MindmapApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file_arg: Option<PathBuf>) -> Self {
        // Register Excalidraw's hand-drawn font (Virgil)
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Virgil".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(
                include_bytes!("assets/Virgil-Regular.ttf"),
            )),
        );
        // Set Virgil as the primary proportional font
        fonts.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Virgil".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let mut app = Self {
            tree: None,
            viewport: Viewport::default(),
            selection: Selection::default(),
            history: History::default(),
            editing: EditingState::default(),
            node_rects: NodeRects::default(),
            file_path: None,
            needs_initial_fit: false,
            menu_open: false,
            clipboard: Clipboard::default(),
            context_menu: None,
            drag_state: None,
            style_panel_open: false,
            style_selected_depth: None,
            depth_color_config: DepthColorConfig::default(),
            search: SearchState::default(),
            help_open: false,
            help_suppress_close: false,
            recent_files: load_recent_files(),
            notes_panel_open: false,
            notes_focused: false,
            notes_suppress_close: false,
            notes_edit_node: None,
            notes_saved_at: None,
            dark_mode: cc.storage
                .and_then(|s| s.get_string("dark_mode"))
                .map(|v| v == "true")
                .unwrap_or(false),
            minimap_dragging: false,
            link_edit: None,
            link_edit_suppress_close: false,
        };

        // Load file from command-line argument if provided
        if let Some(path) = file_arg {
            app.load_file(path);
        }

        app
    }

    fn load_file(&mut self, path: PathBuf) {
        match crate::io::freemind_read::load_mm_file(&path) {
            Ok(mut tree) => {
                reingold_tilford::layout(&mut tree);
                self.tree = Some(tree);
                self.add_recent_file(&path);
                self.file_path = Some(path);
                self.selection = Selection::default();
                self.history = History::default();
                self.editing = EditingState::default();
                self.needs_initial_fit = true;
                self.menu_open = false;
                log::info!("File loaded successfully");
            }
            Err(e) => {
                log::error!("Failed to load file: {}", e);
                eprintln!("Failed to load file: {}", e);
            }
        }
    }

    fn add_recent_file(&mut self, path: &PathBuf) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(8);
        save_recent_files(&self.recent_files);
    }

    fn new_map(&mut self) {
        let root = MindmapNode::new(0, "ID_1".to_string(), "Central Topic".to_string());
        self.tree = Some(MindmapTree::new(vec![root], 0));
        self.file_path = None;
        self.selection = Selection::default();
        self.history = History::default();
        self.editing = EditingState::default();
        self.needs_initial_fit = true;
        self.menu_open = false;
    }

    fn close_to_welcome(&mut self) {
        self.tree = None;
        self.file_path = None;
        self.selection = Selection::default();
        self.history = History::default();
        self.editing = EditingState::default();
        self.menu_open = false;
    }
}

impl eframe::App for MindmapApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark_mode", self.dark_mode.to_string());
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Global keyboard shortcuts (work regardless of menu state) ---
        let global_action = ctx.input(|i| {
            for event in &i.events {
                // Detect '?' for help overlay via Text event (keyboard-layout agnostic)
                if let egui::Event::Text(text) = event {
                    if text == "?" && !self.editing.is_active() && !self.search.is_active() {
                        return MenuAction::ToggleHelp;
                    }
                }
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    match (key, modifiers) {
                        (egui::Key::F1, _) if !self.editing.is_active() => {
                            return MenuAction::ToggleHelp;
                        }
                        (egui::Key::Num0, m) if m.ctrl => {
                            return MenuAction::ResetZoom;
                        }
                        (egui::Key::N, m) if m.ctrl && m.shift => {
                            return MenuAction::ToggleNotes;
                        }
                        (egui::Key::N, m) if m.ctrl && !m.shift => {
                            return MenuAction::NewMap;
                        }
                        (egui::Key::O, m) if m.ctrl => {
                            return MenuAction::OpenFile;
                        }
                        (egui::Key::S, m) if m.ctrl && m.shift => {
                            return MenuAction::SaveAs;
                        }
                        (egui::Key::S, m) if m.ctrl && !m.shift => {
                            return MenuAction::Save;
                        }
                        (egui::Key::Q, m) if m.ctrl => {
                            return MenuAction::Quit;
                        }
                        (egui::Key::F, m) if m.ctrl && !m.shift => {
                            return MenuAction::OpenSearch;
                        }
                        (egui::Key::Minus, m) if m.ctrl && m.shift && !self.editing.is_active() => {
                            return MenuAction::FoldAll;
                        }
                        (egui::Key::Equals, m) if m.ctrl && m.shift && !self.editing.is_active() => {
                            return MenuAction::UnfoldAll;
                        }
                        (egui::Key::F, m) if !m.ctrl && !m.shift && !m.alt
                            && !self.editing.is_active() =>
                        {
                            return MenuAction::FitToScreen;
                        }
                        (egui::Key::Home, m) if !m.ctrl && !m.shift && !m.alt => {
                            return MenuAction::FitToScreen;
                        }
                        (egui::Key::B, m) if m.ctrl => {
                            return MenuAction::ToggleBold;
                        }
                        (egui::Key::Escape, _) if self.search.is_active() => {
                            return MenuAction::CloseSearch;
                        }
                        (egui::Key::Escape, _) if self.help_open
                            || self.menu_open
                            || self.style_panel_open
                            || self.notes_panel_open =>
                        {
                            if self.help_open {
                                return MenuAction::ToggleHelp;
                            }
                            return MenuAction::CloseMenu;
                        }
                        _ => {}
                    }
                }
            }
            MenuAction::None
        });

        // Handle global action immediately
        match global_action {
            MenuAction::NewMap => { self.new_map(); }
            MenuAction::OpenFile => {
                self.menu_open = false;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("FreeMind", &["mm"])
                    .pick_file()
                {
                    self.load_file(path);
                }
            }
            MenuAction::SaveAs => {
                self.menu_open = false;
                if let Some(ref tree) = self.tree {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("FreeMind", &["mm"])
                        .save_file()
                    {
                        match crate::io::freemind_write::save_mm_file(tree, &path) {
                            Ok(_) => {
                                self.file_path = Some(path);
                                self.history.mark_clean();
                                log::info!("File saved successfully");
                            }
                            Err(e) => log::error!("Failed to save: {}", e),
                        }
                    }
                }
            }
            MenuAction::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            MenuAction::OpenSearch => {
                if self.search.is_active() {
                    self.search.select_all_pending = true;
                } else {
                    self.search.open();
                    self.menu_open = false;
                    self.style_panel_open = false;
                    self.style_selected_depth = None;
                    self.context_menu = None;
                    self.search.replace_active = false;
                }
            }
            MenuAction::CloseSearch => {
                self.search.close();
            }
            MenuAction::CloseMenu => {
                self.menu_open = false;
                self.style_panel_open = false;
                self.style_selected_depth = None;
                self.notes_panel_open = false;
                self.notes_edit_node = None;
            }
            MenuAction::Save => {
                self.menu_open = false;
                if let Some(ref tree) = self.tree {
                    input::save_file(tree, &mut self.file_path);
                    self.history.mark_clean();
                }
            }
            MenuAction::FitToScreen => {
                self.needs_initial_fit = true;
            }
            MenuAction::ToggleBold => {
                if let Some(node_id) = self.selection.primary() {
                    if let Some(ref mut tree) = self.tree {
                        let old_bold = tree.nodes[node_id].bold;
                        let new_bold = !old_bold;
                        tree.nodes[node_id].bold = new_bold;
                        self.history.push(crate::history::Action::SetBold { node_id, old_bold, new_bold });
                    }
                }
            }
            MenuAction::ToggleNotes => {
                self.notes_panel_open = !self.notes_panel_open;
                if self.notes_panel_open {
                    self.notes_suppress_close = true;
                    self.menu_open = false;
                    self.style_panel_open = false;
                    self.style_selected_depth = None;
                    self.context_menu = None;
                    self.search.close();
                } else {
                    self.notes_edit_node = None;
                }
            }
            MenuAction::FoldAll => {
                if let Some(ref mut tree) = self.tree {
                    for id in 0..tree.nodes.len() {
                        if !tree.nodes[id].children.is_empty() && id != tree.root {
                            tree.nodes[id].folded = true;
                        }
                    }
                    self.needs_initial_fit = true;
                }
            }
            MenuAction::UnfoldAll => {
                if let Some(ref mut tree) = self.tree {
                    for id in 0..tree.nodes.len() {
                        tree.nodes[id].folded = false;
                    }
                    self.needs_initial_fit = true;
                }
            }
            MenuAction::ToggleHelp => {
                self.help_open = !self.help_open;
                if self.help_open {
                    self.help_suppress_close = true;
                }
            }
            _ => {}
        }

        // Handle zoom reset shortcut
        if global_action == MenuAction::ResetZoom {
            self.viewport.zoom = 1.0;
            self.viewport.offset = egui::Vec2::ZERO;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(colors::canvas_bg(self.dark_mode)))
            .show(ctx, |ui| {
                let screen_rect = ui.max_rect();

                // Fit to bounds on first frame after loading
                if self.needs_initial_fit {
                    if let Some(ref mut tree) = self.tree {
                        node_renderer::measure_all_nodes(tree, ui.painter());
                        reingold_tilford::layout(tree);
                        let bounds = compute_tree_bounds(tree);
                        self.viewport.fit_to_bounds(bounds, screen_rect, 80.0);
                    }
                    self.needs_initial_fit = false;
                }

                // Allocate the full panel as an interactive area
                let response = ui.allocate_rect(screen_rect, egui::Sense::click_and_drag());

                if let Some(ref mut tree) = &mut self.tree {
                    let painter = ui.painter();

                    // Apply live preview color from context menu (if any)
                    let preview = self.context_menu.as_ref().and_then(|cm| cm.preview_color);
                    self.depth_color_config.set_preview(preview);

                    // Build search match set for renderer
                    let search_match_set: HashSet<NodeId> = self.search.matches.iter().copied().collect();
                    let search_current = self.search.current_match();

                    // Render canvas
                    self.node_rects = renderer::draw_canvas(
                        painter,
                        tree,
                        &self.viewport,
                        screen_rect,
                        &self.selection,
                        &self.drag_state,
                        &self.depth_color_config,
                        &search_match_set,
                        search_current,
                        self.dark_mode,
                    );

                    // --- Right-click to open context menu ---
                    let secondary_clicked = ui.input(|i| i.pointer.secondary_clicked());
                    if secondary_clicked {
                        self.drag_state = None; // cancel any drag
                        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                            let clicked_node = input::find_node_at(pointer, &self.node_rects);
                            // If right-clicked a node that isn't selected, select it
                            if let Some(node_id) = clicked_node {
                                if !self.selection.is_selected(node_id) {
                                    self.selection.select_single(node_id);
                                }
                            }
                            // Clamp position to keep menu on screen
                            let pos = egui::pos2(pointer.x.round(), pointer.y.round());
                            self.context_menu = Some(ContextMenuState {
                                pos,
                                target_node: clicked_node,
                                color_picker_open: false,
                                color_picker_depth: None,
                                preview_color: None,
                            });
                            self.menu_open = false; // close hamburger
                        }
                    }

                    // --- Dismiss context menu / style panel on scroll/zoom ---
                    if self.context_menu.is_some() || self.style_panel_open {
                        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll_delta != 0.0 {
                            self.context_menu = None;
                            self.style_panel_open = false;
                            self.style_selected_depth = None;
                        }
                    }

                    // --- Dismiss context menu on Escape ---
                    if self.context_menu.is_some() {
                        let escape_pressed = ui.input(|i| {
                            i.events.iter().any(|e| matches!(e, egui::Event::Key {
                                key: egui::Key::Escape, pressed: true, ..
                            }))
                        });
                        if escape_pressed {
                            self.context_menu = None;
                        }
                    }

                    let any_menu_open = self.menu_open
                        || self.context_menu.is_some()
                        || self.style_panel_open
                        || self.notes_panel_open;

                    // Handle canvas input (skip when any menu is open)
                    if !any_menu_open {
                        let input_result = input::handle_input(
                            ui,
                            &response,
                            &mut self.viewport,
                            tree,
                            &mut self.selection,
                            &self.node_rects,
                            screen_rect,
                            &mut self.history,
                            &mut self.editing,
                            &mut self.file_path,
                            &mut self.clipboard,
                            &mut self.drag_state,
                            self.search.is_active()
                                || (self.notes_panel_open && self.notes_focused),
                        );

                        let mut needs_relayout = input_result.needs_relayout;
                        let mut ensure_visible = input_result.ensure_visible;

                        // Draw text editor overlay
                        let edit_result = self.editing.draw(
                            ui,
                            tree,
                            &self.viewport,
                            screen_rect,
                            &mut self.history,
                        );

                        match edit_result {
                            EditResult::None => {}
                            EditResult::Finished => {
                                needs_relayout = true;
                            }
                            EditResult::CreateSibling(node_id) => {
                                // Create sibling below and enter edit mode
                                let new_id = tree.add_sibling(node_id, "");
                                self.history.push(crate::history::Action::AddNode {
                                    node_id: new_id,
                                    parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
                                });
                                self.selection.select_single(new_id);
                                self.editing.start(new_id, String::new());
                                tree.nodes[new_id].state = NodeState::Editing;
                                needs_relayout = true;
                                ensure_visible = Some(new_id);
                            }
                            EditResult::CreateChild(node_id) => {
                                // Create child and enter edit mode
                                let new_id = tree.add_child(node_id, "");
                                self.history.push(crate::history::Action::AddNode {
                                    node_id: new_id,
                                    parent_id: node_id,
                                });
                                self.selection.select_single(new_id);
                                self.editing.start(new_id, String::new());
                                tree.nodes[new_id].state = NodeState::Editing;
                                needs_relayout = true;
                                ensure_visible = Some(new_id);
                            }
                            EditResult::DeleteEmpty(node_id) => {
                                // Delete the empty node
                                if node_id != tree.root {
                                    let parent_id = tree.nodes[node_id].parent;
                                    let child_index = tree.child_index(node_id).unwrap_or(0);
                                    if let Some(subtree) = tree.delete_subtree(node_id) {
                                        self.history.push(crate::history::Action::DeleteSubtree {
                                            subtree,
                                            parent_id: parent_id.unwrap_or(tree.root),
                                            child_index,
                                        });
                                        if let Some(pid) = parent_id {
                                            self.selection.select_single(pid);
                                            ensure_visible = Some(pid);
                                        } else {
                                            self.selection.clear();
                                        }
                                        needs_relayout = true;
                                    }
                                }
                            }
                        }

                        // Re-run layout if needed
                        if needs_relayout {
                            node_renderer::measure_all_nodes(tree, ui.painter());
                            reingold_tilford::layout(tree);
                        }

                        // Auto-scroll to keep selected node visible
                        if let Some(vis_id) = ensure_visible {
                            ensure_node_visible(
                                vis_id,
                                &mut self.viewport,
                                screen_rect,
                                tree,
                            );
                        }

                        // Cursor icons for drag-and-drop
                        if self.drag_state.is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        } else if let Some(hovered) = self.selection.hovered {
                            if hovered != tree.root {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            }
                        }
                    }

                    // --- Context menu ---
                    let mut ctx_action = ContextAction::None;
                    if let Some(ref mut cm) = self.context_menu {
                        ctx_action = draw_context_menu(
                            ui,
                            cm,
                            &self.selection,
                            &self.clipboard,
                            tree,
                            screen_rect,
                            &self.depth_color_config,
                            self.dark_mode,
                        );

                        // Click outside context menu → close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let panel_rect = context_menu_rect(cm.pos, cm.target_node.is_some(), &self.clipboard, tree, &self.selection, screen_rect, cm.color_picker_open);
                            let pointer_pos = ui.input(|i| i.pointer.hover_pos());
                            let clicked_in = pointer_pos.map_or(false, |p| panel_rect.contains(p));
                            if !clicked_in {
                                self.context_menu = None;
                            }
                        }
                    }

                    // Handle context menu action
                    if ctx_action != ContextAction::None {
                        // OpenColorPicker stays in the menu; everything else closes it
                        let keep_menu = matches!(ctx_action, ContextAction::OpenColorPicker);
                        if !keep_menu {
                            self.context_menu = None;
                        }
                        let mut needs_relayout = false;
                        let mut ensure_visible: Option<NodeId> = None;

                        match ctx_action {
                            ContextAction::OpenColorPicker => {
                                // Handled inline by draw_context_menu (toggles color_picker_open)
                            }
                            ContextAction::SetLevelColor(depth, idx) => {
                                self.depth_color_config.set_fill_index(depth, idx);
                            }
                            ContextAction::AddChild => {
                                if let Some(parent_id) = self.selection.primary() {
                                    let new_id = tree.add_child(parent_id, "");
                                    self.history.push(crate::history::Action::AddNode {
                                        node_id: new_id,
                                        parent_id,
                                    });
                                    self.selection.select_single(new_id);
                                    self.editing.start(new_id, String::new());
                                    tree.nodes[new_id].state = NodeState::Editing;
                                    needs_relayout = true;
                                    ensure_visible = Some(new_id);
                                }
                            }
                            ContextAction::AddSibling => {
                                if let Some(node_id) = self.selection.primary() {
                                    let new_id = tree.add_sibling(node_id, "");
                                    self.history.push(crate::history::Action::AddNode {
                                        node_id: new_id,
                                        parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
                                    });
                                    self.selection.select_single(new_id);
                                    self.editing.start(new_id, String::new());
                                    tree.nodes[new_id].state = NodeState::Editing;
                                    needs_relayout = true;
                                    ensure_visible = Some(new_id);
                                }
                            }
                            ContextAction::Edit => {
                                if let Some(node_id) = self.selection.primary() {
                                    self.editing.start(node_id, tree.nodes[node_id].text.clone());
                                    tree.nodes[node_id].state = NodeState::Editing;
                                }
                            }
                            ContextAction::ViewNotes => {
                                self.notes_panel_open = true;
                                self.notes_suppress_close = true;
                                self.notes_edit_node = self.selection.primary();
                                self.style_panel_open = false;
                                self.style_selected_depth = None;
                                self.search.close();
                            }
                            ContextAction::Copy => {
                                if !self.selection.selected.is_empty() {
                                    let deduped = tree.deduplicate_selection(&self.selection.selected);
                                    self.clipboard.clear();
                                    for &id in &deduped {
                                        self.clipboard.blueprints.push(tree.clone_subtree(id));
                                    }
                                }
                            }
                            ContextAction::Cut => {
                                if !self.selection.selected.is_empty() {
                                    let deduped = tree.deduplicate_selection(&self.selection.selected);
                                    let has_root = deduped.iter().any(|&id| id == tree.root);
                                    if !has_root && !deduped.is_empty() {
                                        self.clipboard.clear();
                                        for &id in &deduped {
                                            self.clipboard.blueprints.push(tree.clone_subtree(id));
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
                                            self.history.push(crate::history::Action::Batch(batch_actions));
                                        }
                                        if let Some(pid) = select_after {
                                            self.selection.select_single(pid);
                                            ensure_visible = Some(pid);
                                        } else {
                                            self.selection.clear();
                                        }
                                        needs_relayout = true;
                                    }
                                }
                            }
                            ContextAction::Paste => {
                                if !self.clipboard.is_empty() {
                                    if let Some(parent_id) = self.selection.primary() {
                                        let mut entries = Vec::new();
                                        let mut first_root: Option<NodeId> = None;
                                        for bp in &self.clipboard.blueprints {
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
                                        self.history.push(crate::history::Action::PasteSubtrees { entries });
                                        if let Some(root_id) = first_root {
                                            self.selection.select_single(root_id);
                                            ensure_visible = Some(root_id);
                                        }
                                        needs_relayout = true;
                                    }
                                }
                            }
                            ContextAction::Delete => {
                                if let Some(node_id) = self.selection.primary() {
                                    if node_id != tree.root {
                                        let parent_id = tree.nodes[node_id].parent;
                                        let child_index = tree.child_index(node_id).unwrap_or(0);
                                        if let Some(subtree) = tree.delete_subtree(node_id) {
                                            self.history.push(crate::history::Action::DeleteSubtree {
                                                subtree,
                                                parent_id: parent_id.unwrap_or(tree.root),
                                                child_index,
                                            });
                                            if let Some(pid) = parent_id {
                                                self.selection.select_single(pid);
                                                ensure_visible = Some(pid);
                                            } else {
                                                self.selection.clear();
                                            }
                                            needs_relayout = true;
                                        }
                                    }
                                }
                            }
                            ContextAction::ToggleFold => {
                                if let Some(node_id) = self.selection.primary() {
                                    tree.toggle_fold(node_id);
                                    self.history.push(crate::history::Action::ToggleFold { node_id });
                                    needs_relayout = true;
                                }
                            }
                            ContextAction::FoldAll => {
                                for id in 0..tree.nodes.len() {
                                    if !tree.nodes[id].children.is_empty() && id != tree.root {
                                        tree.nodes[id].folded = true;
                                    }
                                }
                                needs_relayout = true;
                                self.needs_initial_fit = true;
                            }
                            ContextAction::UnfoldAll => {
                                for id in 0..tree.nodes.len() {
                                    tree.nodes[id].folded = false;
                                }
                                needs_relayout = true;
                                self.needs_initial_fit = true;
                            }
                            ContextAction::OpenLink => {
                                if let Some(node_id) = self.selection.primary() {
                                    if let Some(ref url) = tree.nodes[node_id].link.clone() {
                                        let _ = std::process::Command::new("cmd")
                                            .args(["/c", "start", "", url.as_str()])
                                            .spawn();
                                    }
                                }
                            }
                            ContextAction::EditLink => {
                                if let Some(node_id) = self.selection.primary() {
                                    let current = tree.nodes[node_id].link.clone().unwrap_or_default();
                                    self.link_edit = Some((node_id, current));
                                    self.link_edit_suppress_close = true;
                                }
                            }
                            ContextAction::RemoveLink => {
                                if let Some(node_id) = self.selection.primary() {
                                    let old_link = tree.nodes[node_id].link.clone();
                                    tree.nodes[node_id].link = None;
                                    self.history.push(crate::history::Action::SetLink {
                                        node_id,
                                        old_link,
                                        new_link: None,
                                    });
                                }
                            }
                            ContextAction::None => {}
                        }

                        if needs_relayout {
                            node_renderer::measure_all_nodes(tree, ui.painter());
                            reingold_tilford::layout(tree);
                        }
                        if let Some(vis_id) = ensure_visible {
                            ensure_node_visible(vis_id, &mut self.viewport, screen_rect, tree);
                        }
                    }

                    // Draw minimap
                    let minimap_rect = draw_minimap(
                        ui.painter(),
                        tree,
                        &self.viewport,
                        screen_rect,
                        &self.depth_color_config,
                        self.dark_mode,
                    );

                    // Handle minimap click/drag
                    {
                        let ptr = ui.input(|i| i.pointer.hover_pos());
                        let primary_down = ui.input(|i| i.pointer.primary_down());
                        let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
                        let in_minimap = ptr.map_or(false, |p| minimap_rect.contains(p));

                        if in_minimap && primary_down {
                            self.minimap_dragging = true;
                        }
                        if !primary_down {
                            self.minimap_dragging = false;
                        }

                        if (in_minimap && primary_clicked) || self.minimap_dragging {
                            if let Some(p) = ptr {
                                if minimap_rect.contains(p) {
                                    // Compute canvas bounds for all nodes
                                    let all_bounds = compute_all_nodes_bounds(tree);
                                    if all_bounds.width() > 0.0 && all_bounds.height() > 0.0 {
                                        let scale = (minimap_rect.width() / all_bounds.width())
                                            .min(minimap_rect.height() / all_bounds.height());
                                        let scaled_w = all_bounds.width() * scale;
                                        let scaled_h = all_bounds.height() * scale;
                                        let offset_x = (minimap_rect.width() - scaled_w) / 2.0;
                                        let offset_y = (minimap_rect.height() - scaled_h) / 2.0;

                                        // mm coords → canvas coords
                                        let rel = p - minimap_rect.min;
                                        let canvas_x = all_bounds.min.x + (rel.x - offset_x) / scale;
                                        let canvas_y = all_bounds.min.y + (rel.y - offset_y) / scale;

                                        // Center viewport on clicked canvas point
                                        self.viewport.offset = egui::vec2(
                                            -canvas_x * self.viewport.zoom,
                                            -canvas_y * self.viewport.zoom,
                                        );
                                    }
                                }
                            }
                        }

                        if in_minimap {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }

                    // Draw status bar
                    draw_status_bar(ui, screen_rect, tree, &self.file_path, &self.viewport, self.history.is_dirty());

                    // --- Search bar ---
                    if self.search.is_active() {
                        self.search.update_matches(tree);

                        let search_result = draw_search_bar(ui, &mut self.search, screen_rect, self.dark_mode);
                        match search_result {
                            SearchBarAction::None => {}
                            SearchBarAction::Close => {
                                self.search.close();
                            }
                            SearchBarAction::Next => {
                                self.search.next();
                                if let Some(nid) = self.search.current_match() {
                                    if tree.unfold_path_to(nid) {
                                        node_renderer::measure_all_nodes(tree, ui.painter());
                                        reingold_tilford::layout(tree);
                                    }
                                    ensure_node_visible(nid, &mut self.viewport, screen_rect, tree);
                                }
                            }
                            SearchBarAction::Prev => {
                                self.search.prev();
                                if let Some(nid) = self.search.current_match() {
                                    if tree.unfold_path_to(nid) {
                                        node_renderer::measure_all_nodes(tree, ui.painter());
                                        reingold_tilford::layout(tree);
                                    }
                                    ensure_node_visible(nid, &mut self.viewport, screen_rect, tree);
                                }
                            }
                            SearchBarAction::ZoomTo => {
                                if let Some(nid) = self.search.current_match() {
                                    if tree.unfold_path_to(nid) {
                                        node_renderer::measure_all_nodes(tree, ui.painter());
                                        reingold_tilford::layout(tree);
                                    }
                                    // Zoom so the node fills ~25% of screen width, then center
                                    let node = &tree.nodes[nid];
                                    let canvas_pos = node.layout_pos;
                                    let target_zoom = (screen_rect.width() * 0.25 / node.layout_size.x)
                                        .clamp(1.0, 3.0);
                                    self.viewport.zoom = target_zoom;
                                    self.viewport.offset = egui::vec2(
                                        -canvas_pos.x * target_zoom,
                                        -canvas_pos.y * target_zoom,
                                    );
                                    // Select the node and close search
                                    self.selection.select_single(nid);
                                    self.search.close();
                                }
                            }
                            SearchBarAction::ReplaceOne => {
                                if let Some(node_id) = self.search.current_match() {
                                    let old_text = tree.nodes[node_id].text.clone();
                                    let new_text = old_text.replace(&self.search.query, &self.search.replace_text);
                                    if new_text != old_text {
                                        tree.nodes[node_id].text = new_text.clone();
                                        self.history.push(crate::history::Action::EditText { node_id, old_text, new_text });
                                        node_renderer::measure_all_nodes(tree, ui.painter());
                                        reingold_tilford::layout(tree);
                                    }
                                    self.search.update_matches_force(tree);
                                    self.search.next();
                                }
                            }
                            SearchBarAction::ReplaceAll => {
                                let ids = self.search.matches.clone();
                                let mut batch = vec![];
                                for node_id in ids {
                                    let old_text = tree.nodes[node_id].text.clone();
                                    let new_text = old_text.replace(&self.search.query, &self.search.replace_text);
                                    if new_text != old_text {
                                        tree.nodes[node_id].text = new_text.clone();
                                        batch.push(crate::history::Action::EditText { node_id, old_text, new_text });
                                    }
                                }
                                if !batch.is_empty() {
                                    self.history.push(crate::history::Action::Batch(batch));
                                    node_renderer::measure_all_nodes(tree, ui.painter());
                                    reingold_tilford::layout(tree);
                                }
                                self.search.update_matches_force(tree);
                            }
                        }

                        // Auto-scroll to current match when query changes
                        if let Some(nid) = self.search.current_match() {
                            if tree.unfold_path_to(nid) {
                                node_renderer::measure_all_nodes(tree, ui.painter());
                                reingold_tilford::layout(tree);
                            }
                        }

                        // If user clicked a node that is a search match, jump to it
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                                if let Some(node_id) = input::find_node_at(pointer, &self.node_rects) {
                                    if self.search.matches.contains(&node_id) {
                                        self.search.jump_to_node(node_id);
                                    }
                                }
                            }
                        }
                    }

                    // --- Hamburger menu (drawn on top of everything) ---
                    let hamburger_rect = egui::Rect::from_min_size(
                        egui::pos2(screen_rect.min.x + 16.0, screen_rect.min.y + 16.0),
                        egui::vec2(36.0, 36.0),
                    );

                    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
                    let hamburger_hovered =
                        pointer_pos.map_or(false, |p| hamburger_rect.contains(p));

                    draw_hamburger_button(
                        ui.painter(),
                        hamburger_rect,
                        hamburger_hovered,
                        self.menu_open,
                        self.dark_mode,
                    );

                    // Menu panel
                    let mut menu_action = MenuAction::None;
                    if self.menu_open {
                        let panel_pos = egui::pos2(
                            hamburger_rect.min.x,
                            hamburger_rect.max.y + 8.0,
                        );
                        menu_action = draw_menu_panel(ui, panel_pos, &self.recent_files, self.history.can_undo(), self.history.can_redo(), self.dark_mode);

                        // Click outside menu + hamburger → close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let panel_rect = menu_panel_rect(panel_pos, self.recent_files.len());
                            let clicked_in_menu = pointer_pos
                                .map_or(false, |p| panel_rect.contains(p));
                            let clicked_in_hamburger = pointer_pos
                                .map_or(false, |p| hamburger_rect.contains(p));
                            if !clicked_in_menu && !clicked_in_hamburger {
                                self.menu_open = false;
                            }
                        }
                    }

                    // Hamburger click toggle
                    if ui.input(|i| i.pointer.primary_clicked()) {
                        if hamburger_hovered && menu_action == MenuAction::None {
                            self.menu_open = !self.menu_open;
                            if self.menu_open {
                                // Close other panels when hamburger opens
                                self.style_panel_open = false;
                                self.style_selected_depth = None;
                                self.context_menu = None;
                                self.search.close();
                            }
                        }
                    }

                    // Cursor for hamburger
                    if hamburger_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // --- Style (palette) button ---
                    let style_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(hamburger_rect.max.x + 8.0, hamburger_rect.min.y),
                        egui::vec2(36.0, 36.0),
                    );
                    let style_btn_hovered = pointer_pos.map_or(false, |p| style_btn_rect.contains(p));
                    draw_style_button(ui.painter(), style_btn_rect, style_btn_hovered, self.style_panel_open, self.dark_mode);

                    if style_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Style button click toggle
                    let mut _style_panel_clicked_inside = false;
                    if ui.input(|i| i.pointer.primary_clicked()) {
                        if style_btn_hovered && menu_action == MenuAction::None {
                            self.style_panel_open = !self.style_panel_open;
                            if self.style_panel_open {
                                self.menu_open = false;
                                self.context_menu = None;
                                self.search.close();
                            } else {
                                self.style_selected_depth = None;
                            }
                        }
                    }

                    // Style panel
                    if self.style_panel_open {
                        let panel_pos = egui::pos2(
                            style_btn_rect.min.x,
                            style_btn_rect.max.y + 8.0,
                        );
                        let panel_rect = style_panel_rect(panel_pos, self.style_selected_depth);

                        let result = draw_style_panel(
                            ui,
                            panel_pos,
                            self.style_selected_depth,
                            &self.depth_color_config,
                            self.dark_mode,
                        );

                        match result {
                            StyleAction::None => {}
                            StyleAction::SelectDepth(d) => {
                                if self.style_selected_depth == Some(d) {
                                    self.style_selected_depth = None;
                                } else {
                                    self.style_selected_depth = Some(d);
                                }
                            }
                            StyleAction::SetColor(depth, idx) => {
                                self.depth_color_config.set_fill_index(depth, idx);
                            }
                            StyleAction::ResetAll => {
                                self.depth_color_config.reset_all();
                            }
                        }

                        // Click outside style panel → close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let ptr = pointer_pos;
                            let in_panel = ptr.map_or(false, |p| panel_rect.contains(p));
                            let in_btn = ptr.map_or(false, |p| style_btn_rect.contains(p));
                            if in_panel || in_btn {
                                _style_panel_clicked_inside = true;
                            }
                            if !in_panel && !in_btn {
                                self.style_panel_open = false;
                                self.style_selected_depth = None;
                            }
                        }
                    }

                    // --- Search button ---
                    let search_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(style_btn_rect.max.x + 8.0, style_btn_rect.min.y),
                        egui::vec2(36.0, 36.0),
                    );
                    let search_btn_hovered = pointer_pos.map_or(false, |p| search_btn_rect.contains(p));
                    draw_search_button(ui.painter(), search_btn_rect, search_btn_hovered, self.search.is_active(), self.dark_mode);

                    if search_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if ui.input(|i| i.pointer.primary_clicked()) {
                        if search_btn_hovered && menu_action == MenuAction::None {
                            if self.search.is_active() {
                                self.search.close();
                            } else {
                                self.search.open();
                                self.menu_open = false;
                                self.style_panel_open = false;
                                self.style_selected_depth = None;
                                self.context_menu = None;
                            }
                        }
                    }

                    // --- Notes button ---
                    let notes_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(search_btn_rect.max.x + 8.0, search_btn_rect.min.y),
                        egui::vec2(36.0, 36.0),
                    );
                    let notes_btn_hovered = pointer_pos.map_or(false, |p| notes_btn_rect.contains(p));
                    draw_notes_button(ui.painter(), notes_btn_rect, notes_btn_hovered, self.notes_panel_open, self.dark_mode);

                    if notes_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if ui.input(|i| i.pointer.primary_clicked()) {
                        if notes_btn_hovered && menu_action == MenuAction::None {
                            self.notes_panel_open = !self.notes_panel_open;
                            if self.notes_panel_open {
                                self.notes_suppress_close = true;
                                self.menu_open = false;
                                self.style_panel_open = false;
                                self.style_selected_depth = None;
                                self.context_menu = None;
                                self.search.close();
                            } else {
                                self.notes_edit_node = None;
                            }
                        }
                    }

                    // --- Zoom controls ---
                    {
                        let zoom_pct = (self.viewport.zoom * 100.0).round() as i32;
                        let minus_rect = egui::Rect::from_min_size(
                            egui::pos2(notes_btn_rect.max.x + 8.0, notes_btn_rect.min.y),
                            egui::vec2(28.0, 36.0),
                        );
                        let zoom_display_rect = egui::Rect::from_min_size(
                            egui::pos2(minus_rect.max.x + 4.0, notes_btn_rect.min.y),
                            egui::vec2(56.0, 36.0),
                        );
                        let plus_rect = egui::Rect::from_min_size(
                            egui::pos2(zoom_display_rect.max.x + 4.0, notes_btn_rect.min.y),
                            egui::vec2(28.0, 36.0),
                        );

                        let minus_hovered = pointer_pos.map_or(false, |p| minus_rect.contains(p));
                        let zoom_hovered = pointer_pos.map_or(false, |p| zoom_display_rect.contains(p));
                        let plus_hovered = pointer_pos.map_or(false, |p| plus_rect.contains(p));

                        draw_zoom_controls(
                            ui.painter(),
                            minus_rect, zoom_display_rect, plus_rect,
                            zoom_pct,
                            minus_hovered, zoom_hovered, plus_hovered,
                            self.dark_mode,
                        );

                        if minus_hovered || zoom_hovered || plus_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if ui.input(|i| i.pointer.primary_clicked()) {
                            if minus_hovered {
                                let center = screen_rect.center();
                                self.viewport.zoom_around(center, -0.20, screen_rect);
                            } else if plus_hovered {
                                let center = screen_rect.center();
                                self.viewport.zoom_around(center, 0.25, screen_rect);
                            } else if zoom_hovered {
                                self.viewport.zoom = 1.0;
                                self.viewport.offset = egui::Vec2::ZERO;
                            }
                        }
                    }

                    // --- Link edit bar ---
                    if self.link_edit.is_some() {
                        let suppress = std::mem::replace(&mut self.link_edit_suppress_close, false);
                        let link_result = draw_link_edit_bar(ui, &mut self.link_edit, screen_rect, self.dark_mode);
                        match link_result {
                            LinkEditAction::None => {}
                            LinkEditAction::Confirm => {
                                if let Some((node_id, new_url)) = self.link_edit.take() {
                                    let old_link = tree.nodes[node_id].link.clone();
                                    let new_link = if new_url.trim().is_empty() { None } else { Some(new_url.trim().to_string()) };
                                    tree.nodes[node_id].link = new_link.clone();
                                    self.history.push(crate::history::Action::SetLink { node_id, old_link, new_link });
                                }
                            }
                            LinkEditAction::Cancel => {
                                self.link_edit = None;
                            }
                        }
                        // Click outside → close (unless suppressed this frame)
                        if !suppress && ui.input(|i| i.pointer.primary_clicked()) {
                            let bar_rect = link_edit_bar_rect(screen_rect);
                            let in_bar = pointer_pos.map_or(false, |p| bar_rect.contains(p));
                            if !in_bar {
                                self.link_edit = None;
                            }
                        }
                    }

                    // --- Notes panel ---
                    if self.notes_panel_open {
                        let toolbar_bottom = hamburger_rect.max.y;
                        let panel_x = screen_rect.max.x - NOTES_PANEL_WIDTH - 8.0;
                        let panel_y = toolbar_bottom + 8.0;
                        let panel_h = (screen_rect.height() - toolbar_bottom - 16.0)
                            .max(NOTES_PANEL_MIN_HEIGHT);
                        let notes_panel_rect = egui::Rect::from_min_size(
                            egui::pos2(panel_x, panel_y),
                            egui::vec2(NOTES_PANEL_WIDTH, panel_h),
                        );

                        // Compute "Saved" label alpha from timestamp
                        let now = ui.input(|i| i.time);
                        const HOLD_SECS: f64 = 1.2;
                        const FADE_SECS: f64 = 0.8;
                        let saved_alpha = if let Some(saved_at) = self.notes_saved_at {
                            let elapsed = now - saved_at;
                            if elapsed < HOLD_SECS {
                                1.0_f32
                            } else if elapsed < HOLD_SECS + FADE_SECS {
                                (1.0 - (elapsed - HOLD_SECS) / FADE_SECS) as f32
                            } else {
                                0.0_f32
                            }
                        } else {
                            0.0_f32
                        };
                        if saved_alpha > 0.0 {
                            ui.ctx().request_repaint();
                        }

                        let result = draw_notes_panel(
                            ui,
                            notes_panel_rect,
                            tree,
                            &mut self.notes_edit_node,
                            &self.selection,
                            &self.depth_color_config,
                            saved_alpha,
                            self.dark_mode,
                        );
                        self.notes_focused = result.text_focused;
                        if result.notes_changed {
                            input::save_file(tree, &mut self.file_path);
                            self.notes_saved_at = Some(now);
                        }
                        if result.close {
                            self.notes_panel_open = false;
                            self.notes_edit_node = None;
                        }
                        if let Some(nav_id) = result.navigate_to {
                            self.selection.select_single(nav_id);
                            let node = &tree.nodes[nav_id];
                            let canvas_pos = node.layout_pos;
                            let target_zoom = (screen_rect.width() * 0.25 / node.layout_size.x).clamp(1.0, 3.0);
                            self.viewport.zoom = target_zoom;
                            self.viewport.offset = egui::vec2(
                                -canvas_pos.x * target_zoom,
                                -canvas_pos.y * target_zoom,
                            );
                        }

                        // Click outside notes panel → close.
                        // notes_suppress_close is set for one frame whenever the panel opens,
                        // preventing the opening click from immediately closing it.
                        let suppress = std::mem::replace(&mut self.notes_suppress_close, false);
                        if !suppress && ui.input(|i| i.pointer.primary_clicked()) {
                            let ptr = pointer_pos;
                            let in_panel = ptr.map_or(false, |p| notes_panel_rect.contains(p));
                            let in_btn = ptr.map_or(false, |p| notes_btn_rect.contains(p));
                            if !in_panel && !in_btn {
                                self.notes_panel_open = false;
                                self.notes_edit_node = None;
                            }
                        }
                    }

                    // Handle menu action
                    match menu_action {
                        MenuAction::NewMap => { self.new_map(); }
                        MenuAction::OpenFile => {
                            self.menu_open = false;
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("FreeMind", &["mm"])
                                .pick_file()
                            {
                                self.load_file(path);
                            }
                        }
                        MenuAction::Save => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                input::save_file(tree, &mut self.file_path);
                                self.history.mark_clean();
                            }
                        }
                        MenuAction::SaveAs => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("FreeMind", &["mm"])
                                    .save_file()
                                {
                                    match crate::io::freemind_write::save_mm_file(tree, &path)
                                    {
                                        Ok(_) => {
                                            self.file_path = Some(path);
                                            self.history.mark_clean();
                                            log::info!("File saved successfully");
                                        }
                                        Err(e) => log::error!("Failed to save: {}", e),
                                    }
                                }
                            }
                        }
                        MenuAction::ExportSvg => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                let svg = export::svg::export_svg(tree, &self.depth_color_config);
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("SVG", &["svg"])
                                    .save_file()
                                {
                                    let _ = std::fs::write(&path, svg);
                                }
                            }
                        }
                        MenuAction::ExportPng => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                if let Some(png_data) = export::png::export_png(tree, &self.depth_color_config) {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("PNG", &["png"])
                                        .save_file()
                                    {
                                        let _ = std::fs::write(&path, png_data);
                                    }
                                }
                            }
                        }
                        MenuAction::ExportMarkdown => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                let md = export::markdown::export_markdown(tree);
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Markdown", &["md"])
                                    .save_file()
                                {
                                    let _ = std::fs::write(&path, md);
                                }
                            }
                        }
                        MenuAction::ExportOpml => {
                            self.menu_open = false;
                            if let Some(ref tree) = self.tree {
                                let opml = export::opml::export_opml(tree);
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("OPML", &["opml"])
                                    .save_file()
                                {
                                    let _ = std::fs::write(&path, opml);
                                }
                            }
                        }
                        MenuAction::ToggleDarkMode => {
                            self.dark_mode = !self.dark_mode;
                            self.menu_open = false;
                        }
                        MenuAction::Undo => {
                            self.menu_open = false;
                            if let Some(ref mut tree) = self.tree {
                                if self.history.undo(tree) {
                                    node_renderer::measure_all_nodes(tree, ui.painter());
                                    reingold_tilford::layout(tree);
                                }
                            }
                        }
                        MenuAction::Redo => {
                            self.menu_open = false;
                            if let Some(ref mut tree) = self.tree {
                                if self.history.redo(tree) {
                                    node_renderer::measure_all_nodes(tree, ui.painter());
                                    reingold_tilford::layout(tree);
                                }
                            }
                        }
                        MenuAction::CloseToWelcome => { self.close_to_welcome(); }
                        MenuAction::OpenRecentFile(idx) => {
                            self.menu_open = false;
                            if let Some(path) = self.recent_files.get(idx).cloned() {
                                self.load_file(path);
                            }
                        }
                        MenuAction::Quit => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        MenuAction::ToggleHelp => {
                            self.menu_open = false;
                            self.help_open = !self.help_open;
                            if self.help_open {
                                self.help_suppress_close = true;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // No file loaded — show welcome screen
                    let action = draw_welcome_screen(ui, screen_rect, &self.depth_color_config, &self.recent_files);
                    match action {
                        WelcomeAction::None => {}
                        WelcomeAction::NewMap => { self.new_map(); }
                        WelcomeAction::OpenFile => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("FreeMind", &["mm"])
                                .pick_file()
                            {
                                self.load_file(path);
                            }
                        }
                        WelcomeAction::OpenRecentFile(idx) => {
                            if let Some(path) = self.recent_files.get(idx).cloned() {
                                self.load_file(path);
                            }
                        }
                    }
                }

                // --- Help overlay (drawn on top of everything) ---
                if self.help_open {
                    let suppress = std::mem::replace(&mut self.help_suppress_close, false);
                    if !suppress && draw_help_overlay(ui, screen_rect, self.dark_mode) {
                        self.help_open = false;
                    }
                }
            });

        // Handle file drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped_files.first() {
            if let Some(ref path) = file.path {
                let p = path.clone();
                self.load_file(p);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Menu types
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum MenuAction {
    None,
    NewMap,
    OpenFile,
    OpenRecentFile(usize),
    Save,
    SaveAs,
    ExportSvg,
    ExportPng,
    ExportMarkdown,
    ExportOpml,
    ResetZoom,
    ZoomIn,
    ZoomOut,
    CloseToWelcome,
    CloseMenu,
    OpenSearch,
    CloseSearch,
    OpenReplace,
    ToggleNotes,
    Quit,
    Undo,
    Redo,
    FitToScreen,
    ToggleBold,
    FoldAll,
    UnfoldAll,
    ToggleHelp,
    ToggleDarkMode,
    ExportSubmenu,
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum ContextAction {
    None,
    AddChild,
    AddSibling,
    Edit,
    Cut,
    Copy,
    Paste,
    Delete,
    OpenColorPicker,
    SetLevelColor(usize, usize), // (depth, palette_index)
    ToggleFold,
    ViewNotes,
    FoldAll,
    UnfoldAll,
    OpenLink,
    EditLink,
    RemoveLink,
}

const CTX_MENU_WIDTH: f32 = 260.0;

struct CtxMenuItem {
    label: String,
    shortcut: &'static str,
    action: ContextAction,
    enabled: bool,
}

fn build_context_items(
    on_node: bool,
    selection: &Selection,
    clipboard: &Clipboard,
    tree: &MindmapTree,
) -> Vec<Option<CtxMenuItem>>
{
    if !on_node {
        // Right-clicked empty canvas — only Paste if clipboard has content
        if clipboard.is_empty() {
            return Vec::new();
        }
        // Need a selection target for paste
        if selection.primary().is_none() {
            return Vec::new();
        }
        return vec![Some(CtxMenuItem {
            label: "Paste".to_string(),
            shortcut: "Ctrl+V",
            action: ContextAction::Paste,
            enabled: true,
        })];
    }

    let primary = selection.primary();
    let is_root = primary.map_or(false, |id| id == tree.root);
    let is_multi = selection.selected.len() > 1;
    let is_leaf = primary.map_or(true, |id| tree.nodes[id].children.is_empty());
    let is_folded = primary.map_or(false, |id| tree.nodes[id].folded);
    let has_link = primary.map_or(false, |id| tree.nodes[id].link.is_some());

    let fold_label = if is_folded { "Unfold" } else { "Fold" };

    // Build link items (only for single-node selection)
    let link_items: Vec<Option<CtxMenuItem>> = if !is_multi {
        if has_link {
            vec![
                None, // divider
                Some(CtxMenuItem {
                    label: "Open Link".to_string(),
                    shortcut: "",
                    action: ContextAction::OpenLink,
                    enabled: true,
                }),
                Some(CtxMenuItem {
                    label: "Edit Link...".to_string(),
                    shortcut: "",
                    action: ContextAction::EditLink,
                    enabled: true,
                }),
                Some(CtxMenuItem {
                    label: "Remove Link".to_string(),
                    shortcut: "",
                    action: ContextAction::RemoveLink,
                    enabled: true,
                }),
            ]
        } else {
            vec![
                None, // divider
                Some(CtxMenuItem {
                    label: "Add Link...".to_string(),
                    shortcut: "",
                    action: ContextAction::EditLink,
                    enabled: true,
                }),
            ]
        }
    } else {
        vec![]
    };

    let mut items = vec![
        Some(CtxMenuItem {
            label: "Add Child".to_string(),
            shortcut: "Tab",
            action: ContextAction::AddChild,
            enabled: true,
        }),
        Some(CtxMenuItem {
            label: "Add Sibling".to_string(),
            shortcut: "Enter",
            action: ContextAction::AddSibling,
            enabled: !is_root,
        }),
        None, // divider
        Some(CtxMenuItem {
            label: "Edit".to_string(),
            shortcut: "F2",
            action: ContextAction::Edit,
            enabled: !is_multi,
        }),
        Some(CtxMenuItem {
            label: "Notes".to_string(),
            shortcut: "Ctrl+Shift+N",
            action: ContextAction::ViewNotes,
            enabled: !is_multi,
        }),
        None, // divider
        Some(CtxMenuItem {
            label: "Cut".to_string(),
            shortcut: "Ctrl+X",
            action: ContextAction::Cut,
            enabled: !is_root,
        }),
        Some(CtxMenuItem {
            label: "Copy".to_string(),
            shortcut: "Ctrl+C",
            action: ContextAction::Copy,
            enabled: true,
        }),
        Some(CtxMenuItem {
            label: "Paste".to_string(),
            shortcut: "Ctrl+V",
            action: ContextAction::Paste,
            enabled: !clipboard.is_empty(),
        }),
        None, // divider
        Some(CtxMenuItem {
            label: "Delete".to_string(),
            shortcut: "Del",
            action: ContextAction::Delete,
            enabled: !is_root,
        }),
        None, // divider
        Some(CtxMenuItem {
            label: "Level Color...".to_string(),
            shortcut: "",
            action: ContextAction::OpenColorPicker,
            enabled: true,
        }),
        Some(CtxMenuItem {
            label: fold_label.to_string(),
            shortcut: "Ctrl+.",
            action: ContextAction::ToggleFold,
            enabled: !is_leaf,
        }),
        None, // divider
        Some(CtxMenuItem {
            label: "Fold All".to_string(),
            shortcut: "Ctrl+Shift+-",
            action: ContextAction::FoldAll,
            enabled: true,
        }),
        Some(CtxMenuItem {
            label: "Unfold All".to_string(),
            shortcut: "Ctrl+Shift+=",
            action: ContextAction::UnfoldAll,
            enabled: true,
        }),
    ];
    items.extend(link_items);
    items
}

const LEVEL_COLOR_ITEM_HEIGHT: f32 = 44.0; // taller to fit subtitle

fn ctx_menu_height(items: &[Option<CtxMenuItem>], color_picker_open: bool) -> f32 {
    let mut h = MENU_PAD_Y * 2.0;
    for item in items {
        match item {
            Some(mi) if mi.action == ContextAction::OpenColorPicker => {
                h += LEVEL_COLOR_ITEM_HEIGHT;
                if color_picker_open {
                    // Swatch grid: header + 5 rows of swatches + padding
                    let grid_h = 20.0 + SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
                    h += grid_h;
                }
            }
            Some(_) => h += ITEM_HEIGHT,
            None => h += DIVIDER_HEIGHT,
        }
    }
    h
}

fn context_menu_rect(
    pos: egui::Pos2,
    on_node: bool,
    clipboard: &Clipboard,
    tree: &MindmapTree,
    selection: &Selection,
    screen_rect: egui::Rect,
    color_picker_open: bool,
) -> egui::Rect {
    let items = build_context_items(on_node, selection, clipboard, tree);
    let h = ctx_menu_height(&items, color_picker_open);
    let w = CTX_MENU_WIDTH;

    // Clamp to screen bounds
    let mut x = pos.x;
    let mut y = pos.y;
    if x + w > screen_rect.max.x - 4.0 {
        x = pos.x - w; // flip left
    }
    if y + h > screen_rect.max.y - 4.0 {
        y = pos.y - h; // flip up
    }
    x = x.max(screen_rect.min.x + 4.0);
    y = y.max(screen_rect.min.y + 4.0);

    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

fn draw_context_menu(
    ui: &egui::Ui,
    state: &mut ContextMenuState,
    selection: &Selection,
    clipboard: &Clipboard,
    tree: &MindmapTree,
    screen_rect: egui::Rect,
    color_config: &DepthColorConfig,
    dark_mode: bool,
) -> ContextAction {
    let on_node = state.target_node.is_some();
    let items = build_context_items(on_node, selection, clipboard, tree);
    if items.is_empty() {
        return ContextAction::None;
    }

    // Determine the depth of the target node for the color picker
    let target_depth = state.target_node.map(|id| tree.depth(id));

    let panel_rect = context_menu_rect(state.pos, on_node, clipboard, tree, selection, screen_rect, state.color_picker_open);
    let painter = ui.painter();

    // Derive wobble seed from position
    let seed = (state.pos.x as u32).wrapping_mul(31).wrapping_add(state.pos.y as u32);

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(3.0, 3.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, seed, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Draw items
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut action = ContextAction::None;
    let mut y = panel_rect.min.y + MENU_PAD_Y;
    let mut div_idx: u32 = 0;

    let label_color = colors::ui_text(dark_mode);
    let disabled_label_color = colors::ui_text_muted(dark_mode);
    let shortcut_color = colors::ui_text_muted(dark_mode);
    let disabled_shortcut_color = colors::ui_text_muted(dark_mode);
    let muted_color = colors::ui_text_muted(dark_mode);

    // Clear preview each frame (will be re-set if hovering a swatch)
    state.preview_color = None;

    for item in &items {
        match item {
            Some(menu_item) if menu_item.action == ContextAction::OpenColorPicker => {
                // Special rendering for "Level Color..." item
                let item_h = LEVEL_COLOR_ITEM_HEIGHT;
                let item_rect = egui::Rect::from_min_size(
                    egui::pos2(panel_rect.min.x + 4.0, y),
                    egui::vec2(CTX_MENU_WIDTH - 8.0, item_h),
                );
                let hovered = pointer_pos.map_or(false, |p| item_rect.contains(p));

                // Hover background
                if hovered || state.color_picker_open {
                    painter.add(RectShape::new(
                        item_rect,
                        egui::CornerRadius::same(4),
                        colors::hover_bg(dark_mode),
                        egui::Stroke::NONE,
                        StrokeKind::Outside,
                    ));
                }
                if hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Color dot indicator (current color for this depth)
                if let Some(depth) = target_depth {
                    let fill_idx = color_config.get_fill_index(depth);
                    let fill_color = colors::depth_fill_color(fill_idx);
                    let dot_rect = egui::Rect::from_min_size(
                        egui::pos2(item_rect.min.x + 12.0, y + 7.0),
                        egui::vec2(12.0, 12.0),
                    );
                    painter.rect_filled(dot_rect, 3.0, fill_color);
                    painter.rect_stroke(dot_rect, 3.0, egui::Stroke::new(0.8, colors::border_color(dark_mode)), StrokeKind::Outside);
                }

                // Label (shifted right for color dot)
                painter.text(
                    egui::pos2(item_rect.min.x + 30.0, y + 13.0),
                    egui::Align2::LEFT_CENTER,
                    &menu_item.label,
                    egui::FontId::proportional(14.0),
                    label_color,
                );

                // Subtitle: "Affects all Level N nodes"
                if let Some(depth) = target_depth {
                    let level_name = if depth == 0 { "Root".to_string() } else { format!("Level {}", depth) };
                    painter.text(
                        egui::pos2(item_rect.min.x + 30.0, y + 32.0),
                        egui::Align2::LEFT_CENTER,
                        format!("Affects all {} nodes", level_name),
                        egui::FontId::proportional(11.0),
                        muted_color,
                    );
                }

                // Click toggles color picker
                if hovered && clicked {
                    state.color_picker_open = !state.color_picker_open;
                    if let Some(depth) = target_depth {
                        state.color_picker_depth = Some(depth);
                    }
                }

                y += item_h;

                // Draw inline swatch grid if open
                if state.color_picker_open {
                    if let Some(depth) = state.color_picker_depth {
                        let current_idx = color_config.get_fill_index(depth);

                        // Count nodes at this depth
                        let visible = tree.visible_nodes();
                        let node_count = visible.iter().filter(|&&id| tree.depth(id) % 8 == depth % 8).count();
                        let level_name = if depth == 0 { "Root".to_string() } else { format!("Level {}", depth) };

                        // Header
                        painter.text(
                            egui::pos2(panel_rect.min.x + 16.0, y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{} \u{2014} {} nodes", level_name, node_count),
                            egui::FontId::proportional(11.0),
                            muted_color,
                        );
                        y += 20.0;

                        // Swatch grid
                        let grid_x = panel_rect.min.x + 16.0;
                        for row in 0..SWATCH_ROWS {
                            for col in 0..SWATCH_COLS {
                                let idx = row * SWATCH_COLS + col;
                                if idx >= colors::DEPTH_FILL_COUNT { break; }
                                let sx = grid_x + col as f32 * (SWATCH_SIZE + SWATCH_GAP);
                                let sy = y + row as f32 * (SWATCH_SIZE + SWATCH_GAP);
                                let swatch_r = egui::Rect::from_min_size(
                                    egui::pos2(sx, sy),
                                    egui::vec2(SWATCH_SIZE, SWATCH_SIZE),
                                );

                                let color = colors::depth_fill_color(idx);
                                painter.rect_filled(swatch_r, 4.0, color);

                                // Highlight current selection
                                if current_idx == idx {
                                    painter.rect_stroke(
                                        swatch_r.expand(1.0), 4.0,
                                        egui::Stroke::new(2.0, colors::border_color(dark_mode)),
                                        StrokeKind::Outside,
                                    );
                                }

                                let swatch_hovered = pointer_pos.map_or(false, |p| swatch_r.contains(p));
                                if swatch_hovered {
                                    painter.rect_stroke(
                                        swatch_r, 4.0,
                                        egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 80)),
                                        StrokeKind::Outside,
                                    );
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);

                                    // Live preview
                                    state.preview_color = Some((depth % 8, idx));

                                    if clicked {
                                        action = ContextAction::SetLevelColor(depth, idx);
                                    }
                                }
                            }
                        }
                        let grid_h = SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
                        y += grid_h;
                    }
                }
            }
            Some(menu_item) => {
                let item_rect = egui::Rect::from_min_size(
                    egui::pos2(panel_rect.min.x + 4.0, y),
                    egui::vec2(CTX_MENU_WIDTH - 8.0, ITEM_HEIGHT),
                );
                let hovered = pointer_pos.map_or(false, |p| item_rect.contains(p));

                if menu_item.enabled {
                    // Hover background
                    if hovered {
                        painter.add(RectShape::new(
                            item_rect,
                            egui::CornerRadius::same(4),
                            colors::hover_bg(dark_mode),
                            egui::Stroke::NONE,
                            StrokeKind::Outside,
                        ));
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Label
                    painter.text(
                        egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &menu_item.label,
                        egui::FontId::proportional(14.0),
                        label_color,
                    );

                    // Shortcut
                    if !menu_item.shortcut.is_empty() {
                        painter.text(
                            egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            menu_item.shortcut,
                            egui::FontId::proportional(13.0),
                            shortcut_color,
                        );
                    }

                    // Click
                    if hovered && clicked {
                        action = menu_item.action;
                    }
                } else {
                    // Disabled item
                    painter.text(
                        egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &menu_item.label,
                        egui::FontId::proportional(14.0),
                        disabled_label_color,
                    );
                    if !menu_item.shortcut.is_empty() {
                        painter.text(
                            egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            menu_item.shortcut,
                            egui::FontId::proportional(13.0),
                            disabled_shortcut_color,
                        );
                    }
                }

                y += ITEM_HEIGHT;
            }
            None => {
                // Wobbled divider
                let div_y = y + DIVIDER_HEIGHT / 2.0;
                let div_seed = seed.wrapping_add(700 + div_idx);
                div_idx += 1;
                let div_paths = wobble::rough_line(
                    egui::pos2(panel_rect.min.x + 12.0, div_y),
                    egui::pos2(panel_rect.max.x - 12.0, div_y),
                    div_seed,
                    &RoughOptions {
                        roughness: 0.6,
                        max_randomness_offset: 0.8,
                        bowing: 0.3,
                        disable_multi_stroke: true,
                        ..Default::default()
                    },
                );
                let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
                for path in div_paths {
                    if path.len() >= 2 {
                        painter.add(PathShape::line(path, div_stroke));
                    }
                }
                y += DIVIDER_HEIGHT;
            }
        }
    }

    action
}

// ---------------------------------------------------------------------------
// Hamburger button
// ---------------------------------------------------------------------------

fn draw_hamburger_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    _menu_open: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    // Background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 999, &rough_opts);
    let stroke_width = if hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Three wobbly horizontal lines
    let cx = rect.center().x;
    let cy = rect.center().y;
    let line_half_w = 8.0;
    let line_gap = 5.0;
    let line_color = colors::border_color(dark_mode);
    let line_stroke = egui::Stroke::new(1.5, line_color);

    let line_opts = RoughOptions {
        roughness: 0.6,
        max_randomness_offset: 0.8,
        bowing: 0.3,
        disable_multi_stroke: true,
        ..Default::default()
    };

    for (i, dy) in [-line_gap, 0.0, line_gap].iter().enumerate() {
        let y = cy + dy;
        let seed = 1000 + i as u32;
        let paths = wobble::rough_line(
            egui::pos2(cx - line_half_w, y),
            egui::pos2(cx + line_half_w, y),
            seed,
            &line_opts,
        );
        for path in paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, line_stroke));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Menu panel
// ---------------------------------------------------------------------------

const MENU_WIDTH: f32 = 260.0;
const ITEM_HEIGHT: f32 = 32.0;
const DIVIDER_HEIGHT: f32 = 9.0;
const MENU_PAD_Y: f32 = 6.0;

struct MenuItem {
    label: &'static str,
    shortcut: &'static str,
    action: MenuAction,
}

const MENU_ITEMS: &[Option<MenuItem>] = &[
    Some(MenuItem { label: "New Mind Map", shortcut: "Ctrl+N", action: MenuAction::NewMap }),
    Some(MenuItem { label: "Open File...", shortcut: "Ctrl+O", action: MenuAction::OpenFile }),
    None, // divider
    Some(MenuItem { label: "Save", shortcut: "Ctrl+S", action: MenuAction::Save }),
    Some(MenuItem { label: "Save As...", shortcut: "Ctrl+Shift+S", action: MenuAction::SaveAs }),
    Some(MenuItem { label: "Export", shortcut: "", action: MenuAction::ExportSubmenu }),
    None, // divider
    Some(MenuItem { label: "Keyboard Shortcuts", shortcut: "?", action: MenuAction::ToggleHelp }),
    None, // divider
    Some(MenuItem { label: "Dark Mode", shortcut: "", action: MenuAction::ToggleDarkMode }),
    None, // divider
    Some(MenuItem { label: "Close to Welcome", shortcut: "", action: MenuAction::CloseToWelcome }),
    Some(MenuItem { label: "Quit", shortcut: "Ctrl+Q", action: MenuAction::Quit }),
];

fn menu_panel_height(n_recent: usize) -> f32 {
    let mut h = MENU_PAD_Y * 2.0;
    for item in MENU_ITEMS {
        h += if item.is_some() { ITEM_HEIGHT } else { DIVIDER_HEIGHT };
    }
    // Undo + Redo items + divider after them
    h += ITEM_HEIGHT * 2.0 + DIVIDER_HEIGHT;
    if n_recent > 0 {
        h += DIVIDER_HEIGHT; // section divider
        h += n_recent as f32 * ITEM_HEIGHT;
    }
    h
}

fn menu_panel_rect(pos: egui::Pos2, n_recent: usize) -> egui::Rect {
    egui::Rect::from_min_size(pos, egui::vec2(MENU_WIDTH, menu_panel_height(n_recent)))
}

const SUBMENU_WIDTH: f32 = 180.0;
const EXPORT_SUBMENU_ITEMS: &[(&str, MenuAction)] = &[
    ("SVG...",      MenuAction::ExportSvg),
    ("PNG...",      MenuAction::ExportPng),
    ("Markdown...", MenuAction::ExportMarkdown),
    ("OPML...",     MenuAction::ExportOpml),
];

fn export_submenu_rect(pos: egui::Pos2) -> egui::Rect {
    let h = MENU_PAD_Y * 2.0 + EXPORT_SUBMENU_ITEMS.len() as f32 * ITEM_HEIGHT;
    egui::Rect::from_min_size(pos, egui::vec2(SUBMENU_WIDTH, h))
}

fn draw_export_submenu(ui: &egui::Ui, pos: egui::Pos2, dark_mode: bool) -> MenuAction {
    let painter = ui.painter();
    let panel_rect = export_submenu_rect(pos);

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(3.0, 3.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, 999, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let label_color = colors::ui_text(dark_mode);
    let hover_color = colors::hover_bg(dark_mode);
    let mut action = MenuAction::None;
    let mut y = panel_rect.min.y + MENU_PAD_Y;

    for &(label, menu_action) in EXPORT_SUBMENU_ITEMS {
        let item_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.min.x + 4.0, y),
            egui::vec2(SUBMENU_WIDTH - 8.0, ITEM_HEIGHT),
        );
        let hovered = pointer_pos.map_or(false, |p| item_rect.contains(p));
        if hovered {
            painter.add(RectShape::new(
                item_rect,
                egui::CornerRadius::same(4),
                hover_color,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        painter.text(
            egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.0),
            label_color,
        );
        if hovered && clicked {
            action = menu_action;
        }
        y += ITEM_HEIGHT;
    }

    action
}

fn draw_menu_panel(ui: &egui::Ui, pos: egui::Pos2, recent_files: &[std::path::PathBuf], can_undo: bool, can_redo: bool, dark_mode: bool) -> MenuAction {
    let painter = ui.painter();
    let panel_rect = menu_panel_rect(pos, recent_files.len());

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(3.0, 3.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, 888, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Draw items
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut action = MenuAction::None;
    let mut y = panel_rect.min.y + MENU_PAD_Y;
    let mut divider_count = 0;
    let mut export_row_rect: Option<egui::Rect> = None;

    let label_color = colors::ui_text(dark_mode);
    let shortcut_color = colors::ui_text_muted(dark_mode);
    let hover_color = colors::hover_bg(dark_mode);
    let div_color = colors::divider_color(dark_mode);

    // Helper: draw a divider line
    let draw_divider = |painter: &egui::Painter, panel_rect: egui::Rect, y: f32, seed: u32| {
        let div_paths = wobble::rough_line(
            egui::pos2(panel_rect.min.x + 12.0, y + DIVIDER_HEIGHT / 2.0),
            egui::pos2(panel_rect.max.x - 12.0, y + DIVIDER_HEIGHT / 2.0),
            seed,
            &RoughOptions {
                roughness: 0.6,
                max_randomness_offset: 0.8,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, div_color);
        for path in div_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, div_stroke));
            }
        }
    };

    for item in MENU_ITEMS {
        match item {
            Some(menu_item) => {
                let item_rect = egui::Rect::from_min_size(
                    egui::pos2(panel_rect.min.x + 4.0, y),
                    egui::vec2(MENU_WIDTH - 8.0, ITEM_HEIGHT),
                );
                let hovered = pointer_pos.map_or(false, |p| item_rect.contains(p));

                // Hover background
                if hovered {
                    painter.add(RectShape::new(
                        item_rect,
                        egui::CornerRadius::same(4),
                        hover_color,
                        egui::Stroke::NONE,
                        StrokeKind::Outside,
                    ));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Label (with checkmark for dark mode toggle)
                let display_label = if menu_item.action == MenuAction::ToggleDarkMode && dark_mode {
                    format!("\u{2713} {}", menu_item.label)
                } else {
                    menu_item.label.to_string()
                };
                painter.text(
                    egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &display_label,
                    egui::FontId::proportional(14.0),
                    label_color,
                );

                // Shortcut hint or submenu arrow
                if menu_item.action == MenuAction::ExportSubmenu {
                    // Record this row for submenu rendering
                    export_row_rect = Some(item_rect);
                    painter.text(
                        egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        "\u{25B6}",
                        egui::FontId::proportional(11.0),
                        shortcut_color,
                    );
                    // No click action — submenu handles it
                } else if !menu_item.shortcut.is_empty() {
                    painter.text(
                        egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        menu_item.shortcut,
                        egui::FontId::proportional(13.0),
                        shortcut_color,
                    );
                }

                // Click (not for submenu trigger row)
                if hovered && clicked && menu_item.action != MenuAction::ExportSubmenu {
                    action = menu_item.action;
                }

                y += ITEM_HEIGHT;
            }
            None => {
                draw_divider(painter, panel_rect, y, 777u32.wrapping_add(divider_count));
                y += DIVIDER_HEIGHT;
                divider_count += 1;

                // After first divider: inject Undo and Redo items
                if divider_count == 1 {
                    let disabled_color = if dark_mode {
                        egui::Color32::from_rgb(80, 80, 80)
                    } else {
                        egui::Color32::from_rgb(180, 175, 170)
                    };
                    let disabled_shortcut = if dark_mode {
                        egui::Color32::from_rgb(70, 70, 70)
                    } else {
                        egui::Color32::from_rgb(200, 197, 194)
                    };
                    for &(label, shortcut, menu_action, enabled) in &[
                        ("Undo", "Ctrl+Z", MenuAction::Undo, can_undo),
                        ("Redo", "Ctrl+Shift+Z", MenuAction::Redo, can_redo),
                    ] {
                        let item_rect = egui::Rect::from_min_size(
                            egui::pos2(panel_rect.min.x + 4.0, y),
                            egui::vec2(MENU_WIDTH - 8.0, ITEM_HEIGHT),
                        );
                        let hovered = enabled && pointer_pos.map_or(false, |p| item_rect.contains(p));
                        if hovered {
                            painter.add(RectShape::new(
                                item_rect,
                                egui::CornerRadius::same(4),
                                hover_color,
                                egui::Stroke::NONE,
                                StrokeKind::Outside,
                            ));
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let item_label_color = if enabled {
                            label_color
                        } else {
                            disabled_color
                        };
                        painter.text(
                            egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(14.0),
                            item_label_color,
                        );
                        painter.text(
                            egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            shortcut,
                            egui::FontId::proportional(13.0),
                            if enabled { shortcut_color } else { disabled_shortcut },
                        );
                        if hovered && clicked {
                            action = menu_action;
                        }
                        y += ITEM_HEIGHT;
                    }
                    draw_divider(painter, panel_rect, y, 778);
                    y += DIVIDER_HEIGHT;
                }
            }
        }
    }

    // Export submenu (rendered on top, after all items)
    if let Some(row_rect) = export_row_rect {
        let sub_pos = egui::pos2(panel_rect.max.x, row_rect.min.y);
        let sub_rect = export_submenu_rect(sub_pos);
        let show = pointer_pos.map_or(false, |p|
            row_rect.contains(p) || sub_rect.expand(2.0).contains(p)
        );
        if show {
            let sub_action = draw_export_submenu(ui, sub_pos, dark_mode);
            if sub_action != MenuAction::None {
                action = sub_action;
            }
        }
    }

    // Recent files section
    if !recent_files.is_empty() {
        // Divider
        let div_y = y + DIVIDER_HEIGHT / 2.0;
        let div_paths = wobble::rough_line(
            egui::pos2(panel_rect.min.x + 12.0, div_y),
            egui::pos2(panel_rect.max.x - 12.0, div_y),
            888,
            &RoughOptions {
                roughness: 0.6,
                max_randomness_offset: 0.8,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, div_color);
        for path in div_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, div_stroke));
            }
        }
        y += DIVIDER_HEIGHT;

        // Recent file items
        for (idx, path) in recent_files.iter().enumerate() {
            let item_rect = egui::Rect::from_min_size(
                egui::pos2(panel_rect.min.x + 4.0, y),
                egui::vec2(MENU_WIDTH - 8.0, ITEM_HEIGHT),
            );
            let hovered = pointer_pos.map_or(false, |p| item_rect.contains(p));

            if hovered {
                painter.add(RectShape::new(
                    item_rect,
                    egui::CornerRadius::same(4),
                    hover_color,
                    egui::Stroke::NONE,
                    StrokeKind::Outside,
                ));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());

            let parent_str = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().into_owned());

            // Filename on the left
            painter.text(
                egui::pos2(item_rect.min.x + 12.0, item_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &filename,
                egui::FontId::proportional(13.0),
                label_color,
            );

            // Parent directory name on the right in muted gray
            if let Some(dir) = parent_str {
                painter.text(
                    egui::pos2(item_rect.max.x - 12.0, item_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    &dir,
                    egui::FontId::proportional(11.0),
                    shortcut_color,
                );
            }

            if hovered && clicked {
                action = MenuAction::OpenRecentFile(idx);
            }

            y += ITEM_HEIGHT;
        }
    }

    action
}

// ---------------------------------------------------------------------------
// Style panel
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum StyleAction {
    None,
    SelectDepth(usize),
    SetColor(usize, usize),
    ResetAll,
}

const STYLE_PANEL_WIDTH: f32 = 280.0;
const DEPTH_ROW_HEIGHT: f32 = 32.0;
const SWATCH_SIZE: f32 = 24.0;
const SWATCH_GAP: f32 = 4.0;
const SWATCH_COLS: usize = 8;
const SWATCH_ROWS: usize = 5;
const STYLE_TITLE_HEIGHT: f32 = 36.0;

const DEPTH_LABELS: [&str; 8] = [
    "Root", "Level 1", "Level 2", "Level 3",
    "Level 4", "Level 5", "Level 6", "Level 7",
];

const STYLE_FOOTER_HEIGHT: f32 = 36.0;

fn style_panel_height(selected_depth: Option<usize>) -> f32 {
    let mut h = MENU_PAD_Y * 2.0 + STYLE_TITLE_HEIGHT;
    h += DEPTH_ROW_HEIGHT * 8.0;
    if selected_depth.is_some() {
        let grid_h = SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
        h += grid_h;
    }
    h += STYLE_FOOTER_HEIGHT;
    h
}

fn style_panel_rect(pos: egui::Pos2, selected_depth: Option<usize>) -> egui::Rect {
    egui::Rect::from_min_size(pos, egui::vec2(STYLE_PANEL_WIDTH, style_panel_height(selected_depth)))
}

fn draw_search_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    active: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered || active {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 3141, &rough_opts);
    let stroke_width = if hovered || active { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Magnifying glass icon
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);
    let icon_r = 6.0;
    let n_pts = 18;
    let mut circle_pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        circle_pts.push(egui::pos2(cx - 1.5 + icon_r * angle.cos(), cy - 1.5 + icon_r * angle.sin()));
    }
    painter.add(PathShape::line(circle_pts, egui::Stroke::new(1.5, icon_color)));
    let handle_angle: f32 = std::f32::consts::FRAC_PI_4;
    let handle_start = egui::pos2(
        cx - 1.5 + icon_r * handle_angle.cos(),
        cy - 1.5 + icon_r * handle_angle.sin(),
    );
    let handle_end = egui::pos2(
        cx - 1.5 + (icon_r + 5.0) * handle_angle.cos(),
        cy - 1.5 + (icon_r + 5.0) * handle_angle.sin(),
    );
    painter.line_segment([handle_start, handle_end], egui::Stroke::new(2.0, icon_color));
}

fn draw_style_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    _panel_open: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    // Background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 1234, &rough_opts);
    let stroke_width = if hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Draw a simple palette icon: a circle with colored dots
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);

    // Palette circle outline (wobbled)
    let palette_r = 9.0;
    let line_opts = RoughOptions {
        roughness: 0.6,
        max_randomness_offset: 0.8,
        bowing: 0.3,
        disable_multi_stroke: true,
        ..Default::default()
    };

    // Draw an oval/circle shape as a palette
    let n_pts = 20;
    let mut pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        pts.push(egui::pos2(
            cx + palette_r * angle.cos(),
            cy + palette_r * 0.85 * angle.sin(),
        ));
    }
    painter.add(PathShape::line(pts, egui::Stroke::new(1.2, icon_color)));

    // Colored dots inside
    let dot_r = 2.5;
    let dots = [
        (cx - 4.0, cy - 3.0, egui::Color32::from_rgb(255, 186, 194)),  // pink
        (cx + 3.0, cy - 3.0, egui::Color32::from_rgb(164, 216, 255)),  // blue
        (cx - 1.0, cy + 3.0, egui::Color32::from_rgb(176, 232, 181)),  // green
        (cx + 5.0, cy + 2.0, egui::Color32::from_rgb(255, 244, 168)),  // yellow
    ];
    for (dx, dy, color) in dots {
        painter.circle_filled(egui::pos2(dx, dy), dot_r, color);
    }
}

fn draw_style_panel(
    ui: &egui::Ui,
    pos: egui::Pos2,
    selected_depth: Option<usize>,
    config: &DepthColorConfig,
    dark_mode: bool,
) -> StyleAction {
    let painter = ui.painter();
    let panel_rect = style_panel_rect(pos, selected_depth);

    let seed: u32 = 4567;

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(3.0, 3.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, seed, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut action = StyleAction::None;

    let label_color = colors::ui_text(dark_mode);
    let muted_color = colors::ui_text_muted(dark_mode);

    // Title row
    let mut y = panel_rect.min.y + MENU_PAD_Y;
    painter.text(
        egui::pos2(panel_rect.min.x + 14.0, y + STYLE_TITLE_HEIGHT / 2.0),
        egui::Align2::LEFT_CENTER,
        "Depth Colors",
        egui::FontId::proportional(15.0),
        label_color,
    );

    // Reset button (if there are overrides)
    if config.has_overrides() {
        let reset_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.max.x - 40.0, y + 4.0),
            egui::vec2(28.0, 28.0),
        );
        let reset_hovered = pointer_pos.map_or(false, |p| reset_rect.contains(p));
        if reset_hovered {
            painter.add(RectShape::new(
                reset_rect,
                egui::CornerRadius::same(4),
                colors::hover_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // Reset icon: circular arrow (↺) as text
        painter.text(
            reset_rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{21BA}",
            egui::FontId::proportional(16.0),
            if reset_hovered { label_color } else { muted_color },
        );
        if reset_hovered && clicked {
            action = StyleAction::ResetAll;
        }
    }

    y += STYLE_TITLE_HEIGHT;

    // Divider under title
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 12.0, y),
        egui::pos2(panel_rect.max.x - 12.0, y),
        seed.wrapping_add(100),
        &RoughOptions {
            roughness: 0.6,
            max_randomness_offset: 0.8,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        },
    );
    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
    for path in div_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, div_stroke));
        }
    }

    y += 2.0;

    // Depth rows
    for depth in 0..8usize {
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.min.x + 4.0, y),
            egui::vec2(STYLE_PANEL_WIDTH - 8.0, DEPTH_ROW_HEIGHT),
        );

        let row_hovered = pointer_pos.map_or(false, |p| row_rect.contains(p));
        let is_selected = selected_depth == Some(depth);

        // Hover/selection background
        if is_selected {
            painter.add(RectShape::new(
                row_rect,
                egui::CornerRadius::same(4),
                colors::selected_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        } else if row_hovered {
            painter.add(RectShape::new(
                row_rect,
                egui::CornerRadius::same(4),
                colors::hover_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        }

        if row_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Depth number
        let arrow = if is_selected { "\u{25BC} " } else { "" };
        painter.text(
            egui::pos2(row_rect.min.x + 12.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{}{}", arrow, depth),
            egui::FontId::proportional(13.0),
            muted_color,
        );

        // Label
        painter.text(
            egui::pos2(row_rect.min.x + 48.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            DEPTH_LABELS[depth],
            egui::FontId::proportional(14.0),
            label_color,
        );

        // Color swatch (current color for this depth)
        let fill_idx = config.get_fill_index(depth);
        let fill_color = colors::depth_fill_color(fill_idx);
        let swatch_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.max.x - 32.0, row_rect.center().y - 8.0),
            egui::vec2(16.0, 16.0),
        );
        painter.rect_filled(swatch_rect, 3.0, fill_color);
        painter.rect_stroke(swatch_rect, 3.0, egui::Stroke::new(1.0, colors::border_color(dark_mode)), StrokeKind::Outside);

        // Click handler
        if row_hovered && clicked && action == StyleAction::None {
            action = StyleAction::SelectDepth(depth);
        }

        y += DEPTH_ROW_HEIGHT;

        // If this depth is selected, draw the swatch grid
        if is_selected {
            let grid_x = panel_rect.min.x + 16.0;
            let grid_y = y + 4.0;

            for row in 0..SWATCH_ROWS {
                for col in 0..SWATCH_COLS {
                    let idx = row * SWATCH_COLS + col;
                    if idx >= colors::DEPTH_FILL_COUNT {
                        break;
                    }
                    let sx = grid_x + col as f32 * (SWATCH_SIZE + SWATCH_GAP);
                    let sy = grid_y + row as f32 * (SWATCH_SIZE + SWATCH_GAP);
                    let swatch_r = egui::Rect::from_min_size(
                        egui::pos2(sx, sy),
                        egui::vec2(SWATCH_SIZE, SWATCH_SIZE),
                    );

                    let color = colors::depth_fill_color(idx);
                    painter.rect_filled(swatch_r, 4.0, color);

                    // Highlight current selection
                    let is_current = fill_idx == idx;
                    if is_current {
                        painter.rect_stroke(
                            swatch_r.expand(1.0),
                            4.0,
                            egui::Stroke::new(2.0, colors::border_color(dark_mode)),
                            StrokeKind::Outside,
                        );
                    }

                    let swatch_hovered = pointer_pos.map_or(false, |p| swatch_r.contains(p));
                    if swatch_hovered {
                        painter.rect_stroke(
                            swatch_r,
                            4.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 80)),
                            StrokeKind::Outside,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);

                        if clicked && action == StyleAction::None {
                            action = StyleAction::SetColor(depth, idx);
                        }
                    }
                }
            }

            let grid_h = SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
            y += grid_h;
        }
    }

    // Footer: explain color cycling
    let footer_color = colors::ui_text_muted(dark_mode);
    // Wobbled divider above footer
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 12.0, y + 2.0),
        egui::pos2(panel_rect.max.x - 12.0, y + 2.0),
        seed.wrapping_add(200),
        &RoughOptions {
            roughness: 0.6,
            max_randomness_offset: 0.8,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        },
    );
    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
    for path in div_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, div_stroke));
        }
    }
    painter.text(
        egui::pos2(panel_rect.min.x + 14.0, y + STYLE_FOOTER_HEIGHT / 2.0 + 2.0),
        egui::Align2::LEFT_CENTER,
        "Colors cycle every 8 levels (8=Root, 9=L1...)",
        egui::FontId::proportional(11.0),
        footer_color,
    );

    action
}

// ---------------------------------------------------------------------------
// Notes panel
// ---------------------------------------------------------------------------

const NOTES_PANEL_WIDTH: f32 = 300.0;
const NOTES_PANEL_MIN_HEIGHT: f32 = 300.0;
const NOTES_HEADER_H: f32 = 36.0;
const NOTES_PAD: f32 = 12.0;

struct NotesPanelResult {
    close: bool,
    text_focused: bool,
    navigate_to: Option<NodeId>,
    notes_changed: bool,
}

fn draw_notes_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    active: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered || active {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 7878, &rough_opts);
    let stroke_width = if hovered || active { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Document icon: small rectangle outline with three lines
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);
    let doc_x = cx - 5.0;
    let doc_y = cy - 6.0;
    let doc_w = 10.0;
    let doc_h = 12.0;

    // Outline
    painter.rect_stroke(
        egui::Rect::from_min_size(egui::pos2(doc_x, doc_y), egui::vec2(doc_w, doc_h)),
        1.0,
        egui::Stroke::new(1.2, icon_color),
        StrokeKind::Outside,
    );

    // Three horizontal lines
    for y_offset in [2.5_f32, 5.0, 7.5] {
        painter.line_segment(
            [
                egui::pos2(doc_x + 1.5, doc_y + y_offset),
                egui::pos2(doc_x + doc_w - 1.5, doc_y + y_offset),
            ],
            egui::Stroke::new(1.0, icon_color),
        );
    }
}

fn collect_nodes_with_notes(tree: &MindmapTree) -> Vec<NodeId> {
    let mut result = Vec::new();
    dfs_collect_notes(tree, tree.root, &mut result);
    result
}

fn dfs_collect_notes(tree: &MindmapTree, id: NodeId, out: &mut Vec<NodeId>) {
    if !tree.nodes[id].notes.is_empty() {
        out.push(id);
    }
    for &child in &tree.nodes[id].children {
        dfs_collect_notes(tree, child, out);
    }
}

fn draw_notes_panel(
    ui: &mut egui::Ui,
    panel_rect: egui::Rect,
    tree: &mut MindmapTree,
    edit_node: &mut Option<NodeId>,
    selection: &Selection,
    depth_color_config: &DepthColorConfig,
    saved_alpha: f32,
    dark_mode: bool,
) -> NotesPanelResult {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut close_clicked = false;
    let mut text_focused = false;
    let mut navigate_to: Option<NodeId> = None;
    let mut notes_changed = false;

    // Collect notes-bearing nodes once (only needed in browser mode)
    let notes_ids: Vec<NodeId> = if edit_node.is_none() {
        collect_nodes_with_notes(tree)
    } else {
        Vec::new()
    };

    // --- Background + border ---
    {
        let mut shapes: Vec<egui::Shape> = Vec::new();
        shapes.push(
            RectShape::new(
                panel_rect.translate(egui::vec2(3.0, 3.0)),
                egui::CornerRadius::same(8),
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            )
            .into(),
        );
        shapes.push(
            RectShape::new(
                panel_rect,
                egui::CornerRadius::same(8),
                colors::panel_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            )
            .into(),
        );
        let rough_opts = RoughOptions {
            roughness: 0.5,
            max_randomness_offset: 1.0,
            bowing: 0.5,
            ..Default::default()
        };
        let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, 9123, &rough_opts);
        let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
        for path in border_paths {
            if path.len() >= 2 {
                shapes.push(PathShape::line(path, border_stroke).into());
            }
        }
        ui.painter().extend(shapes);
    }

    // --- Close button ---
    let close_center = egui::pos2(
        panel_rect.max.x - NOTES_PAD,
        panel_rect.min.y + NOTES_HEADER_H / 2.0,
    );
    let close_btn_rect = egui::Rect::from_center_size(close_center, egui::vec2(20.0, 20.0));
    let close_hovered = pointer_pos.map_or(false, |p| close_btn_rect.contains(p));
    if close_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().circle_filled(
            close_btn_rect.center(),
            10.0,
            colors::hover_bg(dark_mode),
        );
    }
    let close_color = if close_hovered {
        colors::ui_text(dark_mode)
    } else {
        colors::ui_text_muted(dark_mode)
    };
    ui.painter().text(
        close_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(16.0),
        close_color,
    );
    if close_hovered && clicked {
        close_clicked = true;
    }

    // --- Header ---
    let header_center_y = panel_rect.min.y + NOTES_HEADER_H / 2.0;
    let header_x = panel_rect.min.x + NOTES_PAD;

    if edit_node.is_some() {
        // Edit mode: "← All Notes" link
        let back_color = egui::Color32::from_rgb(30, 136, 229);
        let back_hovered = pointer_pos.map_or(false, |p| {
            p.y >= panel_rect.min.y
                && p.y <= panel_rect.min.y + NOTES_HEADER_H
                && p.x >= header_x
                && p.x < close_btn_rect.min.x
        });
        if back_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let back_galley = ui
            .painter()
            .layout_no_wrap("← All Notes".to_string(), egui::FontId::proportional(13.0), back_color);
        let back_h = back_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x, header_center_y - back_h / 2.0),
            back_galley,
            back_color,
        );
        if back_hovered && clicked {
            *edit_node = None;
        }

        // "Saved" indicator — centered in header, fades in/out via saved_alpha
        if saved_alpha > 0.0 {
            let alpha = (saved_alpha * 255.0) as u8;
            let saved_color = egui::Color32::from_rgba_unmultiplied(0x9E, 0x9E, 0x96, alpha);
            let saved_galley = ui.painter().layout_no_wrap(
                "Saved".to_string(),
                egui::FontId::proportional(13.0),
                saved_color,
            );
            let saved_w = saved_galley.size().x;
            let saved_h = saved_galley.size().y;
            let center_x = panel_rect.center().x;
            ui.painter().galley(
                egui::pos2(center_x - saved_w / 2.0, header_center_y - saved_h / 2.0),
                saved_galley,
                saved_color,
            );
        }
    } else {
        // Browser mode: "Notes" bold + " (N)" count muted
        let count = notes_ids.len();
        let notes_galley = ui.painter().layout_no_wrap(
            "Notes".to_string(),
            egui::FontId::proportional(14.0),
            colors::ui_text(dark_mode),
        );
        let notes_w = notes_galley.size().x;
        let notes_h = notes_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x, header_center_y - notes_h / 2.0),
            notes_galley,
            colors::ui_text(dark_mode),
        );
        let count_text = format!(" ({})", count);
        let count_galley = ui.painter().layout_no_wrap(
            count_text,
            egui::FontId::proportional(12.0),
            colors::ui_text_muted(dark_mode),
        );
        let count_h = count_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x + notes_w, header_center_y - count_h / 2.0),
            count_galley,
            colors::ui_text_muted(dark_mode),
        );
    }

    // --- Wobbly divider below header ---
    let divider_y = panel_rect.min.y + NOTES_HEADER_H;
    {
        let div_paths = wobble::rough_line(
            egui::pos2(panel_rect.min.x + NOTES_PAD, divider_y),
            egui::pos2(panel_rect.max.x - NOTES_PAD, divider_y),
            9999,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.7,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
        for path in div_paths {
            if path.len() >= 2 {
                ui.painter().add(PathShape::line(path, div_stroke));
            }
        }
    }

    // --- Content area ---
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(panel_rect.min.x, divider_y + 8.0),
        egui::pos2(panel_rect.max.x, panel_rect.max.y - NOTES_PAD),
    );

    if let Some(id) = *edit_node {
        // Edit mode: label + TextEdit
        if id < tree.nodes.len() {
            let node_label = tree.nodes[id].text.clone();
            let label_max_w = content_rect.width() - NOTES_PAD * 2.0;
            let label_galley = ui.painter().layout(
                node_label,
                egui::FontId::proportional(13.0),
                colors::ui_text_muted(dark_mode),
                label_max_w,
            );
            let label_h = label_galley.size().y;
            ui.painter().galley(
                egui::pos2(content_rect.min.x + NOTES_PAD, content_rect.min.y),
                label_galley,
                colors::ui_text_muted(dark_mode),
            );

            let te_rect = egui::Rect::from_min_max(
                egui::pos2(content_rect.min.x + NOTES_PAD, content_rect.min.y + label_h + 8.0),
                egui::pos2(content_rect.max.x - NOTES_PAD, content_rect.max.y),
            );
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(te_rect));
            let te_response = child_ui.add(
                egui::TextEdit::multiline(&mut tree.nodes[id].notes)
                    .font(egui::FontId::proportional(14.0))
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .hint_text("Add notes…"),
            );
            text_focused = te_response.has_focus();
            if te_response.changed() {
                notes_changed = true;
            }
        }
    } else if notes_ids.is_empty() {
        // Empty state
        let cx = content_rect.center().x;
        let mut y = content_rect.min.y + 24.0;

        // Document icon (2.5× scaled)
        let scale = 2.5_f32;
        let doc_w = 10.0 * scale;
        let doc_h = 12.0 * scale;
        let doc_x = cx - doc_w / 2.0;
        let doc_y = y;
        let ic = colors::ui_text(dark_mode);
        let icon_color = egui::Color32::from_rgba_unmultiplied(ic.r(), ic.g(), ic.b(), 76);
        ui.painter().rect_stroke(
            egui::Rect::from_min_size(egui::pos2(doc_x, doc_y), egui::vec2(doc_w, doc_h)),
            2.0,
            egui::Stroke::new(1.5, icon_color),
            StrokeKind::Outside,
        );
        for y_frac in [2.5_f32 / 12.0, 5.0 / 12.0, 7.5 / 12.0] {
            let ly = doc_y + y_frac * doc_h;
            ui.painter().line_segment(
                [
                    egui::pos2(doc_x + 1.5 * scale, ly),
                    egui::pos2(doc_x + (10.0 - 1.5) * scale, ly),
                ],
                egui::Stroke::new(1.0, icon_color),
            );
        }
        y += doc_h + 16.0;

        let title_galley = ui.painter().layout_no_wrap(
            "No notes yet".to_string(),
            egui::FontId::proportional(15.0),
            colors::ui_text(dark_mode),
        );
        ui.painter().galley(
            egui::pos2(cx - title_galley.size().x / 2.0, y),
            title_galley.clone(),
            colors::ui_text(dark_mode),
        );
        y += title_galley.size().y + 8.0;

        let hint_text = "Right-click any node and\nchoose \"Notes\" to add one.".to_string();
        let hint_galley = ui.painter().layout(
            hint_text,
            egui::FontId::proportional(13.0),
            colors::ui_text_muted(dark_mode),
            content_rect.width() - NOTES_PAD * 2.0,
        );
        ui.painter().galley(
            egui::pos2(cx - hint_galley.size().x / 2.0, y),
            hint_galley,
            colors::ui_text_muted(dark_mode),
        );
    } else {
        // Browser list
        let dot_area = 14.0_f32;
        let pencil_area = 24.0_f32;
        let label_w = content_rect.width() - dot_area - pencil_area - NOTES_PAD * 2.0;
        let selected_id = selection.primary();
        let scroll_width = content_rect.width();

        let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        egui::ScrollArea::vertical().show(&mut scroll_ui, |ui| {
            for (entry_idx, &node_id) in notes_ids.iter().enumerate() {
                let depth = tree.depth(node_id);
                let dot_color = tree.nodes[node_id]
                    .background_color
                    .unwrap_or_else(|| colors::node_palette(depth, depth_color_config).fill);

                let node_label = tree.nodes[node_id].text.clone();
                let notes_preview = tree.nodes[node_id].notes.clone();

                // Layout label (single line, no wrap)
                let label_galley = ui.painter().layout_no_wrap(
                    node_label,
                    egui::FontId::proportional(14.0),
                    colors::ui_text(dark_mode),
                );

                // Layout preview (wraps at label_w)
                let preview_galley = ui.painter().layout(
                    notes_preview,
                    egui::FontId::proportional(13.0),
                    colors::ui_text_muted(dark_mode),
                    label_w,
                );

                // Cap preview height at 3 rows
                let max_preview_rows = 3usize;
                let row_h = preview_galley
                    .rows
                    .first()
                    .map(|r| r.rect.height())
                    .unwrap_or(16.0);
                let preview_h = if preview_galley.rows.len() > max_preview_rows {
                    row_h * max_preview_rows as f32
                } else {
                    preview_galley.size().y
                };

                let label_h = label_galley.size().y;
                let entry_h = label_h + preview_h + 16.0;

                // Allocate entry space (returns screen-space rect)
                let (entry_rect, entry_response) = ui.allocate_exact_size(
                    egui::vec2(scroll_width, entry_h),
                    egui::Sense::click(),
                );

                let ptr = ui.input(|i| i.pointer.hover_pos());
                let hovered = ptr.map_or(false, |p| entry_rect.contains(p));

                // Pencil hit rect (top-right of entry)
                let pencil_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        entry_rect.max.x - pencil_area,
                        entry_rect.min.y + (entry_h - 20.0) / 2.0,
                    ),
                    egui::vec2(20.0, 20.0),
                );
                let pencil_hovered = ptr.map_or(false, |p| pencil_rect.contains(p));

                // Hover background
                if hovered {
                    ui.painter().rect_filled(
                        entry_rect,
                        0.0,
                        colors::hover_bg(dark_mode),
                    );
                    if !pencil_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Left accent bar for currently-selected node
                if selected_id == Some(node_id) {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(entry_rect.min, egui::vec2(3.0, entry_h)),
                        0.0,
                        egui::Color32::from_rgb(30, 136, 229),
                    );
                }

                // Colored dot
                let dot_x = entry_rect.min.x + NOTES_PAD + 3.0;
                let dot_y = entry_rect.min.y + 8.0 + label_h / 2.0;
                ui.painter().circle_filled(egui::pos2(dot_x, dot_y), 3.0, dot_color);

                // Label (single line)
                let label_x = entry_rect.min.x + NOTES_PAD + dot_area;
                let label_y = entry_rect.min.y + 8.0;
                ui.painter().galley(
                    egui::pos2(label_x, label_y),
                    label_galley,
                    colors::ui_text(dark_mode),
                );

                // Preview (clipped to 3 lines)
                let preview_y = label_y + label_h + 2.0;
                let clip_rect = egui::Rect::from_min_size(
                    egui::pos2(label_x, preview_y),
                    egui::vec2(label_w, preview_h),
                );
                ui.painter().with_clip_rect(clip_rect).galley(
                    egui::pos2(label_x, preview_y),
                    preview_galley,
                    colors::ui_text_muted(dark_mode),
                );

                // Pencil icon
                let pencil_alpha: u8 = if pencil_hovered { 204 } else { 102 };
                let pc = colors::ui_text(dark_mode);
                ui.painter().text(
                    pencil_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✏",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), pencil_alpha),
                );
                if pencil_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Click handling
                if entry_response.clicked() {
                    if pencil_hovered {
                        *edit_node = Some(node_id);
                    } else {
                        navigate_to = Some(node_id);
                    }
                }

                // Wobbly divider between entries
                if entry_idx + 1 < notes_ids.len() {
                    let div_y = entry_rect.max.y;
                    let div_paths = wobble::rough_line(
                        egui::pos2(entry_rect.min.x + NOTES_PAD, div_y),
                        egui::pos2(entry_rect.max.x - NOTES_PAD, div_y),
                        9999u32.wrapping_add(entry_idx as u32).wrapping_mul(1234567),
                        &RoughOptions {
                            roughness: 0.4,
                            max_randomness_offset: 0.7,
                            bowing: 0.3,
                            disable_multi_stroke: true,
                            ..Default::default()
                        },
                    );
                    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
                    for path in div_paths {
                        if path.len() >= 2 {
                            ui.painter().add(PathShape::line(path, div_stroke));
                        }
                    }
                }
            }
        });
    }

    NotesPanelResult {
        close: close_clicked,
        text_focused,
        navigate_to,
        notes_changed,
    }
}

// ---------------------------------------------------------------------------
// Welcome screen
// ---------------------------------------------------------------------------

enum WelcomeAction {
    None,
    NewMap,
    OpenFile,
    OpenRecentFile(usize),
}

fn draw_welcome_screen(ui: &egui::Ui, screen_rect: egui::Rect, color_config: &DepthColorConfig, recent_files: &[PathBuf]) -> WelcomeAction {
    let painter = ui.painter();
    let cx = screen_rect.center().x;

    // Position content group at ~35% vertical height (shift up to leave room for recent files)
    let group_top = screen_rect.min.y + screen_rect.height() * 0.35 - 95.0;
    let mut y = group_top;

    // Title
    let title_galley = painter.layout_no_wrap(
        "Mindmap".to_string(),
        egui::FontId::proportional(36.0),
        egui::Color32::from_rgb(30, 30, 30),
    );
    painter.galley(
        egui::pos2(cx - title_galley.size().x / 2.0, y),
        title_galley.clone(),
        egui::Color32::from_rgb(30, 30, 30),
    );
    y += title_galley.size().y + 20.0;

    // Subtitle
    let sub_galley = painter.layout_no_wrap(
        "Create or open a mind map".to_string(),
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(107, 107, 107),
    );
    painter.galley(
        egui::pos2(cx - sub_galley.size().x / 2.0, y),
        sub_galley.clone(),
        egui::Color32::from_rgb(107, 107, 107),
    );
    y += sub_galley.size().y + 40.0;

    // Button dimensions
    let btn_w = 240.0;
    let btn_h = 56.0;
    let btn_gap = 32.0;
    let total_w = btn_w * 2.0 + btn_gap;

    let new_rect = egui::Rect::from_min_size(
        egui::pos2(cx - total_w / 2.0, y),
        egui::vec2(btn_w, btn_h),
    );
    let open_rect = egui::Rect::from_min_size(
        egui::pos2(cx - total_w / 2.0 + btn_w + btn_gap, y),
        egui::vec2(btn_w, btn_h),
    );

    // Hover detection
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let new_hovered = pointer_pos.map_or(false, |p| new_rect.contains(p));
    let open_hovered = pointer_pos.map_or(false, |p| open_rect.contains(p));

    // Draw buttons (use depth-1 and depth-2 palette colors)
    let btn_fill_1 = colors::node_palette(1, color_config).fill;
    let btn_fill_2 = colors::node_palette(2, color_config).fill;
    draw_welcome_button(painter, new_rect, "New Mind Map", btn_fill_1, new_hovered, 42);
    draw_welcome_button(painter, open_rect, "Open Existing File", btn_fill_2, open_hovered, 77);

    y += btn_h + 24.0;

    // Hint text
    let hint_galley = painter.layout_no_wrap(
        "or drag and drop a .mm file".to_string(),
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(155, 155, 155),
    );
    painter.galley(
        egui::pos2(cx - hint_galley.size().x / 2.0, y),
        hint_galley.clone(),
        egui::Color32::from_rgb(155, 155, 155),
    );
    y += hint_galley.size().y;

    // Recent files section
    let mut recent_hovered: Option<usize> = None;
    let section_w = 512.0;
    let row_h = 48.0;
    let max_recent = 5;
    let entries: Vec<&PathBuf> = recent_files.iter().take(max_recent).collect();

    if !entries.is_empty() {
        y += 32.0;

        // Wobbly divider
        let div_x0 = cx - section_w / 2.0 + (section_w - 280.0) / 2.0;
        let div_x1 = div_x0 + 280.0;
        let div_paths = wobble::rough_line(
            egui::pos2(div_x0, y),
            egui::pos2(div_x1, y),
            12345,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.8,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0xD0, 0xC8, 0xC0));
        for path in div_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, div_stroke));
            }
        }
        y += 24.0;

        // "Recent" label
        let recent_label = painter.layout_no_wrap(
            "Recent".to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
        );
        painter.galley(
            egui::pos2(cx - recent_label.size().x / 2.0, y),
            recent_label.clone(),
            egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
        );
        y += recent_label.size().y + 12.0;

        // Each recent file row
        let row_x = cx - section_w / 2.0;
        for (idx, path) in entries.iter().enumerate() {
            let row_rect = egui::Rect::from_min_size(
                egui::pos2(row_x, y),
                egui::vec2(section_w, row_h),
            );
            let hovered = pointer_pos.map_or(false, |p| row_rect.contains(p));
            if hovered {
                recent_hovered = Some(idx);
                painter.rect_filled(
                    row_rect,
                    egui::CornerRadius::same(6),
                    egui::Color32::from_rgb(0xF0, 0xED, 0xE8),
                );
            }

            // Filename
            let filename = path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name_galley = painter.layout_no_wrap(
                filename,
                egui::FontId::proportional(15.0),
                egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
            );
            let text_x = row_x + 12.0;
            let name_y = y + (row_h / 2.0) - name_galley.size().y - 1.0;
            painter.galley(
                egui::pos2(text_x, name_y),
                name_galley.clone(),
                egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
            );

            // Parent dir
            let parent_str = path.parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parent_max_w = section_w - 24.0;
            let parent_galley = painter.layout(
                parent_str,
                egui::FontId::proportional(12.0),
                egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
                parent_max_w,
            );
            let parent_y = y + (row_h / 2.0) + 1.0;
            painter.galley(
                egui::pos2(text_x, parent_y),
                parent_galley,
                egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
            );

            y += row_h;
        }
    }

    // Cursor feedback
    if new_hovered || open_hovered || recent_hovered.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Click detection
    if ui.input(|i| i.pointer.primary_clicked()) {
        if new_hovered {
            return WelcomeAction::NewMap;
        } else if open_hovered {
            return WelcomeAction::OpenFile;
        } else if let Some(idx) = recent_hovered {
            return WelcomeAction::OpenRecentFile(idx);
        }
    }

    WelcomeAction::None
}

fn draw_welcome_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    fill_color: egui::Color32,
    hovered: bool,
    seed: u32,
) {
    let rounding = 12.0;
    let stroke_color = egui::Color32::from_rgb(30, 30, 30);
    let stroke_width = if hovered { 2.0 } else { 1.5 };
    let hatch_stroke_width = if hovered { 2.5 } else { 2.0 };

    // 1. White background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        egui::Color32::WHITE,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // 2. Hachure fill
    let hatch_opts = RoughOptions {
        roughness: 0.8,
        max_randomness_offset: 1.5,
        bowing: 1.0,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let hatch_paths = wobble::hachure_fill_rect(
        rect,
        -41.0,
        5.0,
        seed.wrapping_mul(2654435761).wrapping_add(5555),
        &hatch_opts,
    );
    let hatch_stroke = egui::Stroke::new(hatch_stroke_width, fill_color);
    for path in hatch_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, hatch_stroke));
        }
    }

    // 3. Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, seed, &rough_opts);
    let border_stroke = egui::Stroke::new(stroke_width, stroke_color);
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // 4. Centered label
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(30, 30, 30),
    );
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        egui::Color32::from_rgb(30, 30, 30),
    );
}

// ---------------------------------------------------------------------------
// Search bar
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum SearchBarAction {
    None,
    Close,
    Next,
    Prev,
    ZoomTo,
    ReplaceOne,
    ReplaceAll,
}

fn draw_search_bar(
    ui: &mut egui::Ui,
    search: &mut SearchState,
    screen_rect: egui::Rect,
    dark_mode: bool,
) -> SearchBarAction {
    let mut action = SearchBarAction::None;

    // Bar dimensions
    let caret_w = 24.0;
    let caret_gap = 4.0;
    let bar_w = 350.0;
    let row_h = 36.0;
    let bar_h = if search.replace_active { row_h * 2.0 + 4.0 } else { row_h };
    // Center the bar + caret together
    let total_w = caret_w + caret_gap + bar_w;
    let bar_x = screen_rect.center().x - total_w / 2.0 + caret_w + caret_gap;
    let bar_y = screen_rect.min.y + 12.0;
    let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h));

    // Caret button (sits to the left of bar, vertically centered on the first row)
    let caret_rect = egui::Rect::from_min_size(
        egui::pos2(bar_x - caret_gap - caret_w, bar_y),
        egui::vec2(caret_w, row_h),
    );
    let pointer_pos_early = ui.input(|i| i.pointer.hover_pos());
    let caret_hovered = pointer_pos_early.map_or(false, |p| caret_rect.contains(p));
    let caret_clicked = caret_hovered && ui.input(|i| i.pointer.primary_clicked());
    if caret_clicked {
        search.replace_active = !search.replace_active;
    }
    if caret_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Draw caret button background + border
    {
        let painter = ui.painter();
        let caret_bg = if caret_hovered || search.replace_active {
            colors::hover_bg(dark_mode)
        } else if dark_mode {
            egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
        } else {
            egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
        };
        painter.add(RectShape::new(
            caret_rect,
            egui::CornerRadius::same(8),
            caret_bg,
            egui::Stroke::NONE,
            StrokeKind::Outside,
        ));
        let caret_border = wobble::rough_rounded_rect(caret_rect, 8.0, 4444, &RoughOptions {
            roughness: 0.5,
            max_randomness_offset: 1.0,
            bowing: 0.5,
            ..Default::default()
        });
        let caret_stroke = egui::Stroke::new(1.0, colors::border_color(dark_mode));
        for path in caret_border {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, caret_stroke));
            }
        }
        // Draw ▶ or ▼ symbol
        let caret_char = if search.replace_active { "▼" } else { "▶" };
        painter.text(
            caret_rect.center(),
            egui::Align2::CENTER_CENTER,
            caret_char,
            egui::FontId::proportional(10.0),
            colors::ui_text_muted(dark_mode),
        );
    }

    // Pre-compute shapes to add (avoids holding painter borrow across new_child)
    let mut shapes: Vec<egui::Shape> = Vec::new();

    // Shadow
    let shadow_rect = bar_rect.translate(egui::vec2(3.0, 3.0));
    shapes.push(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ).into());

    // Background
    shapes.push(RectShape::new(
        bar_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ).into());

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(bar_rect, 8.0, 5555, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            shapes.push(PathShape::line(path, border_stroke).into());
        }
    }

    // Magnifying glass icon (always in first row)
    let icon_cx = bar_rect.min.x + 12.0 + 7.0;
    let icon_cy = bar_rect.min.y + row_h / 2.0;
    let icon_color = egui::Color32::from_rgb(150, 150, 150);
    let icon_r = 5.0;
    let n_pts = 16;
    let mut circle_pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        circle_pts.push(egui::pos2(
            icon_cx + icon_r * angle.cos(),
            icon_cy + icon_r * angle.sin(),
        ));
    }
    shapes.push(PathShape::line(circle_pts, egui::Stroke::new(1.5, icon_color)).into());
    // Handle
    let handle_angle: f32 = std::f32::consts::FRAC_PI_4;
    let handle_start = egui::pos2(
        icon_cx + icon_r * handle_angle.cos(),
        icon_cy + icon_r * handle_angle.sin(),
    );
    let handle_end = egui::pos2(
        icon_cx + (icon_r + 4.0) * handle_angle.cos(),
        icon_cy + (icon_r + 4.0) * handle_angle.sin(),
    );
    shapes.push(egui::Shape::line_segment([handle_start, handle_end], egui::Stroke::new(1.5, icon_color)));

    // Add all pre-computed shapes
    ui.painter().extend(shapes);

    // TextEdit input area (first row only)
    let input_x = bar_rect.min.x + 12.0 + 14.0 + 8.0;
    let input_w = bar_w - 12.0 - 14.0 - 8.0 - 8.0 - 60.0 - 8.0 - 20.0 - 12.0;
    let input_rect = egui::Rect::from_min_size(
        egui::pos2(input_x, bar_rect.min.y + 4.0),
        egui::vec2(input_w, row_h - 8.0),
    );

    // TextEdit for search input (requires mutable borrow of ui)
    let text_edit_id = egui::Id::new("search_text_edit");
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(input_rect));
    let te_response = child_ui.add(
        egui::TextEdit::singleline(&mut search.query)
            .font(egui::FontId::proportional(14.0))
            .text_color(colors::ui_text(dark_mode))
            .frame(false)
            .hint_text(egui::RichText::new("Search nodes...").color(colors::ui_text_muted(dark_mode)))
            .desired_width(input_w)
            .id(text_edit_id),
    );
    te_response.request_focus();

    // Select all text if Ctrl+F was pressed while search was already open
    if search.select_all_pending {
        search.select_all_pending = false;
        let mut state = egui::TextEdit::load_state(ui.ctx(), text_edit_id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(search.query.len()),
        )));
        egui::TextEdit::store_state(ui.ctx(), text_edit_id, state);
    }

    // If TextEdit lost focus (Escape pressed inside it), close the search bar
    if te_response.lost_focus() {
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape_pressed {
            return SearchBarAction::Close;
        }
        // Re-focus if lost focus for any other reason (e.g. clicking elsewhere)
        te_response.request_focus();
    }

    // Handle Tab/Shift+Tab for next/prev, Enter to zoom to current match
    let keys = ui.input(|i| i.events.clone());
    for event in &keys {
        match event {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                modifiers,
                ..
            } => {
                if modifiers.shift {
                    action = SearchBarAction::Prev;
                } else {
                    action = SearchBarAction::Next;
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                action = SearchBarAction::ZoomTo;
            }
            _ => {}
        }
    }

    // First-row vertical center (used for X button and counter)
    let row1_cy = bar_rect.min.y + row_h / 2.0;

    // X button hit test (need pointer info before painting)
    let x_btn_size = 20.0;
    let x_btn_rect = egui::Rect::from_min_size(
        egui::pos2(bar_rect.max.x - 12.0 - x_btn_size, row1_cy - x_btn_size / 2.0),
        egui::vec2(x_btn_size, x_btn_size),
    );
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let x_hovered = pointer_pos.map_or(false, |p| x_btn_rect.contains(p));
    let x_clicked = x_hovered && ui.input(|i| i.pointer.primary_clicked());

    if x_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if x_clicked {
        action = SearchBarAction::Close;
    }

    // Now paint the counter and X button (new painter borrow)
    let painter = ui.painter();

    // Counter display
    if !search.query.is_empty() {
        let counter_x = bar_rect.max.x - 12.0 - 20.0 - 8.0 - 60.0;
        let counter_text;
        let counter_color;
        if search.matches.is_empty() {
            counter_text = "0 / 0".to_string();
            counter_color = egui::Color32::from_rgb(200, 100, 100);
        } else {
            counter_text = format!("{} / {}", search.current_index + 1, search.matches.len());
            counter_color = egui::Color32::from_rgb(150, 150, 150);
        }
        painter.text(
            egui::pos2(counter_x + 30.0, row1_cy),
            egui::Align2::CENTER_CENTER,
            counter_text,
            egui::FontId::proportional(13.0),
            counter_color,
        );
    }

    // X hover background
    if x_hovered {
        painter.circle_filled(
            x_btn_rect.center(),
            x_btn_size / 2.0,
            colors::hover_bg(dark_mode),
        );
    }

    // X lines (wobbled)
    let x_color = if x_hovered {
        egui::Color32::from_rgb(80, 80, 80)
    } else {
        egui::Color32::from_rgb(150, 150, 150)
    };
    let x_cx = x_btn_rect.center().x;
    let x_cy = x_btn_rect.center().y;
    let x_half = 4.0;
    let x_stroke = egui::Stroke::new(1.5, x_color);
    let x_line_opts = RoughOptions {
        roughness: 0.6,
        max_randomness_offset: 0.8,
        bowing: 0.3,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let x1_paths = wobble::rough_line(
        egui::pos2(x_cx - x_half, x_cy - x_half),
        egui::pos2(x_cx + x_half, x_cy + x_half),
        6666,
        &x_line_opts,
    );
    let x2_paths = wobble::rough_line(
        egui::pos2(x_cx + x_half, x_cy - x_half),
        egui::pos2(x_cx - x_half, x_cy + x_half),
        6677,
        &x_line_opts,
    );
    for path in x1_paths.into_iter().chain(x2_paths.into_iter()) {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, x_stroke));
        }
    }

    // --- Replace row (drawn when replace_active) ---
    if search.replace_active {
        let row2_y = bar_rect.min.y + row_h + 4.0;
        let row2_cy = row2_y + row_h / 2.0;

        // Divider line between rows
        let div_paths = wobble::rough_line(
            egui::pos2(bar_rect.min.x + 12.0, bar_rect.min.y + row_h + 2.0),
            egui::pos2(bar_rect.max.x - 12.0, bar_rect.min.y + row_h + 2.0),
            7777,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.6,
                bowing: 0.2,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
        for path in div_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, div_stroke));
            }
        }

        // Replace icon (arrows ↔ style)
        painter.text(
            egui::pos2(bar_rect.min.x + 12.0 + 7.0, row2_cy),
            egui::Align2::CENTER_CENTER,
            "↔",
            egui::FontId::proportional(13.0),
            colors::ui_text_muted(dark_mode),
        );

        // Replace text input
        let repl_input_x = bar_rect.min.x + 12.0 + 14.0 + 8.0;
        let btn_w = 60.0;
        let btn_gap = 4.0;
        let repl_input_w = bar_w - 12.0 - 14.0 - 8.0 - 8.0 - btn_w * 2.0 - btn_gap - 8.0;
        let repl_rect = egui::Rect::from_min_size(
            egui::pos2(repl_input_x, row2_y + 4.0),
            egui::vec2(repl_input_w, row_h - 8.0),
        );
        let mut child2 = ui.new_child(egui::UiBuilder::new().max_rect(repl_rect));
        child2.add(
            egui::TextEdit::singleline(&mut search.replace_text)
                .font(egui::FontId::proportional(14.0))
                .text_color(colors::ui_text(dark_mode))
                .frame(false)
                .hint_text(egui::RichText::new("Replace with...").color(colors::ui_text_muted(dark_mode)))
                .desired_width(repl_input_w),
        );

        // "Replace" button
        let btn1_x = bar_rect.max.x - 8.0 - btn_w * 2.0 - btn_gap;
        let btn1_rect = egui::Rect::from_min_size(
            egui::pos2(btn1_x, row2_y + 4.0),
            egui::vec2(btn_w, row_h - 8.0),
        );
        let btn1_hovered = pointer_pos.map_or(false, |p| btn1_rect.contains(p));
        let btn1_clicked = btn1_hovered && ui.input(|i| i.pointer.primary_clicked());

        // "All" button
        let btn2_x = btn1_x + btn_w + btn_gap;
        let btn2_rect = egui::Rect::from_min_size(
            egui::pos2(btn2_x, row2_y + 4.0),
            egui::vec2(btn_w, row_h - 8.0),
        );
        let btn2_hovered = pointer_pos.map_or(false, |p| btn2_rect.contains(p));
        let btn2_clicked = btn2_hovered && ui.input(|i| i.pointer.primary_clicked());

        if btn1_hovered || btn2_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let painter2 = ui.painter();
        for (btn_rect, label, hovered) in &[
            (btn1_rect, "Replace", btn1_hovered),
            (btn2_rect, "All", btn2_hovered),
        ] {
            let seed = if *label == "Replace" { 8881u32 } else { 8882u32 };
            let bg = if *hovered {
                colors::selected_bg(dark_mode)
            } else {
                colors::hover_bg(dark_mode)
            };
            painter2.add(RectShape::new(
                *btn_rect,
                egui::CornerRadius::same(5),
                bg,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            let btn_border_paths = wobble::rough_rounded_rect(*btn_rect, 5.0, seed, &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.8,
                bowing: 0.3,
                ..Default::default()
            });
            let btn_stroke = egui::Stroke::new(1.0, colors::border_color(dark_mode));
            for path in btn_border_paths {
                if path.len() >= 2 {
                    painter2.add(PathShape::line(path, btn_stroke));
                }
            }
            painter2.text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(13.0),
                colors::ui_text(dark_mode),
            );
        }

        if btn1_clicked && action == SearchBarAction::None {
            action = SearchBarAction::ReplaceOne;
        } else if btn2_clicked && action == SearchBarAction::None {
            action = SearchBarAction::ReplaceAll;
        }
    }

    action
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Zoom controls
// ---------------------------------------------------------------------------

fn draw_zoom_controls(
    painter: &egui::Painter,
    minus_rect: egui::Rect,
    zoom_display_rect: egui::Rect,
    plus_rect: egui::Rect,
    zoom_pct: i32,
    minus_hovered: bool,
    zoom_hovered: bool,
    plus_hovered: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };

    // Draw a single toolbar-style button
    let draw_btn = |painter: &egui::Painter, rect: egui::Rect, hovered: bool, label: &str, seed: u32| {
        let bg = if hovered {
            colors::hover_bg(dark_mode)
        } else if dark_mode {
            egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
        } else {
            egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
        };
        painter.add(RectShape::new(
            rect,
            egui::CornerRadius::same(rounding as u8),
            bg,
            egui::Stroke::NONE,
            StrokeKind::Outside,
        ));
        let border_paths = wobble::rough_rounded_rect(rect, rounding, seed, &rough_opts);
        let stroke_w = if hovered { 1.5 } else { 1.0 };
        let border_stroke = egui::Stroke::new(stroke_w, colors::border_color(dark_mode));
        for path in border_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, border_stroke));
            }
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            colors::ui_text(dark_mode),
        );
    };

    draw_btn(painter, minus_rect, minus_hovered, "−", 7001);
    draw_btn(painter, plus_rect, plus_hovered, "+", 7002);

    // Zoom display (clickable label)
    let display_label = format!("{}%", zoom_pct);
    let bg = if zoom_hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };
    painter.add(RectShape::new(
        zoom_display_rect,
        egui::CornerRadius::same(rounding as u8),
        bg,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));
    let border_paths = wobble::rough_rounded_rect(zoom_display_rect, rounding, 7003, &rough_opts);
    let stroke_w = if zoom_hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_w, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }
    painter.text(
        zoom_display_rect.center(),
        egui::Align2::CENTER_CENTER,
        &display_label,
        egui::FontId::proportional(12.0),
        colors::ui_text(dark_mode),
    );
}

// ---------------------------------------------------------------------------
// Minimap
// ---------------------------------------------------------------------------

fn compute_all_nodes_bounds(tree: &MindmapTree) -> egui::Rect {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut found = false;
    for node in &tree.nodes {
        if node.text.is_empty() && node.id != tree.root {
            continue;
        }
        let hw = node.layout_size.x / 2.0;
        let hh = node.layout_size.y / 2.0;
        min_x = min_x.min(node.layout_pos.x - hw);
        max_x = max_x.max(node.layout_pos.x + hw);
        min_y = min_y.min(node.layout_pos.y - hh);
        max_y = max_y.max(node.layout_pos.y + hh);
        found = true;
    }
    if found {
        egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
    } else {
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO)
    }
}

fn draw_minimap(
    painter: &egui::Painter,
    tree: &MindmapTree,
    viewport: &Viewport,
    screen_rect: egui::Rect,
    color_config: &DepthColorConfig,
    dark_mode: bool,
) -> egui::Rect {
    let mm_w = 200.0_f32;
    let mm_h = 120.0_f32;
    let status_bar_h = 28.0_f32;
    let margin = 8.0_f32;
    let minimap_rect = egui::Rect::from_min_size(
        egui::pos2(
            screen_rect.max.x - mm_w - margin,
            screen_rect.max.y - status_bar_h - margin - mm_h,
        ),
        egui::vec2(mm_w, mm_h),
    );

    let bounds = compute_all_nodes_bounds(tree);
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return minimap_rect;
    }

    // Drop shadow
    painter.add(RectShape::new(
        minimap_rect.translate(egui::vec2(3.0, 4.0)),
        egui::CornerRadius::same(6),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 35),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Panel bg — slightly off-white to contrast with the canvas
    let panel_bg = if dark_mode {
        egui::Color32::from_rgba_premultiplied(45, 45, 50, 245)
    } else {
        egui::Color32::from_rgba_premultiplied(242, 240, 236, 245)
    };
    painter.add(RectShape::new(
        minimap_rect,
        egui::CornerRadius::same(6),
        panel_bg,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Border — a bit thicker/darker for visibility
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(minimap_rect, 6.0, 8765, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Scale to fit bounds into minimap with centering
    let scale = (mm_w / bounds.width()).min(mm_h / bounds.height());
    let scaled_w = bounds.width() * scale;
    let scaled_h = bounds.height() * scale;
    let offset_x = (mm_w - scaled_w) / 2.0;
    let offset_y = (mm_h - scaled_h) / 2.0;

    let canvas_to_mm = |cp: egui::Pos2| -> egui::Pos2 {
        egui::pos2(
            minimap_rect.min.x + offset_x + (cp.x - bounds.min.x) * scale,
            minimap_rect.min.y + offset_y + (cp.y - bounds.min.y) * scale,
        )
    };

    // Draw nodes as tiny rects
    for node in &tree.nodes {
        if node.text.is_empty() && node.id != tree.root {
            continue;
        }
        let depth = node.depth(&tree.nodes);
        let palette = colors::node_palette_themed(depth, dark_mode, color_config);
        let node_min = canvas_to_mm(egui::pos2(
            node.layout_pos.x - node.layout_size.x / 2.0,
            node.layout_pos.y - node.layout_size.y / 2.0,
        ));
        let node_max = canvas_to_mm(egui::pos2(
            node.layout_pos.x + node.layout_size.x / 2.0,
            node.layout_pos.y + node.layout_size.y / 2.0,
        ));
        let w = (node_max.x - node_min.x).max(2.0);
        let h = (node_max.y - node_min.y).max(1.5);
        let nr = egui::Rect::from_min_size(node_min, egui::vec2(w, h));
        // Clip to minimap
        if minimap_rect.intersects(nr) {
            let f = palette.fill;
            let mm_fill = if dark_mode {
                // Dark mode fills are already dark — lighten them so they show against the dark panel
                egui::Color32::from_rgb(
                    (f.r() as u32 * 5 / 2).min(255) as u8,
                    (f.g() as u32 * 5 / 2).min(255) as u8,
                    (f.b() as u32 * 5 / 2).min(255) as u8,
                )
            } else {
                // Light mode: darken the pastels to ~55% so they read clearly
                egui::Color32::from_rgb(
                    (f.r() as f32 * 0.55) as u8,
                    (f.g() as f32 * 0.55) as u8,
                    (f.b() as f32 * 0.55) as u8,
                )
            };
            painter.add(RectShape::new(
                nr,
                egui::CornerRadius::same(1),
                mm_fill,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        }
    }

    // Draw viewport indicator
    let vis = viewport.canvas_visible_rect(screen_rect);
    let vp_min = canvas_to_mm(vis.min);
    let vp_max = canvas_to_mm(vis.max);
    let vp_rect = egui::Rect::from_min_max(
        egui::pos2(vp_min.x.max(minimap_rect.min.x), vp_min.y.max(minimap_rect.min.y)),
        egui::pos2(vp_max.x.min(minimap_rect.max.x), vp_max.y.min(minimap_rect.max.y)),
    );
    if vp_rect.width() > 0.0 && vp_rect.height() > 0.0 {
        painter.add(RectShape::new(
            vp_rect,
            egui::CornerRadius::same(2),
            egui::Color32::from_rgba_premultiplied(30, 136, 229, 30),
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(30, 136, 229, 160)),
            StrokeKind::Outside,
        ));
    }

    minimap_rect
}

// ---------------------------------------------------------------------------
// Link edit bar
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum LinkEditAction {
    None,
    Confirm,
    Cancel,
}

fn link_edit_bar_rect(screen_rect: egui::Rect) -> egui::Rect {
    let bar_w = 420.0;
    let bar_h = 36.0;
    let bar_x = screen_rect.center().x - bar_w / 2.0;
    let bar_y = screen_rect.max.y - 28.0 - 8.0 - bar_h;
    egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h))
}

fn draw_link_edit_bar(
    ui: &mut egui::Ui,
    link_edit: &mut Option<(NodeId, String)>,
    screen_rect: egui::Rect,
    dark_mode: bool,
) -> LinkEditAction {
    let Some((_, ref mut url_text)) = link_edit else {
        return LinkEditAction::None;
    };

    let bar_rect = link_edit_bar_rect(screen_rect);

    let mut shapes: Vec<egui::Shape> = Vec::new();

    // Shadow
    shapes.push(RectShape::new(
        bar_rect.translate(egui::vec2(3.0, 3.0)),
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ).into());

    // Background
    shapes.push(RectShape::new(
        bar_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ).into());

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(bar_rect, 8.0, 6543, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            shapes.push(PathShape::line(path, border_stroke).into());
        }
    }

    for shape in shapes {
        ui.painter().add(shape);
    }

    // Hint label on the left
    ui.painter().text(
        egui::pos2(bar_rect.min.x + 10.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "URL:",
        egui::FontId::proportional(13.0),
        colors::ui_text_muted(dark_mode),
    );

    // Text input
    let te_rect = egui::Rect::from_min_max(
        egui::pos2(bar_rect.min.x + 42.0, bar_rect.min.y + 4.0),
        egui::pos2(bar_rect.max.x - 8.0, bar_rect.max.y - 4.0),
    );
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(te_rect));
    let te_response = child_ui.add(
        egui::TextEdit::singleline(url_text)
            .font(egui::FontId::proportional(13.0))
            .frame(false)
            .desired_width(f32::INFINITY)
            .hint_text("https://..."),
    );

    // Auto-focus
    if !te_response.has_focus() {
        te_response.request_focus();
    }

    // Enter = confirm, Escape = cancel
    if te_response.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            return LinkEditAction::Confirm;
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return LinkEditAction::Cancel;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return LinkEditAction::Confirm;
    }

    LinkEditAction::None
}

// Helpers
// ---------------------------------------------------------------------------

fn compute_tree_bounds(tree: &MindmapTree) -> egui::Rect {
    let visible = tree.visible_nodes();
    if visible.is_empty() {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &id in &visible {
        let node = &tree.nodes[id];
        let half_w = node.layout_size.x / 2.0;
        let half_h = node.layout_size.y / 2.0;
        min_x = min_x.min(node.layout_pos.x - half_w);
        max_x = max_x.max(node.layout_pos.x + half_w);
        min_y = min_y.min(node.layout_pos.y - half_h);
        max_y = max_y.max(node.layout_pos.y + half_h);
    }
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

/// Adjust viewport offset so that the given node is visible on screen.
fn ensure_node_visible(
    node_id: NodeId,
    viewport: &mut Viewport,
    screen_rect: egui::Rect,
    tree: &MindmapTree,
) {
    let node = &tree.nodes[node_id];
    let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
    let half_w = (node.layout_size.x / 2.0) * viewport.zoom + 40.0;
    let half_h = (node.layout_size.y / 2.0) * viewport.zoom + 40.0;

    let node_screen_rect = egui::Rect::from_center_size(
        screen_pos,
        egui::vec2(half_w * 2.0, half_h * 2.0),
    );

    let mut dx = 0.0f32;
    let mut dy = 0.0f32;

    if node_screen_rect.min.x < screen_rect.min.x {
        dx = screen_rect.min.x - node_screen_rect.min.x;
    } else if node_screen_rect.max.x > screen_rect.max.x {
        dx = screen_rect.max.x - node_screen_rect.max.x;
    }

    if node_screen_rect.min.y < screen_rect.min.y {
        dy = screen_rect.min.y - node_screen_rect.min.y;
    } else if node_screen_rect.max.y > screen_rect.max.y {
        dy = screen_rect.max.y - node_screen_rect.max.y;
    }

    if dx != 0.0 || dy != 0.0 {
        viewport.offset += egui::vec2(dx, dy);
    }
}

// ---------------------------------------------------------------------------
// Help overlay (keyboard shortcut cheatsheet)
// ---------------------------------------------------------------------------

/// Returns true if the overlay was dismissed.
fn draw_help_overlay(ui: &mut egui::Ui, screen_rect: egui::Rect, dark_mode: bool) -> bool {
    let panel_w = 500.0;
    let col_label_w = 120.0;
    let row_h = 22.0;
    let pad = 16.0;

    // Two columns of shortcuts
    let left_shortcuts: &[(&str, &str)] = &[
        ("F2", "Edit node"),
        ("Tab", "Add child"),
        ("Enter", "Add sibling"),
        ("Del", "Delete node"),
        ("Ctrl+.", "Fold / Unfold"),
        ("Ctrl+Shift+-", "Fold all"),
        ("Ctrl+Shift+=", "Unfold all"),
        ("F / Home", "Fit to screen"),
        ("Ctrl+X/C/V", "Cut / Copy / Paste"),
        ("Ctrl+Shift+N", "Notes panel"),
    ];
    let right_shortcuts: &[(&str, &str)] = &[
        ("Ctrl+Z", "Undo"),
        ("Ctrl+Shift+Z", "Redo"),
        ("Ctrl+F", "Search / Replace"),
        ("Ctrl+B", "Bold"),
        ("Ctrl+S", "Save"),
        ("Ctrl+Shift+S", "Save As"),
        ("Ctrl+O", "Open file"),
        ("Ctrl+N", "New map"),
        ("Escape", "Close panels"),
        ("?", "This help"),
    ];

    let rows = left_shortcuts.len().max(right_shortcuts.len()) as f32;
    let title_h = 36.0;
    let footer_h = 28.0;
    let panel_h = pad * 2.0 + title_h + rows * row_h + footer_h;

    let cx = screen_rect.center().x;
    let cy = screen_rect.center().y;
    let panel_rect = egui::Rect::from_center_size(
        egui::pos2(cx, cy),
        egui::vec2(panel_w, panel_h),
    );

    let painter = ui.painter();

    // Dim overlay
    painter.rect_filled(
        screen_rect,
        0.0,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 60),
    );

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(4.0, 4.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(10),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 30),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(10),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 10.0, 9999, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Title
    let mut y = panel_rect.min.y + pad;
    painter.text(
        egui::pos2(cx, y + title_h / 2.0),
        egui::Align2::CENTER_CENTER,
        "Keyboard Shortcuts",
        egui::FontId::proportional(18.0),
        colors::ui_text(dark_mode),
    );
    y += title_h;

    // Title divider
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 16.0, y),
        egui::pos2(panel_rect.max.x - 16.0, y),
        11111,
        &RoughOptions {
            roughness: 0.4,
            max_randomness_offset: 0.7,
            bowing: 0.2,
            disable_multi_stroke: true,
            ..Default::default()
        },
    );
    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
    for path in div_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, div_stroke));
        }
    }

    // Two columns layout
    let col_w = panel_w / 2.0;
    let key_color = colors::ui_text_muted(dark_mode);
    let action_color = colors::ui_text(dark_mode);

    for (col_idx, shortcuts) in [left_shortcuts, right_shortcuts].iter().enumerate() {
        let col_x = panel_rect.min.x + col_idx as f32 * col_w + 16.0;
        for (row_idx, (key, action)) in shortcuts.iter().enumerate() {
            let ry = y + row_idx as f32 * row_h + row_h / 2.0;
            painter.text(
                egui::pos2(col_x, ry),
                egui::Align2::LEFT_CENTER,
                *key,
                egui::FontId::proportional(13.0),
                key_color,
            );
            painter.text(
                egui::pos2(col_x + col_label_w, ry),
                egui::Align2::LEFT_CENTER,
                *action,
                egui::FontId::proportional(13.0),
                action_color,
            );
        }
    }

    y += rows * row_h;

    // Footer hint
    painter.text(
        egui::pos2(cx, y + footer_h / 2.0),
        egui::Align2::CENTER_CENTER,
        "Press ? or Escape to close",
        egui::FontId::proportional(12.0),
        colors::ui_text_muted(dark_mode),
    );

    // Dismiss on click outside the panel
    let clicked_outside = ui.input(|i| {
        if i.pointer.primary_clicked() {
            i.pointer.hover_pos().map_or(true, |p| !panel_rect.contains(p))
        } else {
            false
        }
    });
    clicked_outside
}

fn draw_status_bar(
    ui: &egui::Ui,
    screen_rect: egui::Rect,
    tree: &MindmapTree,
    file_path: &Option<PathBuf>,
    viewport: &Viewport,
    dirty: bool,
) {
    let painter = ui.painter();
    let bar_height = 28.0;
    let bar_rect = egui::Rect::from_min_size(
        egui::pos2(screen_rect.min.x, screen_rect.max.y - bar_height),
        egui::vec2(screen_rect.width(), bar_height),
    );

    painter.rect_filled(
        bar_rect,
        0.0,
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 220),
    );

    let file_name = file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("Untitled");

    let node_count = tree.visible_nodes().len();
    let zoom_pct = (viewport.zoom * 100.0) as i32;

    let dirty_marker = if dirty { "● " } else { "" };
    let status = format!("  {}{}  |  {} nodes  |  {}%", dirty_marker, file_name, node_count, zoom_pct);

    painter.text(
        egui::pos2(bar_rect.min.x + 8.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        status,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(100, 100, 100),
    );
}

// ---------------------------------------------------------------------------
// Recent files persistence
// ---------------------------------------------------------------------------

fn recent_files_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    } else {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config"))
    };
    base.map(|d| d.join("mindmap").join("recent_files.txt"))
}

fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recent_files_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn save_recent_files(recent: &[PathBuf]) {
    let Some(path) = recent_files_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = recent
        .iter()
        .filter_map(|p| p.to_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(&path, content);
}
