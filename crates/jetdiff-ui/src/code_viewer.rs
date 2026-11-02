//! The code in the diff, in Split or Unified: drawing, what the mouse hits and the text width.
//! `FileDiffView` makes a `Painter` in `render` and draws it in the canvas.

mod code_paint;
mod divider;
mod split;
mod split_side;
mod unified;

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use gpui::{App, Bounds, Pixels, Point, Window};
use jetdiff_core::side::Side;
use jetdiff_viewmodel::{DiffMode, HunkMinimapMark, ScrollState, UnifiedDiff, Viewport};

use self::code_paint::HunkMinimapRects;
use self::split::SplitPainter;
use self::split_side::{CursorRow, SideCache};
use self::unified::{RowCache, UnifiedPainter};
use crate::text::TextStyle;
use crate::theme::UiTheme;

/// Lines outside the viewport that are kept shaped, per side.
const CACHE_CAPACITY: usize = 2000;

const SIDES: [Side; 2] = [Side::Left, Side::Right];

/// What paint shares with the events: last bounds, shaped lines and the hunk minimap.
pub struct Shared {
    pub bounds: Cell<Option<Bounds<Pixels>>>,
    /// The widest line that has been drawn, for max horizontal scroll.
    pub widest: Cell<f32>,
    split_lines: RefCell<[SideCache; 2]>,
    split_hunk_minimaps: RefCell<[HunkMinimapRects; 2]>,
    unified_rows: RefCell<RowCache>,
    unified_hunk_minimap: RefCell<HunkMinimapRects>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            bounds: Cell::new(None),
            widest: Cell::new(0.),
            split_lines: RefCell::new(new_side_caches()),
            split_hunk_minimaps: RefCell::default(),
            unified_rows: RefCell::new(RowCache::new(CACHE_CAPACITY)),
            unified_hunk_minimap: RefCell::default(),
        }
    }

    /// A new file or new content.
    pub fn clear(&self) {
        *self.split_lines.borrow_mut() = new_side_caches();
        *self.unified_rows.borrow_mut() = RowCache::new(CACHE_CAPACITY);
        self.widest.set(0.);
    }

    /// The key has the font size, so zoom clears the cache.
    pub fn set_style(&self, style: &TextStyle) {
        for cache in self.split_lines.borrow_mut().iter_mut() {
            cache.set_key(style.cache_key());
        }
        self.unified_rows.borrow_mut().set_key(style.cache_key());
        self.widest.set(0.);
    }

    /// The hunk minimap is recomputed the next time it is drawn.
    pub fn clear_hunk_minimaps(&self) {
        *self.split_hunk_minimaps.borrow_mut() = Default::default();
        *self.unified_hunk_minimap.borrow_mut() = Default::default();
    }
}

/// Rows of a file in Unified, drawn over all of their bounds without a cursor or a scrollbar, for a
/// walkthrough. Each has its own cache, since a cache keeps only the rows it drew last.
pub struct Excerpt {
    diff: Rc<UnifiedDiff>,
    rows: Range<usize>,
    cache: RefCell<RowCache>,
    hunk_minimap: RefCell<HunkMinimapRects>,
}

