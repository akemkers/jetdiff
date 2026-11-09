//! An excerpt in Unified: its rows from the top.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Window};
use jetdiff_viewmodel::{UnifiedDiff, UnifiedFolding};

use super::super::code_paint::HunkMinimapRects;
use super::super::unified::{RowCache, UnifiedPainter};
use super::super::{CACHE_CAPACITY, snap};
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub(super) struct UnifiedExcerpt {
    diff: Rc<UnifiedDiff>,
    /// None, since an excerpt shows all of its rows.
    folding: Rc<UnifiedFolding>,
    rows: Range<usize>,
    cache: RefCell<RowCache>,
    hunk_minimap: RefCell<HunkMinimapRects>,
}

impl UnifiedExcerpt {
    pub(super) fn new(diff: Rc<UnifiedDiff>, rows: Range<usize>, style: &TextStyle) -> Self {
        let mut cache = RowCache::new(CACHE_CAPACITY);
        cache.set_key(style.cache_key());
        Self {
            folding: Rc::new(UnifiedFolding::unified_none(&diff)),
            diff,
            rows,
            cache: RefCell::new(cache),
            hunk_minimap: RefCell::default(),
        }
    }

    pub(super) fn line_count(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn paint(
        &self,
        style: TextStyle,
        theme: UiTheme,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scroll_y = self.rows.start as f32 * style.line_height();
        let painter = UnifiedPainter {
            diff: self.diff.clone(),
            folding: self.folding.clone(),
            theme,
            style,
            scroll_y: snap(scroll_y, window.scale_factor()),
            scroll_x: 0.,
            cursor_row: None,
            hunk_minimap: false,
        };
        painter.paint(
            bounds,
            &mut self.cache.borrow_mut(),
            &mut self.hunk_minimap.borrow_mut(),
            window,
            cx,
        );
    }
}
