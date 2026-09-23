//! GPUI-laget. Tegner bare det `smudiff-view` regner ut.

pub mod cache;
pub mod divider;
pub mod file_diff;
pub mod geometry;
pub mod metrics;
pub mod side_editor;
pub mod tabs;
pub mod text;
pub mod theme;

pub use file_diff::{
    CollapseAllFolds, CollapseFold, ExpandAllFolds, ExpandFold, FileDiffView, NextChange,
    NextContextRange, PreviousChange,
};
pub use theme::UiTheme;