impl Excerpt {
    pub fn new(diff: Rc<UnifiedDiff>, rows: Range<usize>, style: &TextStyle) -> Self {
        let mut cache = RowCache::new(CACHE_CAPACITY);
        cache.set_key(style.cache_key());
        Self {
            diff,
            rows,
            cache: RefCell::new(cache),
            hunk_minimap: RefCell::default(),
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn paint(
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

fn new_side_caches() -> [SideCache; 2] {
    [
        SideCache::new(CACHE_CAPACITY),
        SideCache::new(CACHE_CAPACITY),
    ]
}

/// What is under the mouse.
pub enum Hit {
    Split(Side, usize),
    Unified(usize),
}

pub fn hit(
    mode: &DiffMode,
    style: &TextStyle,
    bounds: Bounds<Pixels>,
    scroll_y: [f32; 2],
    position: Point<Pixels>,
) -> Option<Hit> {
    match mode {
        DiffMode::Split(state) => split::hit(
            &state.diff,
            &state.folding,
            style,
            bounds,
            scroll_y,
            position,
        )
        .map(|(side, line)| Hit::Split(side, line)),
        DiffMode::Unified(state) => {
            unified::hit(&state.diff, style, bounds, scroll_y[0], position).map(Hit::Unified)
        }
    }
}

/// Split scrolls the side the mouse is over, and the divider counts as the nearest side.
pub fn wheel(
    mode: &mut DiffMode,
    bounds: Option<Bounds<Pixels>>,
    x: Pixels,
    delta_y: f32,
    viewport: Viewport,
) {
    match mode {
        DiffMode::Split(state) => {
            let side = bounds.map_or(state.master, |bounds| split::side_at(bounds, x));
            state.wheel(side, delta_y, viewport);
        }
        DiffMode::Unified(state) => {
            state.wheel(delta_y, viewport);
        }
    }
}

pub fn text_width(mode: &DiffMode, style: &TextStyle, bounds: Bounds<Pixels>) -> f32 {
    match mode {
        DiffMode::Split(state) => split::text_width(&state.diff, style, bounds),
        DiffMode::Unified(state) => unified::text_width(&state.diff, style, bounds),
    }
}

/// What to draw in one frame.
pub enum Painter {
    Split(SplitPainter),
    Unified(UnifiedPainter),
}

impl Painter {
    /// `shown` is the scroll as it is shown now, in the middle of any animation.
    pub fn new(
        mode: &DiffMode,
        hunk_minimap_marks: &Rc<[Vec<HunkMinimapMark>; 2]>,
        style: TextStyle,
        shown: ScrollState,
        focused: bool,
        scale: f32,
        theme: UiTheme,
    ) -> Self {
        let scroll_x = snap(shown.x, scale);
        match mode {
            DiffMode::Split(state) => {
                // The other side shows the mirrored cursor more faintly, so you see where H and L land.
                // IntelliJ paints the cursor row only on the active side.
                let cursor = state.cursor;
                let mirrored = state.mirrored_cursor();
                let cursor_rows = SIDES.map(|side| {
                    let side_cursor = if side == cursor.side {
                        cursor
                    } else {
                        mirrored
                    };
                    CursorRow {
                        line: side_cursor.line,
                        active: focused && side == cursor.side,
                    }
                });
                Self::Split(SplitPainter {
                    diff: state.diff.clone(),
                    folding: state.folding.clone(),
                    theme,
                    style,
                    hunk_minimap_marks: hunk_minimap_marks.clone(),
                    scroll_y: shown.y.map(|y| snap(y, scale)),
                    scroll_x,
                    cursor_rows,
                })
            }
            DiffMode::Unified(state) => Self::Unified(UnifiedPainter {
                diff: state.diff.clone(),
                theme,
                style,
                scroll_y: snap(shown.y[0], scale),
                scroll_x,
                cursor_row: Some(CursorRow {
                    line: state.row,
                    active: focused,
                }),
                hunk_minimap: true,
            }),
        }
    }

    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        shared: &Shared,
        window: &mut Window,
        cx: &mut App,
    ) {
        let previous = shared.bounds.replace(Some(bounds));
        if previous.map(|previous| previous.size.height) != Some(bounds.size.height) {
            window.refresh();
        }
        let widest = match self {
            Self::Split(painter) => painter.paint(
                bounds,
                &mut shared.split_lines.borrow_mut(),
                &mut shared.split_hunk_minimaps.borrow_mut(),
                window,
                cx,
            ),
            Self::Unified(painter) => painter.paint(
                bounds,
                &mut shared.unified_rows.borrow_mut(),
                &mut shared.unified_hunk_minimap.borrow_mut(),
                window,
                cx,
            ),
        };
        shared.widest.set(shared.widest.get().max(widest));
    }
}

/// Snaps to whole device pixels so edges and text don't get blurry in the middle of a scroll.
fn snap(y: f32, scale: f32) -> f32 {
    (y * scale).round() / scale
}
