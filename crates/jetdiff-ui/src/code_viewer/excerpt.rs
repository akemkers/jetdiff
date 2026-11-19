//! Rows of a file drawn over all of their bounds without a cursor or a scrollbar, for a walkthrough.
//! Split or Unified as in the diff, and added and deleted files are always shown Unified. Each
//! excerpt has its own horizontal scroll.

mod split;
mod unified;

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Window};
use jetdiff_viewmodel::{SplitShare, UnifiedDiff, diff_divider_offset};

use self::split::SplitExcerpt;
use self::unified::UnifiedExcerpt;
use super::{CodeLayout, snap};
use crate::text::TextStyle;
use crate::theme::UiTheme;

/// Each has its own cache, since a cache keeps only the rows it drew last.
pub struct Excerpt {
    row_count: usize,
    mode: Mode,
    /// The width it was drawn with last, for the handle over the diff divider.
    width: Cell<Option<f32>>,
    scroll_x: Cell<f32>,
    /// From the last time it was drawn, for max horizontal scroll.
    widest: Cell<f32>,
    text_width: Cell<f32>,
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
        Self {
            row_count,
            mode,
            width: Cell::new(None),
            scroll_x: Cell::new(0.),
            widest: Cell::new(0.),
            text_width: Cell::new(0.),
        }
    }

    /// From the left edge, with the width it was drawn with last. `None` in Unified and before it
    /// is drawn.
    pub fn diff_divider_offset(&self, split_share: SplitShare) -> Option<f32> {
        let width = self
            .width
            .get()
            .filter(|_| matches!(self.mode, Mode::Split(_)))?;
        Some(diff_divider_offset(width, split_share))
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

    /// Kept within the widest line. Returns whether it moved.
    pub fn scroll_x_by(&self, delta: f32) -> bool {
        let x = self.clamped_scroll_x(self.scroll_x.get() + delta);
        self.scroll_x.replace(x) != x
    }

    fn clamped_scroll_x(&self, x: f32) -> f32 {
        let max = self.widest.get() - self.text_width.get();
        x.clamp(0., max.max(0.))
    }

    pub fn paint(
        &self,
        style: TextStyle,
        split_share: SplitShare,
        theme: UiTheme,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // The handle is placed from the width when the page is rendered
        let width = f32::from(bounds.size.width);
        if self.width.replace(Some(width)) != Some(width) {
            window.refresh();
        }
        let code_layout = CodeLayout {
            style: &style,
            split_share,
        };
        // A wider window has less to scroll
        self.text_width.set(match &self.mode {
            Mode::Split(excerpt) => excerpt.text_width(code_layout, bounds),
            Mode::Unified(excerpt) => excerpt.text_width(&style, bounds),
        });
        self.scroll_x
            .set(self.clamped_scroll_x(self.scroll_x.get()));
        let scroll_x = snap(self.scroll_x.get(), window.scale_factor());
        let widest = match &self.mode {
            Mode::Split(excerpt) => excerpt.paint(code_layout, scroll_x, theme, bounds, window, cx),
            Mode::Unified(excerpt) => excerpt.paint(style, scroll_x, theme, bounds, window, cx),
        };
        self.widest.set(widest);
    }
}
