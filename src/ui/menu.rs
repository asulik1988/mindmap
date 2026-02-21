use crate::style::colors;
use crate::style::wobble::{self, RoughOptions};
use crate::ui::{DIVIDER_HEIGHT, ITEM_HEIGHT, MENU_PAD_Y};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum MenuAction {
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
    CloseToWelcome,
    CloseMenu,
    OpenSearch,
    CloseSearch,
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

const MENU_WIDTH: f32 = 260.0;

struct MenuItem {
    label: &'static str,
    shortcut: &'static str,
    action: MenuAction,
}

const MENU_ITEMS: &[Option<MenuItem>] = &[
    Some(MenuItem {
        label: "New Mind Map",
        shortcut: "Ctrl+N",
        action: MenuAction::NewMap,
    }),
    Some(MenuItem {
        label: "Open File...",
        shortcut: "Ctrl+O",
        action: MenuAction::OpenFile,
    }),
    None, // divider
    Some(MenuItem {
        label: "Save",
        shortcut: "Ctrl+S",
        action: MenuAction::Save,
    }),
    Some(MenuItem {
        label: "Save As...",
        shortcut: "Ctrl+Shift+S",
        action: MenuAction::SaveAs,
    }),
    Some(MenuItem {
        label: "Export",
        shortcut: "",
        action: MenuAction::ExportSubmenu,
    }),
    None, // divider
    Some(MenuItem {
        label: "Keyboard Shortcuts",
        shortcut: "?",
        action: MenuAction::ToggleHelp,
    }),
    None, // divider
    Some(MenuItem {
        label: "Dark Mode",
        shortcut: "",
        action: MenuAction::ToggleDarkMode,
    }),
    None, // divider
    Some(MenuItem {
        label: "Close to Welcome",
        shortcut: "",
        action: MenuAction::CloseToWelcome,
    }),
    Some(MenuItem {
        label: "Quit",
        shortcut: "Ctrl+Q",
        action: MenuAction::Quit,
    }),
];

fn menu_panel_height(n_recent: usize) -> f32 {
    let mut h = MENU_PAD_Y * 2.0;
    for item in MENU_ITEMS {
        h += if item.is_some() {
            ITEM_HEIGHT
        } else {
            DIVIDER_HEIGHT
        };
    }
    // Undo + Redo items + divider after them
    h += ITEM_HEIGHT * 2.0 + DIVIDER_HEIGHT;
    if n_recent > 0 {
        h += DIVIDER_HEIGHT; // section divider
        h += n_recent as f32 * ITEM_HEIGHT;
    }
    h
}

pub(crate) fn menu_panel_rect(pos: egui::Pos2, n_recent: usize) -> egui::Rect {
    egui::Rect::from_min_size(pos, egui::vec2(MENU_WIDTH, menu_panel_height(n_recent)))
}

const SUBMENU_WIDTH: f32 = 180.0;
const EXPORT_SUBMENU_ITEMS: &[(&str, MenuAction)] = &[
    ("SVG...", MenuAction::ExportSvg),
    ("PNG...", MenuAction::ExportPng),
    ("Markdown...", MenuAction::ExportMarkdown),
    ("OPML...", MenuAction::ExportOpml),
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

pub(crate) fn draw_menu_panel(
    ui: &egui::Ui,
    pos: egui::Pos2,
    recent_files: &[std::path::PathBuf],
    can_undo: bool,
    can_redo: bool,
    dark_mode: bool,
) -> MenuAction {
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
                        let hovered =
                            enabled && pointer_pos.map_or(false, |p| item_rect.contains(p));
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
                        let item_label_color = if enabled { label_color } else { disabled_color };
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
                            if enabled {
                                shortcut_color
                            } else {
                                disabled_shortcut
                            },
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
        let show = pointer_pos.map_or(false, |p| {
            row_rect.contains(p) || sub_rect.expand(2.0).contains(p)
        });
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
