//! Vim sin `scrolloff` for én editor: hvor cursoren kan stå uten at det scrolles, og minste
//! scroll som holder den der. Cursoren er en visuell linje i `layout`.

use crate::layout::{LineLayout, Viewport};

/// Antall linjer til topp/bunn før cursor begynner å scrolle.
const SCROLLOFF: usize = 8;

/// Liten margin mot flyttallsfeil når scroll står på grensen mellom to linjer.
const TOLERANCE: f32 = 1e-3;

/// Hvor mange hele linjer det er plass til i viewporten.
pub(crate) fn screen_lines(viewport: Viewport) -> usize {
    (viewport.height / viewport.line_height + TOLERANCE)
        .floor()
        .max(0.) as usize
}

/// `SCROLLOFF`, men aldri mer enn det er plass til over og under cursoren i et lite vindu.
fn scrolloff(viewport: Viewport) -> usize {
    SCROLLOFF.min(screen_lines(viewport).saturating_sub(1) / 2)
}

/// Første og siste hele visuelle linje med tekst i viewporten.
fn full_lines(layout: &LineLayout, scroll_y: f32, viewport: Viewport) -> (usize, usize) {
    let line_height = viewport.line_height;
    let last_line = layout.visual_line_count() - 1;
    let first = ((scroll_y / line_height - TOLERANCE).ceil().max(0.) as usize).min(last_line);
    let last = ((scroll_y + viewport.height) / line_height + TOLERANCE).floor() as usize;
    (first, last.saturating_sub(1).clamp(first, last_line))
}

/// Visuelle linjer cursoren kan stå på uten at det scrolles. Øverst og nederst i
/// fila slippes scrolloff, ellers kunne cursoren aldri nå første og siste linje.
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

/// Minste scroll som gir cursoren scrolloff. Er hoppet langt, sentreres linja som
/// vim sin `scroll_cursor_halfway`.
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
    // Vim sentrerer når det må scrolles en hel skjerm ned eller en halv opp
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
    fn scrolloff_klemmes_i_lite_vindu() {
        assert_eq!(scrolloff(viewport(50.)), 2);
        assert_eq!(scrolloff(viewport(200.)), 8);
        assert_eq!(scrolloff(viewport(5.)), 0);
    }

    #[test]
    fn scrolloff_slippes_i_toppen_og_bunnen() {
        let layout = LineLayout::new(100, LINE_HEIGHT);
        let max = layout.max_scroll(200.);
        assert_eq!(cursor_range(&layout, 0., viewport(200.)), (0, 11));
        assert_eq!(cursor_range(&layout, 100., viewport(200.)), (18, 21));
        assert_eq!(cursor_range(&layout, max, viewport(200.)).1, 99);
    }

    #[test]
    fn langt_hopp_sentreres() {
        let layout = LineLayout::new(100, LINE_HEIGHT);
        // Linje 50 med scrolloff 8 nederst i 20 linjer
        assert_eq!(follow_y(&layout, 50, 0., viewport(200.), false), 390.);
        // 9 linjer over cursoren
        assert_eq!(follow_y(&layout, 50, 0., viewport(200.), true), 410.);
        // Kort hopp sentreres ikke
        assert_eq!(follow_y(&layout, 25, 0., viewport(200.), true), 140.);
    }
}
