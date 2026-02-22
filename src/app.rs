use crate::canvas::node_renderer;
use crate::canvas::renderer::{self, NodeRects};
use crate::canvas::viewport::Viewport;
use crate::export;
use crate::history::{History, PasteEntry};
use crate::interaction::editing::{EditResult, EditingState};
use crate::interaction::input::{self, DragState};
use crate::interaction::search::SearchState;
use crate::layout::reingold_tilford;
use crate::model::{Clipboard, MindmapNode, MindmapTree, NodeId, NodeState, Selection};
use crate::style::colors::{self, DepthColorConfig};
use crate::ui::context_menu::{self, ContextAction, ContextMenuState};
use crate::ui::menu::{self, MenuAction};
use crate::ui::overlays::{self, WelcomeAction};
use crate::ui::panels::{self, LinkEditAction, StyleAction};
use crate::ui::search_viewport::{self, SearchBarAction};
use crate::ui::toolbar;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

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
    notes_suppress_close: bool,
    notes_edit_node: Option<NodeId>,
    notes_saved_at: Option<f64>,
    dark_mode: bool,
    minimap_dragging: bool,
    link_edit: Option<(NodeId, String)>,
    link_edit_suppress_close: bool,
}

// ---------------------------------------------------------------------------
// Free helper functions (avoid split-borrow issues with &mut self)
// ---------------------------------------------------------------------------

/// Measure all node sizes and recompute the tree layout.
fn measure_and_relayout(tree: &mut MindmapTree, painter: &egui::Painter) {
    node_renderer::measure_all_nodes(tree, painter);
    reingold_tilford::layout(tree);
}

/// Unfold the path to `nid` and relayout if anything changed.
fn unfold_and_relayout(tree: &mut MindmapTree, nid: NodeId, painter: &egui::Painter) {
    if tree.unfold_path_to(nid) {
        measure_and_relayout(tree, painter);
    }
}

/// Zoom and center the viewport so that `nid` fills ~25% of screen width.
fn zoom_to_node(viewport: &mut Viewport, nid: NodeId, tree: &MindmapTree, screen_rect: egui::Rect) {
    let node = &tree.nodes[nid];
    let canvas_pos = node.layout_pos;
    let target_zoom = (screen_rect.width() * 0.25 / node.layout_size.x).clamp(1.0, 3.0);
    viewport.zoom = target_zoom;
    viewport.offset = egui::vec2(-canvas_pos.x * target_zoom, -canvas_pos.y * target_zoom);
}

/// Add a child node to `parent_id`, record history, select it, and start editing.
fn create_child_and_edit(
    tree: &mut MindmapTree,
    parent_id: NodeId,
    history: &mut History,
    selection: &mut Selection,
    editing: &mut EditingState,
) -> NodeId {
    let new_id = tree.add_child(parent_id, "");
    history.push(crate::history::Action::AddNode {
        node_id: new_id,
        parent_id,
    });
    selection.select_single(new_id);
    editing.start(new_id, String::new());
    tree.nodes[new_id].state = NodeState::Editing;
    new_id
}

/// Add a sibling node after `node_id`, record history, select it, and start editing.
fn create_sibling_and_edit(
    tree: &mut MindmapTree,
    node_id: NodeId,
    history: &mut History,
    selection: &mut Selection,
    editing: &mut EditingState,
) -> NodeId {
    let new_id = tree.add_sibling(node_id, "");
    history.push(crate::history::Action::AddNode {
        node_id: new_id,
        parent_id: tree.nodes[new_id].parent.unwrap_or(tree.root),
    });
    selection.select_single(new_id);
    editing.start(new_id, String::new());
    tree.nodes[new_id].state = NodeState::Editing;
    new_id
}

