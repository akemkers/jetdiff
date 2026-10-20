//! Unified diff: én editor der slettede linjer står over de nye i hver endring. Port av
//! `UnifiedFragmentBuilder`, men radene peker inn i `FileDiff` i stedet for å bygge ny
//! tekst, så syntaks og ord-markering gjenbrukes. Ingen folding. `aligned` gir samme rader med
//! slettet og ny linje side om side, for split i terminalen.

use std::ops::Range;
use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::columns;
use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::{Block, DiffType, FileDiff, idx};
use crate::nav::target_offsets;
use crate::scroll::ScrollState;
use crate::scrolloff;
use crate::state::{DiffState, Horizontal, Outcome};
use crate::stripe::{StripeMarker, marker_y};

/// `h` og `l` scroller så mange tegn, som i split.
const SCROLL_COLUMNS: f32 = 4.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// Linja på hver side. Slettet er `[Some, None]`, ny er `[None, Some]` og uendret har
    /// begge. Nye og slettede filer har bare den ene siden.
    pub lines: [Option<usize>; 2],
    pub block: Option<usize>,
}

impl Row {
    /// Siden teksten hentes fra. Uendrede linjer tas fra høyre, som `UnifiedFragmentBuilder`.
    pub fn side(&self) -> Side {
        if self.lines[1].is_some() {
            Side::Right
        } else {
            Side::Left
        }
    }

    pub fn line(&self) -> usize {
        self.lines[idx(self.side())].unwrap_or(0)
    }

