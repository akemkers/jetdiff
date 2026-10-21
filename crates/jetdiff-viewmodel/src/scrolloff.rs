//! Vim's `scrolloff` for one code view: where the cursor can be without scrolling, and the smallest
//! scroll that keeps it there. The cursor is a visual line in `layout`.

use crate::layout::{LineLayout, Viewport};

/// Number of lines to the top/bottom before the cursor starts to scroll.
const SCROLLOFF: usize = 8;

/// A small margin against floating point errors when scroll is on the boundary between two lines.
const TOLERANCE: f32 = 1e-3;

/// How many whole lines fit in the viewport.
pub(crate) fn screen_lines(viewport: Viewport) -> usize {
    (viewport.height / viewport.line_height + TOLERANCE)
        .floor()
        .max(0.) as usize
}

/// `SCROLLOFF`, but never more than there is room for above and below the cursor in a small window.
fn scrolloff(viewport: Viewport) -> usize {
    SCROLLOFF.min(screen_lines(viewport).saturating_sub(1) / 2)
}

/// First and last whole visual line with text in the viewport.
fn full_lines(layout: &LineLayout, scroll_y: f32, viewport: Viewport) -> (usize, usize) {
    let line_height = viewport.line_height;
    let last_line = layout.visual_line_count() - 1;
    let first = ((scroll_y / line_height - TOLERANCE).ceil().max(0.) as usize).min(last_line);
    let last = ((scroll_y + viewport.height) / line_height + TOLERANCE).floor() as usize;
    (first, last.saturating_sub(1).clamp(first, last_line))
}

/// Visual lines the cursor can be on without scrolling. At the top and bottom of
/// the file scrolloff is relaxed, otherwise the cursor could never reach the first and last line.
pub(crate) fn cursor_range(
    layout: &LineLayout,
    scroll_y: f32,
    viewport: Viewport,
) -> (usize, usize) {
    let margin = scrolloff(viewport);
    let last = layout.visual_line_count() - 1;
    let (first_full, last_full) = full_lines(layout, scroll_y, viewport);
    let top = if scroll_y <= TOLERANCE { 0 } else { margin };
    let bottom = if scroll_y >= layout.max_scroll(viewport.height) - TOLERANCE {
        0
    } else {
        margin
    };
    let min = (first_full + top).min(last);
    let max = last_full.saturating_sub(bottom).max(min);
    (min, max)
}

/// The smallest scroll that gives the cursor scrolloff. If the jump is long, the line is centered like
/// vim's `scroll_cursor_halfway`.
pub(crate) fn follow_y(
    layout: &LineLayout,
    cursor: usize,
    scroll_y: f32,
    viewport: Viewport,
    allow_center: bool,
) -> f32 {
    let line_height = viewport.line_height;
    let margin = scrolloff(viewport) as f32;
    let cursor_line = cursor as f32;
    let min_y = (cursor_line + 1. + margin) * line_height - viewport.height;
    let max_y = (cursor_line - margin) * line_height;
    let mut y = scroll_y;
    if y < min_y - TOLERANCE {
        y = min_y;
    }
    if y > max_y + TOLERANCE {
        y = max_y;
    }
    // Vim centers when it has to scroll a whole screen down or half a screen up
    let far = (y > scroll_y && y - scroll_y >= viewport.height)
        || (y < scroll_y && scroll_y - y >= viewport.height / 2.);
    if allow_center && far {
        y = centered_y(cursor, viewport);
    }
    layout.clamp_scroll(y, viewport.height)
}

fn centered_y(cursor: usize, viewport: Viewport) -> f32 {
    let above = (screen_lines(viewport).saturating_sub(1) / 2) as f32;
    (cursor as f32 - above) * viewport.line_height
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_HEIGHT: f32 = 10.;

    fn viewport(height: f32) -> Viewport {
        Viewport {
            height,
            line_height: LINE_HEIGHT,
        }
    }

    #[test]
    fn scrolloff_is_clamped_in_small_window() {
        assert_eq!(scrolloff(viewport(50.)), 2);
        assert_eq!(scrolloff(viewport(200.)), 8);
        assert_eq!(scrolloff(viewport(5.)), 0);
    }

    #[test]
    fn scrolloff_is_relaxed_at_top_and_bottom() {
        let layout = LineLayout::new(100, LINE_HEIGHT);
        let max = layout.max_scroll(200.);
        assert_eq!(cursor_range(&layout, 0., viewport(200.)), (0, 11));
        assert_eq!(cursor_range(&layout, 100., viewport(200.)), (18, 21));
        assert_eq!(cursor_range(&layout, max, viewport(200.)).1, 99);
    }

    #[test]
    fn long_jump_is_centered() {
        let layout = LineLayout::new(100, LINE_HEIGHT);
        // Line 50 with scrolloff 8 at the bottom of 20 lines
        assert_eq!(follow_y(&layout, 50, 0., viewport(200.), false), 390.);
        // 9 lines above the cursor
        assert_eq!(follow_y(&layout, 50, 0., viewport(200.), true), 410.);
        // A short jump is not centered
        assert_eq!(follow_y(&layout, 25, 0., viewport(200.), true), 140.);
    }
}
