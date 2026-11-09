//! The GPUI layer. Draws only what `jetdiff-viewmodel` computes.

mod button_group;
mod cache;
mod code_viewer;
mod commit_list;
mod confirm_dialog;
mod diff_divider_handle;
mod drag_handle;
mod file_diff;
mod file_pair_view;
mod file_tree;
mod help;
mod icons;
mod keystroke;
mod list;
mod notification_slot_view;
mod repo_view;
mod settings;
mod side_panel_size;
mod text;
mod theme;
mod walkthrough_view;
mod watcher;

pub use file_diff::{ResetZoom, ToggleDiffMode, ZoomIn, ZoomOut};
pub use file_pair_view::FilePairView;
pub use help::{HideHelp, ToggleHelp};
pub use icons::Assets;
pub use repo_view::{
    FocusDown, FocusLeft, FocusRight, FocusUp, NextCommit, NextFile, PrevCommit, PrevFile,
    RepoView, SourceReload, ToggleSidePanel, ToggleWalkthrough,
};
pub use settings::Settings;
pub use text::load_font;
pub use theme::UiTheme;

/// Duration of an animated jump, in the diff and the walkthrough.
const NAV_ANIMATION_MS: f32 = 170.;
