//! `DiffLineSeparatorRenderer`: a fold wave over the gutter and text on each closed fold, the
//! same in Split and Unified.

use std::ops::Range;

use gpui::{Hsla, Window, point, px};
use jetdiff_viewmodel::fold_paths::{fold_wave_path, stroke_width};
use jetdiff_viewmodel::{FoldId, LineLayout, VisualFold};

use super::divider::stroke_path;
use super::visible::Visible;

pub struct FoldWaves<'a> {
    pub layout: &'a LineLayout<'a>,
    pub x: Range<f32>,
    /// The phase is counted from here, so in Split the wave meets the fold link at the divider.
    pub edge: f32,
    pub mirrored: bool,
    /// The y on screen of the top of the document.
    pub document_top: f32,
    pub hovered: Option<FoldId>,
    pub color: Hsla,
}

impl FoldWaves<'_> {
    pub fn paint<'f>(
        &self,
        folds: impl IntoIterator<Item = (FoldId, &'f VisualFold)>,
        visible: &Visible,
        window: &mut Window,
    ) {
        let line_height = self.layout.line_height();
        for (id, fold) in folds {
            if fold.visual < *visible.visual.start() {
                continue;
            }
            if fold.visual > *visible.visual.end() {
                break;
            }
            let y = self.document_top + self.layout.visual_line_to_y(fold.visual);
            let path = fold_wave_path(self.x.clone(), self.edge, self.mirrored, y, line_height);
            let width = stroke_width(self.hovered == Some(id));
            let to_pixels = |p: jetdiff_viewmodel::Point| point(px(p.x), px(p.y));
            stroke_path(window, &path, to_pixels, width, self.color);
        }
    }
}