/// Handle context menu action dispatch. Takes individual fields to avoid
/// double-borrowing `self` when `tree` is already split-borrowed.
#[allow(clippy::too_many_arguments)]
fn handle_context_action(
    action: ContextAction,
    tree: &mut MindmapTree,
    ui: &egui::Ui,
    screen_rect: egui::Rect,
    viewport: &mut Viewport,
    selection: &mut Selection,
    history: &mut History,
    editing: &mut EditingState,
    clipboard: &mut Clipboard,
    depth_color_config: &mut DepthColorConfig,
    notes_panel_open: &mut bool,
    notes_suppress_close: &mut bool,
    notes_edit_node: &mut Option<NodeId>,
    style_panel_open: &mut bool,
    style_selected_depth: &mut Option<usize>,
    search: &mut SearchState,
    link_edit: &mut Option<(NodeId, String)>,
    link_edit_suppress_close: &mut bool,
    needs_initial_fit: &mut bool,
) {
    let mut needs_relayout = false;
    let mut ensure_visible: Option<NodeId> = None;

    match action {
        ContextAction::OpenColorPicker => {}
        ContextAction::SetLevelColor(depth, idx) => {
            depth_color_config.set_fill_index(depth, idx);
        }
        ContextAction::AddChild => {
            if let Some(parent_id) = selection.primary() {
                let new_id = create_child_and_edit(tree, parent_id, history, selection, editing);
                needs_relayout = true;
                ensure_visible = Some(new_id);
            }
        }
        ContextAction::AddSibling => {
            if let Some(node_id) = selection.primary() {
                let new_id = create_sibling_and_edit(tree, node_id, history, selection, editing);
                needs_relayout = true;
                ensure_visible = Some(new_id);
            }
        }
        ContextAction::Edit => {
            if let Some(node_id) = selection.primary() {
                editing.start(node_id, tree.nodes[node_id].text.clone());
                tree.nodes[node_id].state = NodeState::Editing;
            }
        }
        ContextAction::ViewNotes => {
            *notes_panel_open = true;
            *notes_suppress_close = true;
            *notes_edit_node = selection.primary();
            *style_panel_open = false;
            *style_selected_depth = None;
            search.close();
        }
        ContextAction::Copy => {
            if !selection.selected.is_empty() {
                let deduped = tree.deduplicate_selection(&selection.selected);
                clipboard.clear();
                for &id in &deduped {
                    clipboard.blueprints.push(tree.clone_subtree(id));
                }
            }
        }
        ContextAction::Cut => {
            if !selection.selected.is_empty() {
                let deduped = tree.deduplicate_selection(&selection.selected);
                let has_root = deduped.contains(&tree.root);
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
                                _child_index: child_index,
                            });
                        }
                    }
                    if !batch_actions.is_empty() {
                        history.push(crate::history::Action::Batch(batch_actions));
                    }
                    if let Some(pid) = select_after {
                        selection.select_single(pid);
                        ensure_visible = Some(pid);
                    } else {
                        selection.clear();
                    }
                    needs_relayout = true;
                }
            }
        }
        ContextAction::Paste => {
            if !clipboard.is_empty() {
                if let Some(parent_id) = selection.primary() {
                    let mut entries = Vec::new();
                    let mut first_root: Option<NodeId> = None;
                    for bp in &clipboard.blueprints {
                        let (new_root, all_ids) = tree.paste_subtree(bp, parent_id);
                        if first_root.is_none() {
                            first_root = Some(new_root);
                        }
                        let saved: Vec<_> =
                            all_ids.iter().map(|&id| tree.nodes[id].clone()).collect();
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
                        ensure_visible = Some(root_id);
                    }
                    needs_relayout = true;
                }
            }
        }
        ContextAction::Delete => {
            if let Some(node_id) = selection.primary() {
                if node_id != tree.root {
                    let parent_id = tree.nodes[node_id].parent;
                    let child_index = tree.child_index(node_id).unwrap_or(0);
                    if let Some(subtree) = tree.delete_subtree(node_id) {
                        history.push(crate::history::Action::DeleteSubtree {
                            subtree,
                            parent_id: parent_id.unwrap_or(tree.root),
                            _child_index: child_index,
                        });
                        if let Some(pid) = parent_id {
                            selection.select_single(pid);
                            ensure_visible = Some(pid);
                        } else {
                            selection.clear();
                        }
                        needs_relayout = true;
                    }
                }
            }
        }
        ContextAction::ToggleFold => {
            if let Some(node_id) = selection.primary() {
                tree.toggle_fold(node_id);
                history.push(crate::history::Action::ToggleFold { node_id });
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
            *needs_initial_fit = true;
        }
        ContextAction::UnfoldAll => {
            for id in 0..tree.nodes.len() {
                tree.nodes[id].folded = false;
            }
            needs_relayout = true;
            *needs_initial_fit = true;
        }
        ContextAction::OpenLink => {
            if let Some(node_id) = selection.primary() {
                if let Some(ref url) = tree.nodes[node_id].link.clone() {
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", url.as_str()])
                        .spawn();
                }
            }
        }
        ContextAction::EditLink => {
            if let Some(node_id) = selection.primary() {
                let current = tree.nodes[node_id].link.clone().unwrap_or_default();
                *link_edit = Some((node_id, current));
                *link_edit_suppress_close = true;
            }
        }
        ContextAction::RemoveLink => {
            if let Some(node_id) = selection.primary() {
                let old_link = tree.nodes[node_id].link.clone();
                tree.nodes[node_id].link = None;
                history.push(crate::history::Action::SetLink {
                    node_id,
                    old_link,
                    new_link: None,
                });
            }
        }
        ContextAction::None => {}
    }

    if needs_relayout {
        measure_and_relayout(tree, ui.painter());
    }
    if let Some(vis_id) = ensure_visible {
        search_viewport::ensure_node_visible(vis_id, viewport, screen_rect, tree);
    }
}

