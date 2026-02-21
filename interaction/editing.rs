use crate::model::{MindmapTree, NodeId, NodeState};
use crate::canvas::viewport::Viewport;
use crate::history::{Action, History};
use egui::{Rect, Ui, TextEdit, FontId, Pos2, Vec2};

pub struct EditingState {
    pub active_node: Option<NodeId>,
    pub text_buffer: String,
    original_text: String,
    pub just_started: bool,
}

impl Default for EditingState {
    fn default() -> Self {
        Self {
            active_node: None,
            text_buffer: String::new(),
            original_text: String::new(),
            just_started: false,
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
    }

    /// Draw the text edit overlay. Returns true if editing just finished (needs relayout).
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        tree: &mut MindmapTree,
        viewport: &Viewport,
        screen_rect: Rect,
        history: &mut History,
    ) -> bool {
        let node_id = match self.active_node {
            Some(id) => id,
            None => return false,
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

        // Check for Enter/Escape before drawing
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
                } if !modifiers.shift => {
                    finished = true;
                }
                _ => {}
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
                    .desired_width(text_edit_width),
            );

            // Request focus on first frame
            if self.just_started {
                response.request_focus();
                self.just_started = false;
            }

            // Lost focus means finished
            if response.lost_focus() && !cancelled {
                finished = true;
            }
        });

        if cancelled {
            // Restore original text
            tree.nodes[node_id].text = self.original_text.clone();
            tree.nodes[node_id].state = NodeState::Default;
            self.active_node = None;
            return false;
        }

        if finished {
            let new_text = self.text_buffer.clone();
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
            return true;
        }

        // Keep node text in sync while editing
        tree.nodes[node_id].text = self.text_buffer.clone();
        false
    }
}
