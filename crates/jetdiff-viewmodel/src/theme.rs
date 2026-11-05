//! The one built-in theme: the colors of the diff and its syntax, and the palette around it.

mod built_in;
mod diff_colors;
mod palette;
mod syntax_style;

use jetdiff_syntax::Token;

pub use diff_colors::DiffColors;
pub use palette::{Palette, Pill};
pub use syntax_style::SyntaxStyle;

use crate::model::HunkKind;

/// RGBA, alpha 255 is opaque.
pub type Rgba = [u8; 4];

const fn rgb(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Also the gutter and divider. `EDITOR_GUTTER_BACKGROUND` is empty in IntelliJ, so they inherit it.
    pub editor_bg: Rgba,
    pub editor_fg: Rgba,
    pub line_number: Rgba,
    pub cursor_row_line_number: Rgba,
    /// The cursor's line. Transparent and stronger than `CARET_ROW_COLOR`, since it is
    /// the only highlight and must show over the diff backgrounds.
    pub cursor_row_background: Rgba,
    /// The fill behind the line numbers on the cursor's line. Yellow, since no diff color is yellow.
    pub cursor_row_gutter: Rgba,
    pub indent_guide: Rgba,
    /// `DIFF_SEPARATOR_WAVE`, for the fold wave and the fold link.
    pub fold_wave: Rgba,
    /// Gray thumb with alpha, like the macOS scrollbar in IntelliJ.
    pub scrollbar_thumb: Rgba,
    /// `Tree.selectionBackground`.
    pub tree_selection: Rgba,
    /// `Tree.selectionInactiveBackground`, when the side panel doesn't have focus.
    pub tree_selection_inactive: Rgba,
    /// The line and dots in the commit list.
    pub commit_graph: Rgba,
    /// Notification above the keymap bar. Warnings are yellow.
    pub notification_info_background: Rgba,
    pub notification_warning_background: Rgba,
    pub notification_foreground: Rgba,
    /// `FileStatus` ADDED, DELETED and MODIFIED. Rename uses MODIFIED.
    pub file_added: Rgba,
    pub file_deleted: Rgba,
    pub file_modified: Rgba,
    pub added: DiffColors,
    pub deleted: DiffColors,
    pub modified: DiffColors,
    pub palette: Palette,
    /// Indexed with `Token as usize`.
    syntax: [SyntaxStyle; Token::ALL.len()],
}

impl Theme {
    pub fn diff(&self, kind: HunkKind) -> &DiffColors {
        match kind {
            HunkKind::Added => &self.added,
            HunkKind::Deleted => &self.deleted,
            HunkKind::Modified => &self.modified,
        }
    }

    pub fn syntax(&self, token: Token) -> SyntaxStyle {
        self.syntax[token as usize]
    }
}