/// Handle search bar interaction. Takes individual fields to avoid
/// double-borrowing `self` when `tree` is already split-borrowed.
#[allow(clippy::too_many_arguments)]
fn handle_search_bar(
    ui: &mut egui::Ui,
    tree: &mut MindmapTree,
    screen_rect: egui::Rect,
    search: &mut SearchState,
    viewport: &mut Viewport,
    selection: &mut Selection,
    history: &mut History,
    node_rects: &NodeRects,
    dark_mode: bool,
) {
    search.update_matches(tree);

    let search_result = search_viewport::draw_search_bar(ui, search, screen_rect, dark_mode);
    match search_result {
        SearchBarAction::None => {}
        SearchBarAction::Close => {
            search.close();
        }
        SearchBarAction::Next => {
            search.next();
            if let Some(nid) = search.current_match() {
                unfold_and_relayout(tree, nid, ui.painter());
                search_viewport::ensure_node_visible(nid, viewport, screen_rect, tree);
            }
        }
        SearchBarAction::Prev => {
            search.prev();
            if let Some(nid) = search.current_match() {
                unfold_and_relayout(tree, nid, ui.painter());
                search_viewport::ensure_node_visible(nid, viewport, screen_rect, tree);
            }
        }
        SearchBarAction::ZoomTo => {
            if let Some(nid) = search.current_match() {
                unfold_and_relayout(tree, nid, ui.painter());
                zoom_to_node(viewport, nid, tree, screen_rect);
                selection.select_single(nid);
                search.close();
            }
        }
        SearchBarAction::ReplaceOne => {
            if let Some(node_id) = search.current_match() {
                let old_text = tree.nodes[node_id].text.clone();
                let new_text = old_text.replace(&search.query, &search.replace_text);
                if new_text != old_text {
                    tree.nodes[node_id].text = new_text.clone();
                    history.push(crate::history::Action::EditText {
                        node_id,
                        old_text,
                        new_text,
                    });
                    measure_and_relayout(tree, ui.painter());
                }
                search.update_matches_force(tree);
                search.next();
            }
        }
        SearchBarAction::ReplaceAll => {
            let ids = search.matches.clone();
            let mut batch = vec![];
            for node_id in ids {
                let old_text = tree.nodes[node_id].text.clone();
                let new_text = old_text.replace(&search.query, &search.replace_text);
                if new_text != old_text {
                    tree.nodes[node_id].text = new_text.clone();
                    batch.push(crate::history::Action::EditText {
                        node_id,
                        old_text,
                        new_text,
                    });
                }
            }
            if !batch.is_empty() {
                history.push(crate::history::Action::Batch(batch));
                measure_and_relayout(tree, ui.painter());
            }
            search.update_matches_force(tree);
        }
    }

    // Auto-scroll to current match when query changes
    if let Some(nid) = search.current_match() {
        unfold_and_relayout(tree, nid, ui.painter());
    }

    // If user clicked a node that is a search match, jump to it
    if ui.input(|i| i.pointer.primary_clicked()) {
        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
            if let Some(node_id) = input::find_node_at(pointer, node_rects) {
                if search.matches.contains(&node_id) {
                    search.jump_to_node(node_id);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MindmapApp methods
// ---------------------------------------------------------------------------

impl MindmapApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file_arg: Option<PathBuf>) -> Self {
        // Register Excalidraw's hand-drawn font (Virgil)
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Virgil".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "assets/Virgil-Regular.ttf"
            ))),
        );
        // Set Virgil as the primary proportional font
        fonts
            .families
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
            dark_mode: cc
                .storage
                .and_then(|s| s.get_string("dark_mode"))
                .map(|v| v == "true")
                .unwrap_or(false),
            minimap_dragging: false,
            link_edit: None,
            link_edit_suppress_close: false,
        };

        if let Some(path) = file_arg {
            app.load_file(path);
        }

        app
    }

    /// Reset transient UI state shared by load_file, new_map, close_to_welcome.
    fn reset_state(&mut self) {
        self.selection = Selection::default();
        self.history = History::default();
        self.editing = EditingState::default();
        self.menu_open = false;
        self.context_menu = None;
        self.search.close();
        self.notes_panel_open = false;
        self.notes_edit_node = None;
        self.style_panel_open = false;
        self.style_selected_depth = None;
        self.link_edit = None;
    }

    fn load_file(&mut self, path: PathBuf) {
        match crate::io::freemind_read::load_mm_file(&path) {
            Ok(mut tree) => {
                reingold_tilford::layout(&mut tree);
                self.tree = Some(tree);
                self.add_recent_file(&path);
                self.file_path = Some(path);
                self.reset_state();
                self.needs_initial_fit = true;
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
        self.reset_state();
        self.needs_initial_fit = true;
    }

    fn close_to_welcome(&mut self) {
        self.tree = None;
        self.file_path = None;
        self.reset_state();
    }

    /// Handle global keyboard shortcuts. Runs before CentralPanel so no
    /// split-borrow issues -- safe to call as `self.handle_global_shortcuts(ctx)`.
    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        let global_action = ctx.input(|i| {
            for event in &i.events {
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
                        (egui::Key::Equals, m)
                            if m.ctrl && m.shift && !self.editing.is_active() =>
                        {
                            return MenuAction::UnfoldAll;
                        }
                        (egui::Key::F, m)
                            if !m.ctrl && !m.shift && !m.alt && !self.editing.is_active() =>
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
                        (egui::Key::Escape, _)
                            if self.help_open
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

        match global_action {
            MenuAction::NewMap => {
                self.new_map();
            }
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
                        self.history.push(crate::history::Action::SetBold {
                            node_id,
                            old_bold,
                            new_bold,
                        });
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
            MenuAction::ResetZoom => {
                self.viewport.zoom = 1.0;
                self.viewport.offset = egui::Vec2::ZERO;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App implementation
// ---------------------------------------------------------------------------

impl eframe::App for MindmapApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark_mode", self.dark_mode.to_string());
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Global keyboard shortcuts (before CentralPanel, no split-borrow) ---
        self.handle_global_shortcuts(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(colors::canvas_bg(self.dark_mode)))
            .show(ctx, |ui| {
                let screen_rect = ui.max_rect();

                // Fit to bounds on first frame after loading
                if self.needs_initial_fit {
                    if let Some(ref mut tree) = self.tree {
                        measure_and_relayout(tree, ui.painter());
                        let bounds = search_viewport::compute_tree_bounds(tree);
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
                    let search_match_set: HashSet<NodeId> =
                        self.search.matches.iter().copied().collect();
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
                        self.drag_state = None;
                        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                            let clicked_node = input::find_node_at(pointer, &self.node_rects);
                            if let Some(node_id) = clicked_node {
                                if !self.selection.is_selected(node_id) {
                                    self.selection.select_single(node_id);
                                }
                            }
                            let pos = egui::pos2(pointer.x.round(), pointer.y.round());
                            self.context_menu = Some(ContextMenuState {
                                pos,
                                target_node: clicked_node,
                                color_picker_open: false,
                                color_picker_depth: None,
                                preview_color: None,
                            });
                            self.menu_open = false;
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
                            i.events.iter().any(|e| {
                                matches!(
                                    e,
                                    egui::Event::Key {
                                        key: egui::Key::Escape,
                                        pressed: true,
                                        ..
                                    }
                                )
                            })
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
                                let new_id = create_sibling_and_edit(
                                    tree,
                                    node_id,
                                    &mut self.history,
                                    &mut self.selection,
                                    &mut self.editing,
                                );
                                needs_relayout = true;
                                ensure_visible = Some(new_id);
                            }
                            EditResult::CreateChild(node_id) => {
                                let new_id = create_child_and_edit(
                                    tree,
                                    node_id,
                                    &mut self.history,
                                    &mut self.selection,
                                    &mut self.editing,
                                );
                                needs_relayout = true;
                                ensure_visible = Some(new_id);
                            }
                            EditResult::DeleteEmpty(node_id) => {
                                if node_id != tree.root {
                                    let parent_id = tree.nodes[node_id].parent;
                                    let child_index = tree.child_index(node_id).unwrap_or(0);
                                    if let Some(subtree) = tree.delete_subtree(node_id) {
                                        self.history.push(crate::history::Action::DeleteSubtree {
                                            subtree,
                                            parent_id: parent_id.unwrap_or(tree.root),
                                            _child_index: child_index,
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

                        if needs_relayout {
                            measure_and_relayout(tree, ui.painter());
                        }

                        if let Some(vis_id) = ensure_visible {
                            search_viewport::ensure_node_visible(
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
                        ctx_action = context_menu::draw_context_menu(
                            ui,
                            cm,
                            &self.selection,
                            &self.clipboard,
                            tree,
                            screen_rect,
                            &self.depth_color_config,
                            self.dark_mode,
                        );

                        // Click outside context menu -> close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let panel_rect = context_menu::context_menu_rect(
                                cm.pos,
                                cm.target_node.is_some(),
                                &self.clipboard,
                                tree,
                                &self.selection,
                                screen_rect,
                                cm.color_picker_open,
                            );
                            let pointer_pos = ui.input(|i| i.pointer.hover_pos());
                            let clicked_in = pointer_pos.is_some_and(|p| panel_rect.contains(p));
                            if !clicked_in {
                                self.context_menu = None;
                            }
                        }
                    }

                    // Handle context menu action
                    if ctx_action != ContextAction::None {
                        if !matches!(ctx_action, ContextAction::OpenColorPicker) {
                            self.context_menu = None;
                        }
                        handle_context_action(
                            ctx_action,
                            tree,
                            ui,
                            screen_rect,
                            &mut self.viewport,
                            &mut self.selection,
                            &mut self.history,
                            &mut self.editing,
                            &mut self.clipboard,
                            &mut self.depth_color_config,
                            &mut self.notes_panel_open,
                            &mut self.notes_suppress_close,
                            &mut self.notes_edit_node,
                            &mut self.style_panel_open,
                            &mut self.style_selected_depth,
                            &mut self.search,
                            &mut self.link_edit,
                            &mut self.link_edit_suppress_close,
                            &mut self.needs_initial_fit,
                        );
                    }

                    // Draw minimap
                    let minimap_rect = search_viewport::draw_minimap(
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
                        let in_minimap = ptr.is_some_and(|p| minimap_rect.contains(p));

                        if in_minimap && primary_down {
                            self.minimap_dragging = true;
                        }
                        if !primary_down {
                            self.minimap_dragging = false;
                        }

                        if (in_minimap && primary_clicked) || self.minimap_dragging {
                            if let Some(p) = ptr {
                                if minimap_rect.contains(p) {
                                    let all_bounds =
                                        search_viewport::compute_all_nodes_bounds(tree);
                                    if all_bounds.width() > 0.0 && all_bounds.height() > 0.0 {
                                        let scale = (minimap_rect.width() / all_bounds.width())
                                            .min(minimap_rect.height() / all_bounds.height());
                                        let scaled_w = all_bounds.width() * scale;
                                        let scaled_h = all_bounds.height() * scale;
                                        let offset_x = (minimap_rect.width() - scaled_w) / 2.0;
                                        let offset_y = (minimap_rect.height() - scaled_h) / 2.0;

                                        let rel = p - minimap_rect.min;
                                        let canvas_x =
                                            all_bounds.min.x + (rel.x - offset_x) / scale;
                                        let canvas_y =
                                            all_bounds.min.y + (rel.y - offset_y) / scale;

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
                    panels::draw_status_bar(
                        ui,
                        screen_rect,
                        tree,
                        &self.file_path,
                        &self.viewport,
                        self.history.is_dirty(),
                    );

                    // --- Search bar ---
                    if self.search.is_active() {
                        handle_search_bar(
                            ui,
                            tree,
                            screen_rect,
                            &mut self.search,
                            &mut self.viewport,
                            &mut self.selection,
                            &mut self.history,
                            &self.node_rects,
                            self.dark_mode,
                        );
                    }

                    // --- Hamburger menu (drawn on top of everything) ---
                    let hamburger_rect = egui::Rect::from_min_size(
                        egui::pos2(screen_rect.min.x + 16.0, screen_rect.min.y + 16.0),
                        egui::vec2(36.0, 36.0),
                    );

                    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
                    let hamburger_hovered = pointer_pos.is_some_and(|p| hamburger_rect.contains(p));

                    toolbar::draw_hamburger_button(
                        ui.painter(),
                        hamburger_rect,
                        hamburger_hovered,
                        self.menu_open,
                        self.dark_mode,
                    );

                    // Menu panel
                    let mut menu_action = MenuAction::None;
                    if self.menu_open {
                        let panel_pos =
                            egui::pos2(hamburger_rect.min.x, hamburger_rect.max.y + 8.0);
                        menu_action = menu::draw_menu_panel(
                            ui,
                            panel_pos,
                            &self.recent_files,
                            self.history.can_undo(),
                            self.history.can_redo(),
                            self.dark_mode,
                        );

                        // Click outside menu + hamburger -> close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let panel_rect =
                                menu::menu_panel_rect(panel_pos, self.recent_files.len());
                            let clicked_in_menu =
                                pointer_pos.is_some_and(|p| panel_rect.contains(p));
                            let clicked_in_hamburger =
                                pointer_pos.is_some_and(|p| hamburger_rect.contains(p));
                            if !clicked_in_menu && !clicked_in_hamburger {
                                self.menu_open = false;
                            }
                        }
                    }

                    // Hamburger click toggle
                    if ui.input(|i| i.pointer.primary_clicked())
                        && hamburger_hovered
                        && menu_action == MenuAction::None
                    {
                        self.menu_open = !self.menu_open;
                        if self.menu_open {
                            self.style_panel_open = false;
                            self.style_selected_depth = None;
                            self.context_menu = None;
                            self.search.close();
                        }
                    }

                    if hamburger_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // --- Style (palette) button ---
                    let style_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(hamburger_rect.max.x + 8.0, hamburger_rect.min.y),
                        egui::vec2(36.0, 36.0),
                    );
                    let style_btn_hovered = pointer_pos.is_some_and(|p| style_btn_rect.contains(p));
                    toolbar::draw_style_button(
                        ui.painter(),
                        style_btn_rect,
                        style_btn_hovered,
                        self.style_panel_open,
                        self.dark_mode,
                    );

                    if style_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    let mut _style_panel_clicked_inside = false;
                    if ui.input(|i| i.pointer.primary_clicked())
                        && style_btn_hovered
                        && menu_action == MenuAction::None
                    {
                        self.style_panel_open = !self.style_panel_open;
                        if self.style_panel_open {
                            self.menu_open = false;
                            self.context_menu = None;
                            self.search.close();
                        } else {
                            self.style_selected_depth = None;
                        }
                    }

                    // Style panel
                    if self.style_panel_open {
                        let panel_pos =
                            egui::pos2(style_btn_rect.min.x, style_btn_rect.max.y + 8.0);
                        let panel_rect =
                            panels::style_panel_rect(panel_pos, self.style_selected_depth);

                        let result = panels::draw_style_panel(
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

                        // Click outside style panel -> close
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            let ptr = pointer_pos;
                            let in_panel = ptr.is_some_and(|p| panel_rect.contains(p));
                            let in_btn = ptr.is_some_and(|p| style_btn_rect.contains(p));
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
                    let search_btn_hovered =
                        pointer_pos.is_some_and(|p| search_btn_rect.contains(p));
                    toolbar::draw_search_button(
                        ui.painter(),
                        search_btn_rect,
                        search_btn_hovered,
                        self.search.is_active(),
                        self.dark_mode,
                    );

                    if search_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if ui.input(|i| i.pointer.primary_clicked())
                        && search_btn_hovered
                        && menu_action == MenuAction::None
                    {
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

                    // --- Notes button ---
                    let notes_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(search_btn_rect.max.x + 8.0, search_btn_rect.min.y),
                        egui::vec2(36.0, 36.0),
                    );
                    let notes_btn_hovered = pointer_pos.is_some_and(|p| notes_btn_rect.contains(p));
                    toolbar::draw_notes_button(
                        ui.painter(),
                        notes_btn_rect,
                        notes_btn_hovered,
                        self.notes_panel_open,
                        self.dark_mode,
                    );

                    if notes_btn_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if ui.input(|i| i.pointer.primary_clicked())
                        && notes_btn_hovered
                        && menu_action == MenuAction::None
                    {
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

                        let minus_hovered = pointer_pos.is_some_and(|p| minus_rect.contains(p));
                        let zoom_hovered =
                            pointer_pos.is_some_and(|p| zoom_display_rect.contains(p));
                        let plus_hovered = pointer_pos.is_some_and(|p| plus_rect.contains(p));

                        toolbar::draw_zoom_controls(
                            ui.painter(),
                            minus_rect,
                            zoom_display_rect,
                            plus_rect,
                            zoom_pct,
                            minus_hovered,
                            zoom_hovered,
                            plus_hovered,
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
                        let link_result = panels::draw_link_edit_bar(
                            ui,
                            &mut self.link_edit,
                            screen_rect,
                            self.dark_mode,
                        );
                        match link_result {
                            LinkEditAction::None => {}
                            LinkEditAction::Confirm => {
                                if let Some((node_id, new_url)) = self.link_edit.take() {
                                    let old_link = tree.nodes[node_id].link.clone();
                                    let new_link = if new_url.trim().is_empty() {
                                        None
                                    } else {
                                        Some(new_url.trim().to_string())
                                    };
                                    tree.nodes[node_id].link = new_link.clone();
                                    self.history.push(crate::history::Action::SetLink {
                                        node_id,
                                        old_link,
                                        new_link,
                                    });
                                }
                            }
                            LinkEditAction::Cancel => {
                                self.link_edit = None;
                            }
                        }
                        if !suppress && ui.input(|i| i.pointer.primary_clicked()) {
                            let bar_rect = panels::link_edit_bar_rect(screen_rect);
                            let in_bar = pointer_pos.is_some_and(|p| bar_rect.contains(p));
                            if !in_bar {
                                self.link_edit = None;
                            }
                        }
                    }

                    // --- Notes panel ---
                    if self.notes_panel_open {
                        let toolbar_bottom = hamburger_rect.max.y;
                        let panel_x = screen_rect.max.x - panels::NOTES_PANEL_WIDTH - 8.0;
                        let panel_y = toolbar_bottom + 8.0;
                        let panel_h = (screen_rect.height() - toolbar_bottom - 16.0)
                            .max(panels::NOTES_PANEL_MIN_HEIGHT);
                        let notes_panel_rect = egui::Rect::from_min_size(
                            egui::pos2(panel_x, panel_y),
                            egui::vec2(panels::NOTES_PANEL_WIDTH, panel_h),
                        );

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

                        let result = panels::draw_notes_panel(
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
                            zoom_to_node(&mut self.viewport, nav_id, tree, screen_rect);
                        }

                        let suppress = std::mem::replace(&mut self.notes_suppress_close, false);
                        if !suppress && ui.input(|i| i.pointer.primary_clicked()) {
                            let ptr = pointer_pos;
                            let in_panel = ptr.is_some_and(|p| notes_panel_rect.contains(p));
                            let in_btn = ptr.is_some_and(|p| notes_btn_rect.contains(p));
                            if !in_panel && !in_btn {
                                self.notes_panel_open = false;
                                self.notes_edit_node = None;
                            }
                        }
                    }

                    // Handle menu action (kept inline because some arms need
                    // self.tree replacement which conflicts with the tree borrow)
                    match menu_action {
                        MenuAction::NewMap => {
                            self.new_map();
                        }
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
                            input::save_file(tree, &mut self.file_path);
                            self.history.mark_clean();
                        }
                        MenuAction::SaveAs => {
                            self.menu_open = false;
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
                        MenuAction::ExportSvg => {
                            self.menu_open = false;
                            let svg = export::svg::export_svg(tree, &self.depth_color_config);
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("SVG", &["svg"])
                                .save_file()
                            {
                                let _ = std::fs::write(&path, svg);
                            }
                        }
                        MenuAction::ExportPng => {
                            self.menu_open = false;
                            if let Some(png_data) =
                                export::png::export_png(tree, &self.depth_color_config)
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PNG", &["png"])
                                    .save_file()
                                {
                                    let _ = std::fs::write(&path, png_data);
                                }
                            }
                        }
                        MenuAction::ExportMarkdown => {
                            self.menu_open = false;
                            let md = export::markdown::export_markdown(tree);
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Markdown", &["md"])
                                .save_file()
                            {
                                let _ = std::fs::write(&path, md);
                            }
                        }
                        MenuAction::ExportOpml => {
                            self.menu_open = false;
                            let opml = export::opml::export_opml(tree);
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("OPML", &["opml"])
                                .save_file()
                            {
                                let _ = std::fs::write(&path, opml);
                            }
                        }
                        MenuAction::ToggleDarkMode => {
                            self.dark_mode = !self.dark_mode;
                            self.menu_open = false;
                        }
                        MenuAction::Undo => {
                            self.menu_open = false;
                            if self.history.undo(tree) {
                                measure_and_relayout(tree, ui.painter());
                            }
                        }
                        MenuAction::Redo => {
                            self.menu_open = false;
                            if self.history.redo(tree) {
                                measure_and_relayout(tree, ui.painter());
                            }
                        }
                        MenuAction::CloseToWelcome => {
                            self.close_to_welcome();
                        }
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
                    // No file loaded -- show welcome screen
                    let action = overlays::draw_welcome_screen(
                        ui,
                        screen_rect,
                        &self.depth_color_config,
                        &self.recent_files,
                    );
                    match action {
                        WelcomeAction::None => {}
                        WelcomeAction::NewMap => {
                            self.new_map();
                        }
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
                    if !suppress && overlays::draw_help_overlay(ui, screen_rect, self.dark_mode) {
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

    // If recent_files.txt exists, load normally
    if let Ok(content) = std::fs::read_to_string(&path) {
        return content
            .lines()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
    }

    // First launch — seed example file
    seed_example_file(&path)
}

fn seed_example_file(recent_path: &std::path::Path) -> Vec<PathBuf> {
    let Some(parent) = recent_path.parent() else {
        return Vec::new();
    };
    let examples_dir = parent.join("examples");
    let _ = std::fs::create_dir_all(&examples_dir);

    let example_path = examples_dir.join("whats-for-dinner.mm");
    let content = include_str!("../examples/whats-for-dinner.mm");
    if std::fs::write(&example_path, content).is_ok() {
        let recent = vec![example_path];
        save_recent_files(&recent);
        recent
    } else {
        Vec::new()
    }
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
