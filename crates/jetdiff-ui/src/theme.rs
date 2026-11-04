//! `jetdiff_viewmodel::Theme` translated to GPUI colors once, so paint skips the conversion.

use gpui::{Font, FontStyle, FontWeight, Hsla};
use jetdiff_viewmodel::theme::{DiffColors, Palette};
use jetdiff_viewmodel::{HunkKind, Rgba, SyntaxStyle, Theme, Token};

pub fn hsla(c: Rgba) -> Hsla {
    gpui::Rgba {
        r: c[0] as f32 / 255.,
        g: c[1] as f32 / 255.,
        b: c[2] as f32 / 255.,
        a: c[3] as f32 / 255.,
    }
    .into()
}

pub type UiPalette = Palette<Hsla>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiDiffColors {
    pub background: Hsla,
    pub hunk_minimap: Hsla,
    pub ignored: Hsla,
}

impl From<&DiffColors> for UiDiffColors {
    fn from(c: &DiffColors) -> Self {
        Self {
            background: hsla(c.background),
            hunk_minimap: hsla(c.hunk_minimap),
            ignored: hsla(c.ignored),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiSyntax {
    pub color: Hsla,
    weight: FontWeight,
    style: FontStyle,
}

impl UiSyntax {
    fn new(s: SyntaxStyle) -> Self {
        Self {
            color: hsla(s.color),
            weight: if s.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            style: if s.italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
        }
    }

    pub fn font(&self, base: &Font) -> Font {
        Font {
            weight: self.weight,
            style: self.style,
            ..base.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTheme {
    pub editor_bg: Hsla,
    pub editor_fg: Hsla,
    pub line_number: Hsla,
    pub cursor_row_line_number: Hsla,
    pub cursor_row_background: Hsla,
    pub cursor_row_gutter: Hsla,
    pub indent_guide: Hsla,
    pub fold_wave: Hsla,
    pub scrollbar_thumb: Hsla,
    pub tree_selection: Hsla,
    pub tree_selection_inactive: Hsla,
    pub commit_graph: Hsla,
    pub notification_info_background: Hsla,
    pub notification_warning_background: Hsla,
    pub notification_foreground: Hsla,
    pub file_added: Hsla,
    pub file_deleted: Hsla,
    pub file_modified: Hsla,
    pub palette: UiPalette,
    added: UiDiffColors,
    deleted: UiDiffColors,
    modified: UiDiffColors,
    syntax: [UiSyntax; Token::ALL.len()],
}

impl UiTheme {
    pub fn new(theme: &Theme) -> Self {
        Self {
            editor_bg: hsla(theme.editor_bg),
            editor_fg: hsla(theme.editor_fg),
            line_number: hsla(theme.line_number),
            cursor_row_line_number: hsla(theme.cursor_row_line_number),
            cursor_row_background: hsla(theme.cursor_row_background),
            cursor_row_gutter: hsla(theme.cursor_row_gutter),
            indent_guide: hsla(theme.indent_guide),
            fold_wave: hsla(theme.fold_wave),
            scrollbar_thumb: hsla(theme.scrollbar_thumb),
            tree_selection: hsla(theme.tree_selection),
            tree_selection_inactive: hsla(theme.tree_selection_inactive),
            commit_graph: hsla(theme.commit_graph),
            notification_info_background: hsla(theme.notification_info_background),
            notification_warning_background: hsla(theme.notification_warning_background),
            notification_foreground: hsla(theme.notification_foreground),
            file_added: hsla(theme.file_added),
            file_deleted: hsla(theme.file_deleted),
            file_modified: hsla(theme.file_modified),
            palette: theme.palette.map(hsla),
            added: (&theme.added).into(),
            deleted: (&theme.deleted).into(),
            modified: (&theme.modified).into(),
            syntax: Token::ALL.map(|token| UiSyntax::new(theme.syntax(token))),
        }
    }

    pub fn syntax(&self, token: Token) -> &UiSyntax {
        &self.syntax[token as usize]
    }

    pub fn diff(&self, kind: HunkKind) -> &UiDiffColors {
        match kind {
            HunkKind::Added => &self.added,
            HunkKind::Deleted => &self.deleted,
            HunkKind::Modified => &self.modified,
        }
    }
}
