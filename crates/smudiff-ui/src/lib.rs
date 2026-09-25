//! GPUI-laget. Tegner bare det `smudiff-view` regner ut.

mod cache;
mod changes_tree;
mod changes_view;
mod divider;
mod file_diff;
mod icons;
mod message;
mod side_editor;
mod text;
mod theme;

pub use changes_view::ChangesView;
pub use file_diff::{FileDiffView, NextContextRange, ResetZoom, ZoomIn, ZoomOut};
pub use icons::Assets;
pub use message::Message;
pub use text::load_font;
pub use theme::UiTheme;
