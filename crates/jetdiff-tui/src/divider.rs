//! Trakten mellom sidene, med halvblokker for dobbel høyde.

use jetdiff_viewmodel::{DiffState, LineLayout, Side, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::color::{color, diff_colors};

/// Kurven fra `jetdiff_viewmodel::divider`. X er ikke lineær i parameteren, så den finnes ved
/// halvering.
fn ease(x: f32) -> f32 {
    let bezier_x =
        |s: f32| 3. * (1. - s) * (1. - s) * s * 0.3 + 3. * (1. - s) * s * s * 0.7 + s * s * s;
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.;
        if bezier_x(mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let s = (lo + hi) / 2.;
    s * s * (3. - 2. * s)
}

fn band(layout: &LineLayout, lines: std::ops::Range<usize>, scroll_y: f32) -> (f32, f32) {
    let scroll = scroll_y.round();
    if lines.is_empty() {
        let boundary = match lines.start {
            0 => 0.,
            start => layout.line_bottom(start - 1),
        };
        let y = boundary - scroll;
        (y - 0.25, y + 0.25)
    } else {
        let top = layout.line_top(lines.start) - scroll;
        (top, layout.line_bottom(lines.end - 1) - scroll)
    }
}

pub fn paint(state: &DiffState, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let bg = color(theme.editor_bg);
    let layouts = [Side::Left, Side::Right].map(|s| state.folding.layout(s, 1.));
    let height = area.height as f32;
    let mut halves = vec![[bg; 2]; area.width as usize * area.height as usize];
    for block in state.diff.blocks() {
        let [l, r] = [Side::Left, Side::Right].map(|s| {
            let i = s.select(0, 1);
            band(&layouts[i], block.lines(s), state.scroll.y(s))
        });
        if l.1 < 0. && r.1 < 0. {
            continue;
        }
        if l.0 > height && r.0 > height {
            break;
        }
        let fill = color(diff_colors(theme, block.diff_type).background);
        for col in 0..area.width {
            let t = ease((col as f32 + 0.5) / area.width as f32);
            let top = l.0 + (r.0 - l.0) * t;
            let bottom = l.1 + (r.1 - l.1) * t;
            for row in 0..area.height {
                let cell = &mut halves[row as usize * area.width as usize + col as usize];
                for (half, color) in cell.iter_mut().enumerate() {
                    let y0 = row as f32 + half as f32 * 0.5;
                    let (center, mid) = (y0 + 0.25, (top + bottom) / 2.);
                    let thin = bottom - top < 0.5 && (y0..y0 + 0.5).contains(&mid);
                    if (top..bottom).contains(&center) || thin {
                        *color = fill;
                    }
                }
            }
        }
    }
    paint_connectors(state, &layouts, area, theme, &mut halves);
    for row in 0..area.height {
        for col in 0..area.width {
            let [upper, lower] = halves[row as usize * area.width as usize + col as usize];
            set_halves(&mut buf[(area.x + col, area.y + row)], upper, lower);
        }
    }
}

fn paint_connectors(
    state: &DiffState,
    layouts: &[LineLayout; 2],
    area: Rect,
    theme: &Theme,
    halves: &mut [[Color; 2]],
) {
    let wave = color(theme.separator_wave);
    for connector in state.folding.connectors() {
        let y = [0, 1].map(|i| {
            let side = [Side::Left, Side::Right][i];
            layouts[i].line_top(connector.lines[i]) - state.scroll.y(side).round() + 0.5
        });
        for col in 0..area.width {
            let t = ease((col as f32 + 0.5) / area.width as f32);
            let at = y[0] + (y[1] - y[0]) * t;
            let half = (at * 2.).floor();
            if half < 0. || half >= area.height as f32 * 2. {
                continue;
            }
            let (row, half) = ((half / 2.) as usize, half as usize % 2);
            halves[row * area.width as usize + col as usize][half] = wave;
        }
    }
}

fn set_halves(cell: &mut ratatui::buffer::Cell, upper: Color, lower: Color) {
    if upper == lower {
        cell.set_char(' ').set_bg(upper);
    } else {
        cell.set_char('▀').set_fg(upper).set_bg(lower);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kurven_går_fra_venstre_til_høyre() {
        assert!(ease(0.) < 1e-4);
        assert!(ease(1.) > 1. - 1e-4);
        assert!((ease(0.5) - 0.5).abs() < 1e-4);
        assert!(ease(0.25) < 0.25);
    }
}