    /// Slettede rader får slettet-farge og nye får ny-farge, også på ordnivå. Unified har
    /// ikke `Modified`.
    pub fn diff_type(&self) -> DiffType {
        match self.side() {
            Side::Left => DiffType::Deleted,
            Side::Right => DiffType::Inserted,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    pub diff: Rc<FileDiff>,
    rows: Vec<Row>,
    /// Raden hver linje står på, per side. Uendrede linjer på venstre side peker på samme
    /// rad som linja på høyre.
    row_of: [Vec<usize>; 2],
    /// Radene til hver blokk.
    block_rows: Vec<Range<usize>>,
    aligned: bool,
}

impl UnifiedDiff {
    pub fn new(diff: Rc<FileDiff>) -> Self {
        Self::build(diff, false)
    }

    /// Linjene i en endring står parvis, og den korteste siden får rader uten linje.
    pub fn aligned(diff: Rc<FileDiff>) -> Self {
        Self::build(diff, true)
    }

    pub fn is_aligned(&self) -> bool {
        self.aligned
    }

    fn build(diff: Rc<FileDiff>, aligned: bool) -> Self {
        let line_counts = [diff.line_count(Side::Left), diff.line_count(Side::Right)];
        let mut rows = Vec::with_capacity(line_counts[0] + line_counts[1]);
        let mut row_of = line_counts.map(|count| vec![0; count]);
        let mut block_rows = Vec::with_capacity(diff.blocks().len());
        let mut push = |lines: [Option<usize>; 2], block| {
            for side in [Side::Left, Side::Right] {
                if let Some(line) = lines[idx(side)] {
                    row_of[idx(side)][line] = rows.len();
                }
            }
            rows.push(Row { lines, block });
            rows.len()
        };

        let mut next = [0, 0];
        let mut len = 0;
        for (index, block) in diff.blocks().iter().enumerate() {
            let [left, right] = &block.lines;
            for (left, right) in (next[0]..left.start).zip(next[1]..right.start) {
                len = push([Some(left), Some(right)], None);
            }
            let start = len;
            if aligned {
                for lines in pair_lines(&diff, block) {
                    len = push(lines, Some(index));
                }
            } else {
                for line in left.clone() {
                    len = push([Some(line), None], Some(index));
                }
                for line in right.clone() {
                    len = push([None, Some(line)], Some(index));
                }
            }
            block_rows.push(start..len);
            next = [left.end, right.end];
        }
        for (left, right) in (next[0]..line_counts[0]).zip(next[1]..line_counts[1]) {
            push([Some(left), Some(right)], None);
        }

        // Den tomme siden har én tom linje, og den skal ikke ha nummer
        if let Some(shown) = diff.single_side() {
            for row in &mut rows {
                row.lines[idx(shown.other())] = None;
            }
        }
        Self {
            diff,
            rows,
            row_of,
            block_rows,
            aligned,
        }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn row(&self, row: usize) -> &Row {
        &self.rows[row]
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn row_of(&self, side: Side, line: usize) -> usize {
        let lines = &self.row_of[idx(side)];
        lines[line.min(lines.len() - 1)]
    }

    pub fn block_rows(&self) -> &[Range<usize>] {
        &self.block_rows
    }

    pub fn text(&self, row: usize) -> &str {
        let row = self.row(row);
        self.diff.line(row.side(), row.line())
    }

    pub fn layout(&self, line_height: f32) -> LineLayout<'static> {
        LineLayout::new(self.row_count(), line_height)
    }

    /// To markører for en endret blokk: én for slettet del og én for ny del. `aligned` har én
    /// per blokk.
    pub fn stripe_markers(&self, line_height: f32) -> Vec<StripeMarker> {
        let layout = self.layout(line_height);
        let mut markers = Vec::new();
        for (rows, block) in self.block_rows.iter().zip(self.diff.blocks()) {
            if self.aligned {
                let (start_y, end_y) = marker_y(&layout, rows.clone());
                markers.push(StripeMarker {
                    diff_type: block.diff_type,
                    start_y,
                    end_y,
                });
                continue;
            }
            let deleted = self.rows[rows.clone()]
                .iter()
                .take_while(|row| row.side() == Side::Left)
                .count();
            let first_inserted = rows.start + deleted;
            for (part, diff_type) in [
                (rows.start..first_inserted, DiffType::Deleted),
                (first_inserted..rows.end, DiffType::Inserted),
            ] {
                if part.is_empty() {
                    continue;
                }
                let (start_y, end_y) = marker_y(&layout, part);
                markers.push(StripeMarker {
                    diff_type,
                    start_y,
                    end_y,
                });
            }
        }
        markers
    }
}

/// Linjer i en endring som har uendret tekst felles står på samme rad. Resten parres i
/// rekkefølge, og den korteste siden får rader uten linje.
fn pair_lines(diff: &FileDiff, block: &Block) -> Vec<[Option<usize>; 2]> {
    let [left, right] = &block.lines;
    let mut rows = Vec::new();
    let zip = |l: Range<usize>, r: Range<usize>, rows: &mut Vec<[Option<usize>; 2]>| {
        let at = |lines: &Range<usize>, i: usize| (i < lines.len()).then(|| lines.start + i);
        for i in 0..l.len().max(r.len()) {
            rows.push([at(&l, i), at(&r, i)]);
        }
    };
    let mut anchors: Vec<(usize, usize)> = Vec::new();
    if let Some(inner) = &block.inner {
        let texts = [Side::Left, Side::Right].map(|s| &diff.text(s)[block.offsets[idx(s)].clone()]);
        let ends = [texts[0].len(), texts[1].len()];
        let changed = inner
            .iter()
            .map(|f| [f.offsets1.clone(), f.offsets2.clone()])
            .chain([[ends[0]..ends[0], ends[1]..ends[1]]]);
        let mut equal_from = [0, 0];
        for [l, r] in changed {
            let equal = (l.start - equal_from[0]).min(r.start - equal_from[1]);
            let segment = &texts[0][equal_from[0]..equal_from[0] + equal];
            for (k, _) in segment.char_indices().filter(|(_, c)| !c.is_whitespace()) {
                let pair = (
                    line_at(diff, Side::Left, left, equal_from[0] + k),
                    line_at(diff, Side::Right, right, equal_from[1] + k),
                );
                if anchors
                    .last()
                    .is_none_or(|&(al, ar)| pair.0 > al && pair.1 > ar)
                {
                    anchors.push(pair);
                }
            }
            equal_from = [l.end, r.end];
        }
    }
    let (mut l, mut r) = (left.start, right.start);
    for (al, ar) in anchors {
        zip(l..al, r..ar, &mut rows);
        rows.push([Some(al), Some(ar)]);
        (l, r) = (al + 1, ar + 1);
    }
    zip(l..left.end, r..right.end, &mut rows);
    rows
}

/// Linja et byte-offset fra starten av blokken står på.
fn line_at(diff: &FileDiff, side: Side, lines: &Range<usize>, offset: usize) -> usize {
    let mut start = 0;
    for line in lines.clone() {
        start += diff.line(side, line).len() + 1;
        if offset < start {
            return line;
        }
    }
    lines.end.saturating_sub(1).max(lines.start)
}

#[derive(Debug, Clone)]
pub struct UnifiedState {
    pub diff: Rc<UnifiedDiff>,
    /// Markert rad.
    pub row: usize,
    /// Begge y er like, så animasjon og tegning kan dele typen med split.
    pub scroll: ScrollState,
}

impl UnifiedState {
    pub fn new(diff: Rc<UnifiedDiff>) -> Self {
        Self {
            diff,
            row: 0,
            scroll: ScrollState::default(),
        }
    }

    /// Fra split. Cursorens side og linje beholdes, og linja står på samme høyde i
    /// viewporten.
    pub fn from_split(split: &DiffState, viewport: Viewport) -> Self {
        let cursor = split.cursor;
        let layout = split.folding.layout(cursor.side, viewport.line_height);
        let offset = layout.line_top(cursor.line) - split.scroll.y(cursor.side);
        let diff = Rc::new(UnifiedDiff::new(split.diff.clone()));
        Self::place(diff, cursor, offset, split.scroll.x, viewport)
    }

    /// Bytter mellom unified og `aligned` på samme måte som mot split.
    pub fn realign(&self, aligned: bool, viewport: Viewport) -> Self {
        let row = self.diff.row(self.row);
        let cursor = Cursor::new(row.side(), row.line());
        let offset = self.row as f32 * viewport.line_height - self.scroll_y();
        let diff = self.diff.diff.clone();
        let diff = Rc::new(if aligned {
            UnifiedDiff::aligned(diff)
        } else {
            UnifiedDiff::new(diff)
        });
        Self::place(diff, cursor, offset, self.scroll.x, viewport)
    }

    /// Linja står `offset` under toppen av viewporten.
    fn place(
        diff: Rc<UnifiedDiff>,
        cursor: Cursor,
        offset: f32,
        scroll_x: f32,
        viewport: Viewport,
    ) -> Self {
        let mut state = Self::new(diff);
        state.row = state.diff.row_of(cursor.side, cursor.line);
        state.scroll.x = scroll_x;
        let y = state.row as f32 * viewport.line_height - offset;
        state.set_scroll_y(y, viewport);
        state.follow(viewport, false);
        state
    }

    /// Til split. Slettede rader gir venstre side, alt annet høyre.
    pub fn to_split(&self, folding: Rc<Folding>, viewport: Viewport) -> DiffState {
        let row = self.diff.row(self.row);
        let offset = self.row as f32 * viewport.line_height - self.scroll_y();
        let mut state = DiffState::new(self.diff.diff.clone(), folding);
        state.scroll.x = self.scroll.x;
        let cursor = Cursor::new(row.side(), row.line());
        state.place_cursor(cursor, offset, viewport);
        state
    }

    /// Første endring, som `DiffState::init`.
    pub fn init(&mut self, viewport: Viewport) {
        if self.diff.block_rows.is_empty() {
            return;
        }
        let (row, y) = self.target_for_block(0, viewport);
        self.row = row;
        self.scroll = ScrollState { y: [y, y], x: 0. };
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll.y[0]
    }

    fn layout(&self, viewport: Viewport) -> LineLayout<'static> {
        self.diff.layout(viewport.line_height)
    }

    fn last_row(&self) -> usize {
        self.diff.row_count() - 1
    }

    fn set_scroll_y(&mut self, y: f32, viewport: Viewport) {
        let y = self.layout(viewport).clamp_scroll(y, viewport.height);
        self.scroll.y = [y, y];
    }

    fn follow_y(&self, scroll_y: f32, viewport: Viewport, allow_center: bool) -> f32 {
        let layout = self.layout(viewport);
        scrolloff::follow_y(&layout, self.row, scroll_y, viewport, allow_center)
    }

    fn follow(&mut self, viewport: Viewport, allow_center: bool) {
        let y = self.follow_y(self.scroll_y(), viewport, allow_center);
        self.set_scroll_y(y, viewport);
    }

    /// Etter scroll uten cursor: flytt cursoren inn hvis scrolloff krever det.
    fn push_cursor(&mut self, viewport: Viewport) -> bool {
        let layout = self.layout(viewport);
        let (min, max) = scrolloff::cursor_range(&layout, self.scroll_y(), viewport);
        let row = self.row.clamp(min, max);
        if row == self.row {
            return false;
        }
        self.row = row;
        true
    }

    pub fn execute(
        &mut self,
        command: Command,
        viewport: Viewport,
        horizontal: Horizontal,
    ) -> Outcome {
        use Command::*;
        let columns = SCROLL_COLUMNS * horizontal.char_width;
        match command {
            MoveDown => self.move_row(1, viewport),
            MoveUp => self.move_row(-1, viewport),
            ScrollLeft => self.scroll_x(self.scroll.x - columns, horizontal),
            ScrollRight => self.scroll_x(self.scroll.x + columns, horizontal),
            ScrollToStart => self.scroll_x(0., horizontal),
            ScrollToEnd => self.scroll_to_end(horizontal),
            GoToTop => self.go_to_row(0, viewport),
            GoToBottom => self.go_to_row(self.last_row(), viewport),
            HalfPageDown => self.half_page(1, viewport),
            HalfPageUp => self.half_page(-1, viewport),
            NextChange => self.change(true, viewport),
            PrevChange => self.change(false, viewport),
            // Én side og ingen folding
            FocusSide(_) | FoldOpen | ToggleFolds => Outcome::default(),
            // Eies av UI-et, som i split
            CopyLocation | Rollback | ZoomIn | ZoomOut | ResetZoom => Outcome::default(),
        }
    }

    fn move_row(&mut self, delta: isize, viewport: Viewport) -> Outcome {
        let row = self.row.saturating_add_signed(delta).min(self.last_row());
        if row == self.row {
            return Outcome::default();
        }
        self.row = row;
        self.follow(viewport, true);
        Outcome::instant()
    }

    fn go_to_row(&mut self, row: usize, viewport: Viewport) -> Outcome {
        self.row = row;
        self.follow(viewport, true);
        Outcome::animated()
    }

    /// Scroll og cursor flyttes like mange rader, som `DiffState`.
    fn half_page(&mut self, direction: isize, viewport: Viewport) -> Outcome {
        let rows = (scrolloff::screen_lines(viewport) / 2).max(1);
        let scroll_y = self.scroll_y();
        self.row = self
            .row
            .saturating_add_signed(direction * rows as isize)
            .min(self.last_row());
        let moved = scroll_y + direction as f32 * rows as f32 * viewport.line_height;
        let y = self.follow_y(moved, viewport, false);
        self.set_scroll_y(y, viewport);
        Outcome::animated()
    }

    fn scroll_x(&mut self, x: f32, horizontal: Horizontal) -> Outcome {
        let before = self.scroll.x;
        self.scroll.scroll_x_to(x, horizontal.max_x);
        if self.scroll.x == before {
            return Outcome::default();
        }
        Outcome::instant()
    }

    /// Slutten av linja ett tegn inn fra høyre kant.
    fn scroll_to_end(&mut self, horizontal: Horizontal) -> Outcome {
        let cells = columns::display_width(self.diff.text(self.row));
        let x = (cells + 1) as f32 * horizontal.char_width - horizontal.text_width;
        self.scroll_x(x.max(0.), horizontal)
    }

    /// Samme landing som split: starten av blokken omtrent 1/3 ned.
    fn target_for_block(&self, block: usize, viewport: Viewport) -> (usize, f32) {
        let rows = self.diff.block_rows[block].clone();
        let layout = self.layout(viewport);
        let [y, _] = target_offsets(
            &[layout, layout],
            [rows.start; 2],
            [rows.end; 2],
            viewport.height,
        );
        (rows.start, y)
    }

    /// `n` går til første blokk under cursoren, `p` til siste blokk helt over.
    fn change(&mut self, next: bool, viewport: Viewport) -> Outcome {
        let blocks = &self.diff.block_rows;
        let block = if next {
            blocks.iter().position(|rows| rows.start > self.row)
        } else {
            blocks.iter().rposition(|rows| rows.end <= self.row)
        };
        let Some(block) = block else {
            return Outcome::default();
        };
        let (row, y) = self.target_for_block(block, viewport);
        self.row = row;
        self.scroll = ScrollState { y: [y, y], x: 0. };
        let followed = self.follow_y(y, viewport, false);
        if followed != y {
            self.set_scroll_y(followed, viewport);
        }
        Outcome::animated()
    }

    /// Blokken cursoren står i.
    pub fn cursor_block(&self) -> Option<usize> {
        self.diff.row(self.row).block
    }

    /// Markert linje i nyeste versjon, som `DiffState::location`. En slettet rad gir linja
    /// rett under på høyre side. Slettede filer har bare venstre side.
    pub fn location(&self) -> Cursor {
        let row = self.diff.row(self.row);
        let diff = &self.diff.diff;
        if row.side() == Side::Right || diff.single_side().is_some() {
            return Cursor::new(row.side(), row.line());
        }
        let line = diff.mapping().transfer(Side::Left, row.line());
        Cursor::new(Side::Right, line.min(diff.line_count(Side::Right) - 1))
    }

    /// Samme fil med nytt innhold. Cursoren blir på samme linje, og scroll beholdes så
    /// langt det går.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, viewport: Viewport) {
        let row = *self.diff.row(self.row);
        self.diff = Rc::new(UnifiedDiff::build(diff, self.diff.aligned));
        self.row = self.diff.row_of(row.side(), row.line());
        self.set_scroll_y(self.scroll_y(), viewport);
        self.follow(viewport, false);
    }

    /// Musehjul eller trackpad. Cursoren flyttes bare når scrolloff krever det.
    pub fn wheel(&mut self, delta_y: f32, viewport: Viewport) -> bool {
        self.set_scroll_y(self.scroll_y() + delta_y, viewport);
        self.push_cursor(viewport)
    }

    /// Klikk markerer raden uten å scrolle.
    pub fn click(&mut self, row: usize) {
        self.row = row.min(self.last_row());
    }

    pub fn resize(&mut self, viewport: Viewport) {
        self.set_scroll_y(self.scroll_y(), viewport);
        self.push_cursor(viewport);
    }

    /// Zoom: cursorraden holdes på samme sted i viewporten, målt i rader.
    pub fn rescale(&mut self, old_line_height: f32, viewport: Viewport, x_scale: f32) {
        let row = self.row as f32;
        let offset = row - self.scroll_y() / old_line_height;
        self.set_scroll_y((row - offset) * viewport.line_height, viewport);
        self.scroll.x *= x_scale;
        self.push_cursor(viewport);
    }
}

#[cfg(test)]
mod tests;
