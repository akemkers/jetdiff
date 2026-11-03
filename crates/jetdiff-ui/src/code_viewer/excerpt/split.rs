//! An excerpt in Split: each side from its first line, with the diff divider between. The shorter
//! side ends early, and the divider has bands only for the excerpt's hunks, so the next hunk doesn't
//! show under it.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Window};
use jetdiff_viewmodel::{FileDiff, Folding, UnifiedDiff, Viewport};

use super::super::code_paint::paint_code_background;
use super::super::divider::Divider;
use super::super::split::side_geometry;
use super::super::split_side::{SideCache, SplitSide};
use super::super::{SIDES, new_side_caches, snap};
use crate::text::TextStyle;
use crate::theme::UiTheme;

pub(super) struct SplitExcerpt {
    diff: Rc<FileDiff>,
    folding: Folding,
    /// Per side.
    lines: [Range<usize>; 2],
    hunks: Range<usize>,
    caches: RefCell<[SideCache; 2]>,
}

impl SplitExcerpt {
    pub(super) fn new(diff: &UnifiedDiff, rows: Range<usize>, style: &TextStyle) -> Self {
        let mut caches = new_side_caches();
        for cache in &mut caches {
            cache.set_key(style.cache_key());
        }
        Self {
            diff: diff.diff.clone(),
            folding: Folding::none(&diff.diff),
            lines: SIDES.map(|side| diff.side_lines(rows.clone(), side)),
            hunks: diff.hunks_in(rows),
            caches: RefCell::new(caches),
        }
    }

    pub(super) fn line_count(&self) -> usize {
        self.lines.iter().map(Range::len).max().unwrap_or(0)
    }

    pub(super) fn paint(
        &self,
        style: TextStyle,
        theme: UiTheme,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let top = f32::from(bounds.origin.y);
        let height = f32::from(bounds.size.height);
        let line_height = style.line_height();
        let scale = window.scale_factor();
        let scroll_y = self
            .lines
            .clone()
            .map(|lines| snap(lines.start as f32 * line_height, scale));
        let geometries = side_geometry(&self.diff, &style, bounds);
        let mut caches = self.caches.borrow_mut();
        for (index, geom) in geometries.iter().enumerate() {
            let side_height = self.lines[index].len() as f32 * line_height;
            paint_code_background(geom, top + side_height, top + height, &theme, window);
            let split_side = SplitSide {
                diff: &self.diff,
                folding: &self.folding,
                geom,
                theme: &theme,
                style: &style,
                top,
                height: side_height,
                scroll_y: scroll_y[index],
                scroll_x: 0.,
                cursor_row: None,
                hunk_minimap: None,
            };
            split_side.paint(&mut caches[index], window, cx);
        }
        Divider {
            diff: &self.diff,
            folding: &self.folding,
            theme: &theme,
            x: geometries[0].gutter.end,
            top,
            viewport: Viewport {
                height,
                line_height,
            },
            scroll_y,
            hunks: self.hunks.clone(),
        }
        .paint(window);
    }
}
