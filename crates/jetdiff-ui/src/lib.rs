//! The GPUI layer. Draws only what `jetdiff-viewmodel` computes.

mod cache;
mod code_viewer;
mod commit_list;
mod file_diff;
mod file_header;
mod file_tree;
mod help;
mod icons;
mod keystroke;
mod list;
mod notification_slot_view;
mod panel_size;
mod repo_view;
mod rollback_dialog;
mod scroll_animator;
mod settings;
mod text;
mod theme;
mod watcher;

pub use file_diff::{FileDiffView, ResetZoom, ToggleView, ZoomIn, ZoomOut};
pub use help::{HideHelp, ToggleHelp};
pub use icons::Assets;
pub use jetdiff_app::LOCAL_CHANGES;
pub use repo_view::{
    FocusDown, FocusLeft, FocusRight, FocusUp, NextFile, PrevFile, RepoView, ToggleSidePanel,
};
pub use settings::Settings;
pub use text::load_font;
pub use theme::UiTheme;
