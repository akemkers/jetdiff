//! Rows of a file drawn over all of their bounds without a cursor or a scrollbar, for a walkthrough.
//! Split or Unified as in the diff, and added and deleted files are always shown Unified.

mod split;
mod unified;

use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Window};
use jetdiff_viewmodel::UnifiedDiff;

use self::split::SplitExcerpt;
use self::unified::UnifiedExcerpt;
use crate::text::TextStyle;
use crate::theme::UiTheme;

/// Each has its own cache, since a cache keeps only the rows it drew last.
pub struct Excerpt {
    row_count: usize,
    mode: Mode,
}

enum Mode {
    Split(Box<SplitExcerpt>),
    Unified(UnifiedExcerpt),
}

impl Excerpt {
    /// `rows` are rows in Unified, and Split shows the lines they have on each side.
    pub fn new(
        diff: Rc<UnifiedDiff>,
        rows: Range<usize>,
        unified: bool,
        style: &TextStyle,
    ) -> Self {
        let row_count = rows.len();
        let mode = if unified || !diff.diff.can_split() {
            Mode::Unified(UnifiedExcerpt::new(diff, rows, style))
        } else {
            Mode::Split(Box::new(SplitExcerpt::new(&diff, rows, style)))
        };
        Self { row_count, mode }
    }

    /// The rows in Unified, also in Split, so a page has the same items in both.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// The height in lines.
    pub fn line_count(&self) -> usize {
        match &self.mode {
            Mode::Split(excerpt) => excerpt.line_count(),
            Mode::Unified(excerpt) => excerpt.line_count(),
        }
    }

    pub fn paint(
        &self,
        style: TextStyle,
        theme: UiTheme,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        match &self.mode {
            Mode::Split(excerpt) => excerpt.paint(style, theme, bounds, window, cx),
            Mode::Unified(excerpt) => excerpt.paint(style, theme, bounds, window, cx),
        }
    }
}
