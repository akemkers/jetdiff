//! The scrollbar: the thumb shows what is visible, and the hunk minimap shows where the hunks are.

use jetdiff_viewmodel::StripeMarker;
use jetdiff_viewmodel::stripe::thumb_range;
use ratatui::layout::Rect;

use super::painter::Painter;

pub(super) fn paint(painter: &mut Painter, x: u16, area: Rect) {
    let markers = painter.markers();
    let track = Track {
        height: area.height as f32,
        content: painter.content_height(),
    };
    let thumb = track.thumb(painter.scroll_y());
    for row in 0..area.height {
        let in_thumb = thumb.is_some_and(|(start, end)| track.covers(row, start, end));
        let marker = markers.iter().find(|marker| track.shows(row, marker));
        painter.scrollbar_cell(
            x,
            area.y + row,
            in_thumb,
            marker.map(|marker| marker.diff_type),
        );
    }
}

/// The scrollbar as rows. The content is scaled down when it is taller than the track.
struct Track {
    height: f32,
    content: f32,
}

impl Track {
    fn thumb(&self, scroll_y: f32) -> Option<(f32, f32)> {
        thumb_range(scroll_y, self.height, self.content, self.height)
    }

    fn to_track(&self, y: f32) -> f32 {
        if self.content <= self.height {
            y
        } else {
            y / self.content * self.height
        }
    }

    /// Row `row` overlaps `start..end` in the track.
    fn covers(&self, row: u16, start: f32, end: f32) -> bool {
        start < row as f32 + 1. && end > row as f32
    }

    /// The mark shows in row `row`, at least one row high.
    fn shows(&self, row: u16, marker: &StripeMarker) -> bool {
        let start = self.to_track(marker.start_y);
        self.covers(row, start, self.to_track(marker.end_y).max(start + 1.))
    }
}
