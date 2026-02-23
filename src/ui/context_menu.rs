use crate::model::{Clipboard, MindmapTree, NodeId, Selection};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use crate::ui::{
    DIVIDER_HEIGHT, ITEM_HEIGHT, MENU_PAD_Y, SWATCH_COLS, SWATCH_GAP, SWATCH_ROWS, SWATCH_SIZE,
};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};

pub(crate) struct ContextMenuState {
    pub pos: egui::Pos2,
    pub target_node: Option<NodeId>,
    pub color_picker_open: bool,
    pub color_picker_depth: Option<usize>,
    pub preview_color: Option<(usize, usize)>, // (depth % 8, palette_index)
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum ContextAction {
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
) -> Vec<Option<CtxMenuItem>> {
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
    let is_root = primary.is_some_and(|id| id == tree.root);
    let is_multi = selection.selected.len() > 1;
    let is_leaf = primary.is_none_or(|id| tree.nodes[id].children.is_empty());
    let is_folded = primary.is_some_and(|id| tree.nodes[id].folded);
    let has_link = primary.is_some_and(|id| tree.nodes[id].link.is_some());

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
                    let grid_h =
                        20.0 + SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
                    h += grid_h;
                }
            }
            Some(_) => h += ITEM_HEIGHT,
            None => h += DIVIDER_HEIGHT,
        }
    }
    h
}

pub(crate) fn context_menu_rect(
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_context_menu(
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
    let target_depth = state.target_node.map(|id| tree.nodes[id].cached_depth);

    let panel_rect = context_menu_rect(
        state.pos,
        on_node,
        clipboard,
        tree,
        selection,
        screen_rect,
        state.color_picker_open,
    );
    let painter = ui.painter();

    // Derive wobble seed from position
    let seed = (state.pos.x as u32)
        .wrapping_mul(31)
        .wrapping_add(state.pos.y as u32);

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
                let hovered = pointer_pos.is_some_and(|p| item_rect.contains(p));

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
                    painter.rect_stroke(
                        dot_rect,
                        3.0,
                        egui::Stroke::new(0.8, colors::border_color(dark_mode)),
                        StrokeKind::Outside,
                    );
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
                    let level_name = if depth == 0 {
                        "Root".to_string()
                    } else {
                        format!("Level {}", depth)
                    };
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
                        let node_count = visible
                            .iter()
                            .filter(|&&id| tree.nodes[id].cached_depth % 8 == depth % 8)
                            .count();
                        let level_name = if depth == 0 {
                            "Root".to_string()
                        } else {
                            format!("Level {}", depth)
                        };

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
                                if idx >= colors::DEPTH_FILL_COUNT {
                                    break;
                                }
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
                                        swatch_r.expand(1.0),
                                        4.0,
                                        egui::Stroke::new(2.0, colors::border_color(dark_mode)),
                                        StrokeKind::Outside,
                                    );
                                }

                                let swatch_hovered =
                                    pointer_pos.is_some_and(|p| swatch_r.contains(p));
                                if swatch_hovered {
                                    painter.rect_stroke(
                                        swatch_r,
                                        4.0,
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
                        let grid_h =
                            SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
                        y += grid_h;
                    }
                }
            }
            Some(menu_item) => {
                let item_rect = egui::Rect::from_min_size(
                    egui::pos2(panel_rect.min.x + 4.0, y),
                    egui::vec2(CTX_MENU_WIDTH - 8.0, ITEM_HEIGHT),
                );
                let hovered = pointer_pos.is_some_and(|p| item_rect.contains(p));

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
