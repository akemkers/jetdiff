// Spike: to tekstkolonner med synkron scrolling og bezier-trakt i midten.
// Bruk: gpui-trakt <venstre-fil> <høyre-fil>

use gpui::{
    App, Application, Bounds, ContentMask, Context, Font, Hsla, PathBuilder, Pixels, Render,
    ScrollWheelEvent, SharedString, TextRun, Window, WindowBounds, WindowOptions, canvas, div,
    font, point, prelude::*, px, quad, rgb, size,
};
use imara_diff::{Algorithm, Diff, InternedInput};

const LINE_HEIGHT: f32 = 20.;
const FONT_SIZE: f32 = 13.;
const GUTTER: f32 = 48.;
const DIVIDER: f32 = 36.;

#[derive(Clone, Copy)]
struct Hunk {
    left: (u32, u32),
    right: (u32, u32),
}

impl Hunk {
    fn color(&self) -> Hsla {
        if self.left.0 == self.left.1 {
            rgb(0x2f4a33).into() // inserted
        } else if self.right.0 == self.right.1 {
            rgb(0x4a3036).into() // deleted
        } else {
            rgb(0x2c3a52).into() // modified
        }
    }
}

struct DiffView {
    left: Vec<SharedString>,
    right: Vec<SharedString>,
    hunks: Vec<Hunk>,
    // Venstre side er master. Høyre side avledes fra den.
    scroll_y: f32,
    // Horisontal scroll deles av begge sider, som i JetBrains.
    scroll_x: f32,
    max_cols: usize,
}

impl DiffView {
    fn new(a: &str, b: &str) -> Self {
        let input = InternedInput::new(a, b);
        let mut diff = Diff::compute(Algorithm::Histogram, &input);
        diff.postprocess_lines(&input);
        let hunks = diff
            .hunks()
            .map(|h| Hunk {
                left: (h.before.start, h.before.end),
                right: (h.after.start, h.after.end),
            })
            .collect();
        let lines = |s: &str| {
            s.lines()
                .map(|l| SharedString::from(l.replace('\t', "    ")))
                .collect()
        };
        let (left, right): (Vec<SharedString>, Vec<SharedString>) = (lines(a), lines(b));
        let max_cols = left
            .iter()
            .chain(&right)
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
        Self {
            left,
            right,
            hunks,
            scroll_y: 0.,
            scroll_x: 0.,
            max_cols,
        }
    }

    // Overfører en (fraksjonell) linje fra venstre til høyre. Lineært inni hunks.
    fn transfer(&self, line: f32) -> f32 {
        let mut offset = 0.;
        for h in &self.hunks {
            let (l0, l1) = (h.left.0 as f32, h.left.1 as f32);
            let (r0, r1) = (h.right.0 as f32, h.right.1 as f32);
            if line < l0 {
                break;
            }
            if line < l1 || l0 == l1 && line < l0 + 1e-3 {
                return r0 + (line - l0) / (l1 - l0).max(1.) * (r1 - r0);
            }
            offset = r1 - l1;
        }
        line + offset
    }

    fn right_scroll(&self, viewport_h: f32) -> f32 {
        let center = (self.scroll_y + viewport_h / 2.) / LINE_HEIGHT;
        (self.transfer(center) * LINE_HEIGHT - viewport_h / 2.).max(0.)
    }
}

