use crate::canvas::viewport::Viewport;
use crate::history::{Action, History};
use crate::model::{MindmapTree, NodeId, NodeState};
use egui::{FontId, Rect, TextEdit, Ui, Vec2};

/// Signals from the edit overlay back to the app loop.
#[derive(Debug, Clone, PartialEq)]
pub enum EditResult {
    /// Nothing happened / still editing.
    None,
    /// Editing finished normally (text confirmed). Needs relayout.
    Finished,
    /// Enter pressed in edit mode → confirm + create sibling below + edit it.
    CreateSibling(NodeId),
    /// Tab pressed in edit mode → confirm + create child + edit it.
    CreateChild(NodeId),
    /// Escape on empty node → delete it.
    DeleteEmpty(NodeId),
}

pub struct EditingState {
    pub active_node: Option<NodeId>,
    pub text_buffer: String,
    original_text: String,
    pub just_started: bool,
    /// When true, the text was cleared because the user started typing on a selected node.
    /// We need to inject the initial character.
    pub replace_mode_char: Option<String>,
}

impl Default for EditingState {
    fn default() -> Self {
        Self {
            active_node: None,
            text_buffer: String::new(),
            original_text: String::new(),
            just_started: false,
            replace_mode_char: None,
        }
    }
}

impl EditingState {
    pub fn is_active(&self) -> bool {
        self.active_node.is_some()
    }

    pub fn start(&mut self, node_id: NodeId, text: String) {
        self.active_node = Some(node_id);
        self.text_buffer = text.clone();
        self.original_text = text;
        self.just_started = true;
        self.replace_mode_char = None;
    }

    /// Start editing with cleared text and an initial character (any-key-starts-editing).
    pub fn start_replace(&mut self, node_id: NodeId, original_text: String, initial_char: String) {
        self.active_node = Some(node_id);
        self.text_buffer = initial_char.clone();
        self.original_text = original_text;
        self.just_started = true;
        self.replace_mode_char = Some(initial_char);
    }

