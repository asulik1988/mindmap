pub mod context_menu;
pub mod menu;
pub mod overlays;
pub mod panels;
pub mod search_viewport;
pub mod toolbar;

// Shared constants used by multiple submodules
pub(crate) const ITEM_HEIGHT: f32 = 32.0;
pub(crate) const DIVIDER_HEIGHT: f32 = 9.0;
pub(crate) const MENU_PAD_Y: f32 = 6.0;
pub(crate) const SWATCH_SIZE: f32 = 24.0;
pub(crate) const SWATCH_GAP: f32 = 4.0;
pub(crate) const SWATCH_COLS: usize = 8;
pub(crate) const SWATCH_ROWS: usize = 5;