fn paint_side(
    lines: &[SharedString],
    ranges: impl Iterator<Item = ((u32, u32), Hsla)>,
    origin_x: f32,
    width: f32,
    top: f32,
    scroll: f32,
    scroll_x: f32,
    bounds: Bounds<Pixels>,
    mono: &Font,
    window: &mut Window,
    cx: &mut App,
) {
    let h = f32::from(bounds.size.height);
    let side = Bounds::new(
        point(px(origin_x), bounds.origin.y),
        size(px(width), bounds.size.height),
    );
    let text_area = Bounds::new(
        point(px(origin_x + GUTTER), bounds.origin.y),
        size(px(width - GUTTER), bounds.size.height),
    );
    window.with_content_mask(Some(ContentMask { bounds: side }), |window| {
        let y_of = |line: u32| top + line as f32 * LINE_HEIGHT - scroll;

        for ((start, end), color) in ranges {
            let (y0, y1) = (y_of(start), y_of(end));
            if y1 < top - 2. || y0 > top + h {
                continue;
            }
            let rect = if start == end {
                Bounds::new(point(px(origin_x), px(y0 - 1.)), size(px(width), px(2.)))
            } else {
                Bounds::new(point(px(origin_x), px(y0)), size(px(width), px(y1 - y0)))
            };
            let border = color.opacity(1.).blend(gpui::white().opacity(0.15));
            window.paint_quad(quad(
                rect,
                px(0.),
                color,
                px(0.),
                border,
                Default::default(),
            ));
        }

        let first = (scroll / LINE_HEIGHT).floor().max(0.) as usize;
        let visible = (h / LINE_HEIGHT).ceil() as usize + 1;
        let text_color: Hsla = rgb(0xbcbec4).into();
        let num_color: Hsla = rgb(0x6f737a).into();
        for (i, line) in lines.iter().enumerate().skip(first).take(visible) {
            let y = y_of(i as u32);
            let num = SharedString::from(format!("{:>4}", i + 1));
            let run = |len, color| {
                [TextRun {
                    len,
                    font: mono.clone(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }]
            };
            let shaped = window.text_system().shape_line(
                num.clone(),
                px(FONT_SIZE),
                &run(num.len(), num_color),
                None,
            );
            let _ = shaped.paint(point(px(origin_x), px(y)), px(LINE_HEIGHT), window, cx);
            if !line.is_empty() {
                let shaped = window.text_system().shape_line(
                    line.clone(),
                    px(FONT_SIZE),
                    &run(line.len(), text_color),
                    None,
                );
                window.with_content_mask(Some(ContentMask { bounds: text_area }), |window| {
                    let _ = shaped.paint(
                        point(px(origin_x + GUTTER - scroll_x), px(y)),
                        px(LINE_HEIGHT),
                        window,
                        cx,
                    );
                });
            }
        }
    });
}

impl Render for DiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_h = f32::from(window.viewport_size().height);
        let left_scroll = self.scroll_y;
        let scroll_x = self.scroll_x;
        let right_scroll = self.right_scroll(viewport_h);
        let left = self.left.clone();
        let right = self.right.clone();
        let hunks = self.hunks.clone();

        div()
            .size_full()
            .bg(rgb(0x1e1f22))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, window, cx| {
                let delta = ev.delta.pixel_delta(px(LINE_HEIGHT));
                let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                let max_y = (this.left.len() as f32 * LINE_HEIGHT
                    - f32::from(window.viewport_size().height) / 2.)
                    .max(0.);
                // Menlo er ca. 0.6 em bred. Godt nok for en spike.
                let max_x = (this.max_cols as f32 * FONT_SIZE * 0.6 - 200.).max(0.);
                this.scroll_y = (this.scroll_y - dy).clamp(0., max_y);
                this.scroll_x = (this.scroll_x - dx).clamp(0., max_x);
                cx.notify();
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        let mono = font("Menlo");
                        let x0 = f32::from(bounds.origin.x);
                        let top = f32::from(bounds.origin.y);
                        let total_w = f32::from(bounds.size.width);
                        let side_w = (total_w - DIVIDER) / 2.;
                        let div_x0 = x0 + side_w;
                        let div_x1 = div_x0 + DIVIDER;

                        paint_side(
                            &left,
                            hunks.iter().map(|h| (h.left, h.color())),
                            x0,
                            side_w,
                            top,
                            left_scroll,
                            scroll_x,
                            bounds,
                            &mono,
                            window,
                            cx,
                        );
                        paint_side(
                            &right,
                            hunks.iter().map(|h| (h.right, h.color())),
                            div_x1,
                            side_w,
                            top,
                            right_scroll,
                            scroll_x,
                            bounds,
                            &mono,
                            window,
                            cx,
                        );

                        // Trakten: to kubiske kurver mellom venstre og høyre y-intervall.
                        let mid = (div_x0 + div_x1) / 2.;
                        for h in &hunks {
                            let ly0 = top + h.left.0 as f32 * LINE_HEIGHT - left_scroll;
                            let ly1 = top + h.left.1 as f32 * LINE_HEIGHT - left_scroll;
                            let ry0 = top + h.right.0 as f32 * LINE_HEIGHT - right_scroll;
                            let ry1 = top + h.right.1 as f32 * LINE_HEIGHT - right_scroll;
                            let h_px = f32::from(bounds.size.height);
                            if ly1.max(ry1) < top || ly0.min(ry0) > top + h_px {
                                continue;
                            }
                            let p = |x: f32, y: f32| point(px(x), px(y));
                            let mut b = PathBuilder::fill();
                            b.move_to(p(div_x0, ly0));
                            b.cubic_bezier_to(p(div_x1, ry0), p(mid, ly0), p(mid, ry0));
                            b.line_to(p(div_x1, ry1.max(ry0 + 1.)));
                            b.cubic_bezier_to(
                                p(div_x0, ly1.max(ly0 + 1.)),
                                p(mid, ry1.max(ry0 + 1.)),
                                p(mid, ly1.max(ly0 + 1.)),
                            );
                            b.close();
                            if let Ok(path) = b.build() {
                                window.paint_path(path, h.color());
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (a, b) = match args.as_slice() {
        [_, a, b, ..] => (
            std::fs::read_to_string(a).unwrap(),
            std::fs::read_to_string(b).unwrap(),
        ),
        _ => {
            eprintln!("bruk: gpui-trakt <venstre> <høyre>");
            std::process::exit(1);
        }
    };

    Application::new().run(move |cx| {
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            |_, cx| cx.new(|_| DiffView::new(&a, &b)),
        )
        .unwrap();
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
}