    /// Draw the text edit overlay. Returns an EditResult signaling what happened.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        tree: &mut MindmapTree,
        viewport: &Viewport,
        screen_rect: Rect,
        history: &mut History,
    ) -> EditResult {
        let node_id = match self.active_node {
            Some(id) => id,
            None => return EditResult::None,
        };

        let node = &tree.nodes[node_id];
        let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
        let depth = node.depth(&tree.nodes);
        let font_size = crate::style::colors::font_size_for_depth(depth) * viewport.zoom;

        let text_edit_width = 250.0 * viewport.zoom;
        let edit_rect = Rect::from_center_size(
            screen_pos,
            Vec2::new(text_edit_width, font_size * 2.0 + 8.0),
        );

        let mut finished = false;
        let mut cancelled = false;
        let mut enter_pressed = false;
        let mut shift_enter_pressed = false;
        let mut tab_pressed = false;

        // Skip event processing on the first frame of editing — the same
        // key event that triggered editing (Enter/Tab from canvas mode) is
        // still in the queue and would immediately cancel/confirm.
        if !self.just_started {
            let events = ui.input(|i| i.events.clone());
            for event in &events {
                match event {
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        cancelled = true;
                    }
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.shift {
                            shift_enter_pressed = true;
                        } else {
                            enter_pressed = true;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        tab_pressed = true;
                    }
                    _ => {}
                }
            }
        }

        // Draw text edit
        let area = egui::Area::new(egui::Id::new("node_editor"))
            .fixed_pos(edit_rect.min)
            .order(egui::Order::Foreground);

        area.show(ui.ctx(), |ui| {
            ui.set_min_size(edit_rect.size());
            let response = ui.add_sized(
                edit_rect.size(),
                TextEdit::singleline(&mut self.text_buffer)
                    .font(FontId::proportional(font_size))
                    .desired_width(text_edit_width)
                    .hint_text(
                        egui::RichText::new("…")
                            .color(egui::Color32::from_rgba_unmultiplied(128, 128, 120, 76)),
                    ),
            );

            // Request focus on first frame
            if self.just_started {
                response.request_focus();
                // Move cursor to end
                if let Some(mut state) = TextEdit::load_state(ui.ctx(), response.id) {
                    let ccursor = egui::text::CCursor::new(self.text_buffer.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                    state.store(ui.ctx(), response.id);
                }
                self.just_started = false;
            }

            // Lost focus means finished (unless cancelled)
            if response.lost_focus() && !cancelled && !enter_pressed && !tab_pressed {
                finished = true;
            }
        });

        // Handle Escape: if text empty, signal delete; otherwise cancel/confirm
        if cancelled {
            let current_text = self.text_buffer.trim().to_string();
            if current_text.is_empty() {
                // Empty node — signal deletion
                let nid = node_id;
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::DeleteEmpty(nid);
            } else {
                // Non-empty: confirm the edit (like a normal finish) and exit
                if current_text != self.original_text {
                    history.push(Action::EditText {
                        node_id,
                        old_text: self.original_text.clone(),
                        new_text: current_text.clone(),
                    });
                    tree.nodes[node_id].text = current_text;
                } else {
                    // Restore original
                    tree.nodes[node_id].text = self.original_text.clone();
                }
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::Finished;
            }
        }

        // Handle Tab in edit mode → confirm + create child (or delete if empty)
        if tab_pressed {
            let new_text = self.text_buffer.trim().to_string();
            if new_text.is_empty() {
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::DeleteEmpty(node_id);
            }
            if new_text != self.original_text {
                history.push(Action::EditText {
                    node_id,
                    old_text: self.original_text.clone(),
                    new_text: new_text.clone(),
                });
                tree.nodes[node_id].text = new_text;
            }
            tree.nodes[node_id].state = NodeState::Default;
            self.active_node = None;
            return EditResult::CreateChild(node_id);
        }

        // Handle Enter in edit mode → confirm + create child (or delete if empty)
        if enter_pressed {
            let new_text = self.text_buffer.trim().to_string();
            if new_text.is_empty() {
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::DeleteEmpty(node_id);
            }
            if new_text != self.original_text {
                history.push(Action::EditText {
                    node_id,
                    old_text: self.original_text.clone(),
                    new_text: new_text.clone(),
                });
                tree.nodes[node_id].text = new_text;
            }
            tree.nodes[node_id].state = NodeState::Default;
            self.active_node = None;
            return EditResult::CreateChild(node_id);
        }

        // Handle Shift+Enter in edit mode → confirm + create sibling (or delete if empty)
        if shift_enter_pressed {
            let new_text = self.text_buffer.trim().to_string();
            if new_text.is_empty() {
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::DeleteEmpty(node_id);
            }
            if new_text != self.original_text {
                history.push(Action::EditText {
                    node_id,
                    old_text: self.original_text.clone(),
                    new_text: new_text.clone(),
                });
                tree.nodes[node_id].text = new_text;
            }
            tree.nodes[node_id].state = NodeState::Default;
            self.active_node = None;
            return EditResult::CreateSibling(node_id);
        }

        // Normal finish (lost focus) — delete if empty
        if finished {
            let new_text = self.text_buffer.trim().to_string();
            if new_text.is_empty() {
                tree.nodes[node_id].state = NodeState::Default;
                self.active_node = None;
                return EditResult::DeleteEmpty(node_id);
            }
            if new_text != self.original_text {
                history.push(Action::EditText {
                    node_id,
                    old_text: self.original_text.clone(),
                    new_text: new_text.clone(),
                });
                tree.nodes[node_id].text = new_text;
            }
            tree.nodes[node_id].state = NodeState::Default;
            self.active_node = None;
            return EditResult::Finished;
        }

        // Keep node text in sync while editing
        tree.nodes[node_id].text = self.text_buffer.clone();
        EditResult::None
    }
}
