//! Diffen som helhet: markert linje, folding og scroll. Utfører `Command` og holder
//! linja og scroll i takt med scrolloff. `scroll` er målet, UI-et animerer dit selv.

use std::rc::Rc;

use jetdiff_core::side::Side;

use crate::columns;
use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::layout::{LineLayout, Viewport};
use crate::model::{FileDiff, idx};
use crate::nav::{next_block, prev_block, target_for_block};
use crate::scroll::{PositionKeeper, ScrollState};

/// Vim sin `scrolloff`, så det alltid er kontekst rundt cursoren. IntelliJ har 0.
const SCROLLOFF: usize = 8;

/// `h` og `l` scroller så mange tegn.
const SCROLL_COLUMNS: f32 = 4.;

/// Liten margin mot flyttallsfeil når scroll står på en linjegrense.
const EPS: f32 = 1e-3;

/// Horisontal geometri fra UI-et, som kjenner tekstbredden.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Horizontal {
    pub text_width: f32,
    pub char_width: f32,
    pub max_x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcome {
    pub changed: bool,
    /// Scroll skal animeres til målet i stedet for å hoppe.
    pub animate: bool,
    pub folding_changed: bool,
}

impl Outcome {
    fn instant() -> Self {
        Self {
            changed: true,
            ..Self::default()
        }
    }

    fn animated() -> Self {
        Self {
            animate: true,
            ..Self::instant()
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffState {
    pub diff: Rc<FileDiff>,
    pub folding: Rc<Folding>,
    pub scroll: ScrollState,
    /// Siden sync scroll regnes fra. Følger cursoren, men musehjulet kan ta den.
    pub master: Side,
    pub cursor: Cursor,
}

/// Hele linjer i viewporten.
fn screen_lines(vp: Viewport) -> usize {
    (vp.height / vp.line_height + EPS).floor().max(0.) as usize
}

/// Vim klemmer scrolloff til halve vinduet, så cursoren alltid har et sted å stå.
fn scrolloff(vp: Viewport) -> usize {
    SCROLLOFF.min(screen_lines(vp).saturating_sub(1) / 2)
}

impl DiffState {
    pub fn new(diff: Rc<FileDiff>, folding: Rc<Folding>) -> Self {
        // `getCurrentSide` er RIGHT som default i SimpleDiffViewer
        let side = diff.single_side().unwrap_or(Side::Right);
        Self {
            diff,
            folding,
            scroll: ScrollState::default(),
            master: side,
            cursor: Cursor::new(side, 0),
        }
    }

    /// `InitialScrollPositionSupport`: første endring, med cursoren på høyre side.
    pub fn init(&mut self, vp: Viewport) {
        if self.diff.blocks().is_empty() {
            return;
        }
        let t = target_for_block(&self.diff, &self.folding, 0, vp);
        self.scroll.y = t.scroll_y;
        self.scroll.x = t.scroll_x;
        let side = self.cursor.side;
        self.cursor = Cursor::new(side, self.clamp_line(side, t.caret_lines[idx(side)]));
    }

    fn layout(&self, side: Side, vp: Viewport) -> LineLayout<'_> {
        self.folding.layout(side, vp.line_height)
    }

    fn text(&self, side: Side, line: usize) -> &str {
        self.diff.line(side, line)
    }

    /// Gyldig linje utenfor kollapsede folds.
    fn clamp_line(&self, side: Side, line: usize) -> usize {
        let line = line.min(self.diff.line_count(side) - 1);
        self.folding.caret_after(side, line)
    }

    fn cursor_visual(&self, vp: Viewport) -> usize {
        self.layout(self.cursor.side, vp)
            .logical_to_visual(self.cursor.line)
    }

    fn set_line(&mut self, line: usize) {
        self.cursor.line = self.clamp_line(self.cursor.side, line);
    }

    fn set_visual(&mut self, visual: usize, vp: Viewport) {
        let layout = self.layout(self.cursor.side, vp);
        let visual = visual.min(layout.visual_line_count() - 1);
        let line = layout.visual_to_logical(visual);
        self.set_line(line);
    }

    /// Første og siste hele visuelle linje med tekst i viewporten.
    fn full_lines(&self, sy: f32, vp: Viewport) -> (usize, usize) {
        let last_line = self.layout(self.cursor.side, vp).visual_line_count() - 1;
        let first = ((sy / vp.line_height - EPS).ceil().max(0.) as usize).min(last_line);
        let last = ((sy + vp.height) / vp.line_height + EPS).floor() as usize;
        (first, last.saturating_sub(1).clamp(first, last_line))
    }

    /// Visuelle linjer cursoren kan stå på uten at det scrolles. Øverst og nederst i
    /// fila slippes scrolloff, ellers kunne cursoren aldri nå første og siste linje.
    fn cursor_range(&self, sy: f32, vp: Viewport) -> (usize, usize) {
        let layout = self.layout(self.cursor.side, vp);
        let so = scrolloff(vp);
        let last = layout.visual_line_count() - 1;
        let (first_full, last_full) = self.full_lines(sy, vp);
        let top = if sy <= EPS { 0 } else { so };
        let bottom = if sy >= layout.max_scroll(vp.height) - EPS {
            0
        } else {
            so
        };
        let min = (first_full + top).min(last);
        let max = last_full.saturating_sub(bottom).max(min);
        (min, max)
    }

    /// Minste scroll som gir cursoren scrolloff. Er hoppet langt, sentreres linja som
    /// vim sin `scroll_cursor_halfway`.
    fn follow_y(&self, sy: f32, vp: Viewport, allow_center: bool) -> f32 {
        let layout = self.layout(self.cursor.side, vp);
        let lh = vp.line_height;
        let so = scrolloff(vp) as f32;
        let cv = self.cursor_visual(vp) as f32;
        let lo = (cv + 1. + so) * lh - vp.height;
        let hi = (cv - so) * lh;
        let mut y = sy;
        if y < lo - EPS {
            y = lo;
        }
        if y > hi + EPS {
            y = hi;
        }
        // Vim sentrerer når det må scrolles en hel skjerm ned eller en halv opp
        let far = (y > sy && y - sy >= vp.height) || (y < sy && sy - y >= vp.height / 2.);
        if allow_center && far {
            y = self.centered_y(vp);
        }
        layout.clamp_scroll(y, vp.height)
    }

    fn centered_y(&self, vp: Viewport) -> f32 {
        let cv = self.cursor_visual(vp) as f32;
        let above = (screen_lines(vp).saturating_sub(1) / 2) as f32;
        (cv - above) * vp.line_height
    }

    /// Cursoren eier master-siden. Slave følger via sync.
    fn scroll_cursor_side_to(&mut self, y: f32, vp: Viewport) {
        let side = self.cursor.side;
        self.master = side;
        self.scroll
            .scroll_to(&self.diff, &self.folding, side, y, vp);
    }

    fn follow(&mut self, vp: Viewport, allow_center: bool) {
        let y = self.follow_y(self.scroll.y(self.cursor.side), vp, allow_center);
        self.scroll_cursor_side_to(y, vp);
    }

    fn scroll_x(&mut self, x: f32, hx: Horizontal) -> Outcome {
        let before = self.scroll.x;
        self.scroll.scroll_x_to(x, hx.max_x);
        if self.scroll.x == before {
            return Outcome::default();
        }
        Outcome::instant()
    }

    /// Slutten av linja ett tegn inn fra høyre kant. En kollapset fold har ingen tekst.
    fn scroll_to_end(&mut self, hx: Horizontal) -> Outcome {
        let c = self.cursor;
        let cells = if self.folding.fold_at_line(c.side, c.line).is_some() {
            0
        } else {
            columns::display_width(self.text(c.side, c.line))
        };
        let x = (cells + 1) as f32 * hx.char_width - hx.text_width;
        self.scroll_x(x.max(0.), hx)
    }

    /// Etter scroll uten cursor: flytt cursoren inn hvis scrolloff krever det.
    fn push_cursor(&mut self, vp: Viewport) -> bool {
        let (min, max) = self.cursor_range(self.scroll.y(self.cursor.side), vp);
        let cv = self.cursor_visual(vp);
        let target = cv.clamp(min, max);
        if target == cv {
            return false;
        }
        self.set_visual(target, vp);
        true
    }

    pub fn execute(&mut self, cmd: Command, vp: Viewport, hx: Horizontal) -> Outcome {
        use Command::*;
        match cmd {
            MoveDown => self.move_visual(1, vp),
            MoveUp => self.move_visual(-1, vp),
            ScrollLeft => self.scroll_x(self.scroll.x - SCROLL_COLUMNS * hx.char_width, hx),
            ScrollRight => self.scroll_x(self.scroll.x + SCROLL_COLUMNS * hx.char_width, hx),
            ScrollToStart => self.scroll_x(0., hx),
            ScrollToEnd => self.scroll_to_end(hx),
            GoToTop => self.go_to_line(0, vp),
            GoToBottom => {
                let last = self.diff.line_count(self.cursor.side) - 1;
                self.go_to_line(last, vp)
            }
            HalfPageDown => self.half_page(1, vp),
            HalfPageUp => self.half_page(-1, vp),
            NextChange => self.change(true, vp),
            PrevChange => self.change(false, vp),
            FocusSide(side) => self.focus_side(side, vp),
            FoldOpen => self.change_folding(vp, |s, f| f.expand_at(s.0, s.1)),
            ToggleFolds => self.change_folding(vp, |_, f| {
                let any_closed = f.visible(Side::Left).next().is_some();
                f.expand_all(any_closed)
            }),
            // Eies av UI-et. Etter zoom kalles `rescale`.
            CopyLocation | Rollback | ZoomIn | ZoomOut | ResetZoom => Outcome::default(),
        }
    }

    fn move_visual(&mut self, delta: isize, vp: Viewport) -> Outcome {
        let cv = self.cursor_visual(vp);
        let target = cv.saturating_add_signed(delta);
        let before = self.cursor;
        self.set_visual(target, vp);
        if self.cursor == before {
            return Outcome::default();
        }
        self.follow(vp, true);
        Outcome::instant()
    }

    fn go_to_line(&mut self, line: usize, vp: Viewport) -> Outcome {
        self.set_line(line);
        self.follow(vp, true);
        Outcome::animated()
    }

    /// `d` og `u`: scroll og cursor flyttes like mange linjer, vim sin `scroll`.
    fn half_page(&mut self, dir: isize, vp: Viewport) -> Outcome {
        let n = (screen_lines(vp) / 2).max(1);
        let side = self.cursor.side;
        let sy = self.scroll.y(side);
        let cv = self.cursor_visual(vp);
        self.set_visual(cv.saturating_add_signed(dir * n as isize), vp);
        let y = self.follow_y(sy + dir as f32 * n as f32 * vp.line_height, vp, false);
        self.scroll_cursor_side_to(y, vp);
        Outcome::animated()
    }

    /// `n` og `p` bruker IntelliJ sin navigasjon og landing, så cursoren til starten av
    /// endringen. Scroll justeres bare hvis landingen bryter scrolloff.
    fn change(&mut self, next: bool, vp: Viewport) -> Outcome {
        let side = self.cursor.side;
        let block = if next {
            next_block(&self.diff, side, self.cursor.line)
        } else {
            prev_block(&self.diff, side, self.cursor.line)
        };
        let Some(block) = block else {
            return Outcome::default();
        };
        let t = target_for_block(&self.diff, &self.folding, block, vp);
        let line = self.clamp_line(side, t.caret_lines[idx(side)]);
        self.cursor = Cursor::new(side, line);
        self.master = side;
        self.scroll.y = t.scroll_y;
        self.scroll.x = t.scroll_x;
        let y = self.follow_y(self.scroll.y(side), vp, false);
        if y != self.scroll.y(side) {
            self.scroll_cursor_side_to(y, vp);
        }
        Outcome::animated()
    }

    /// Linja på den andre siden, via `BaseSyncScrollable.transfer`.
    fn transferred_line(&self) -> usize {
        let c = self.cursor;
        let other = c.side.other();
        let line = self.diff.mapping().transfer(c.side, c.line);
        self.clamp_line(other, line)
    }

    /// `TwosideTextDiffViewer.transferPosition`, uten kolonne.
    pub fn transferred_cursor(&self) -> Cursor {
        Cursor::new(self.cursor.side.other(), self.transferred_line())
    }

    /// Blokken cursoren står i. `None` på en kollapset fold.
    pub fn cursor_block(&self) -> Option<usize> {
        let c = self.cursor;
        if self.folding.fold_at_line(c.side, c.line).is_some() {
            return None;
        }
        self.diff.block_at(c.side, c.line)
    }

    /// Samme fil med nytt innhold. Cursor og scroll beholdes så langt det går.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, vp: Viewport) {
        self.folding = Rc::new(Folding::new(&diff));
        self.diff = diff;
        let side = self.diff.single_side().unwrap_or(self.cursor.side);
        self.cursor = Cursor::new(side, self.clamp_line(side, self.cursor.line));
        self.master = side;
        let y = self.scroll.y(side);
        self.scroll.y[idx(side)] = self.layout(side, vp).clamp_scroll(y, vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, side, vp);
        self.follow(vp, false);
    }

    /// Markert linje i nyeste versjon. Slettede filer har bare venstre side.
    pub fn location(&self) -> Cursor {
        if self.cursor.side == Side::Right || self.diff.single_side().is_some() {
            self.cursor
        } else {
            self.transferred_cursor()
        }
    }

    /// `MyFocusOppositePaneAction`: cursoren flyttes, siden blir master, og så
    /// `scrollToCaret(MAKE_VISIBLE)`.
    fn focus_side(&mut self, side: Side, vp: Viewport) -> Outcome {
        if self.diff.single_side().is_some() || self.cursor.side == side {
            return Outcome::default();
        }
        self.cursor = self.transferred_cursor();
        self.follow(vp, false);
        Outcome::animated()
    }

    /// `runBatchFoldingOperation` med `PositionKeeper`. Den andre sidens caret er
    /// overført posisjon, siden vi bare har én cursor.
    fn change_folding(
        &mut self,
        vp: Viewport,
        op: impl FnOnce((Side, usize), &mut Folding) -> bool,
    ) -> Outcome {
        let side = self.cursor.side;
        let mut carets = [0; 2];
        carets[idx(side)] = self.cursor.line;
        carets[idx(side.other())] = self.transferred_line();
        let keeper = PositionKeeper::save(&self.scroll, &self.folding, carets, vp);
        if !op((side, self.cursor.line), Rc::make_mut(&mut self.folding)) {
            return Outcome::default();
        }
        let after = [Side::Left, Side::Right].map(|s| self.folding.caret_after(s, carets[idx(s)]));
        self.set_line(after[idx(side)]);
        keeper.restore(&mut self.scroll, &self.diff, &self.folding, after, vp);
        Outcome {
            folding_changed: true,
            ..Outcome::instant()
        }
    }

    /// Musehjul eller trackpad på en side. Cursoren flyttes bare når scrolloff krever det.
    pub fn wheel(&mut self, side: Side, dy: f32, vp: Viewport) -> bool {
        self.master = side;
        self.scroll
            .scroll_by(&self.diff, &self.folding, side, dy, vp);
        self.push_cursor(vp)
    }

    /// Klikk setter side og linje uten å scrolle.
    pub fn click(&mut self, side: Side, line: usize) {
        self.master = side;
        self.cursor = Cursor::new(side, self.clamp_line(side, line));
    }

    /// Ny viewport-høyde: klem master, regn slave på nytt og hold cursoren synlig.
    pub fn resize(&mut self, vp: Viewport) {
        let master = self.master;
        let layout = self.layout(master, vp);
        self.scroll.y[idx(master)] = layout.clamp_scroll(self.scroll.y(master), vp.height);
        self.scroll.sync_from(&self.diff, &self.folding, master, vp);
        self.push_cursor(vp);
    }

    /// Zoom: cursorlinja holdes på samme sted i viewporten, målt i linjer.
    pub fn rescale(&mut self, old_line_height: f32, vp: Viewport, x_scale: f32) {
        let side = self.cursor.side;
        let cv = self.cursor_visual(vp) as f32;
        let offset = cv - self.scroll.y(side) / old_line_height;
        let y = (cv - offset) * vp.line_height;
        let y = self.layout(side, vp).clamp_scroll(y, vp.height);
        self.master = side;
        self.scroll.y[idx(side)] = y;
        self.scroll.sync_from(&self.diff, &self.folding, side, vp);
        self.scroll.x *= x_scale;
        self.push_cursor(vp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Command::*;
    use jetdiff_core::ComparisonPolicy;

    const L: Side = Side::Left;
    const R: Side = Side::Right;
    const LH: f32 = 10.;
    /// 20 hele linjer.
    const VP: Viewport = Viewport {
        height: 200.,
        line_height: LH,
    };
    const NO_X: Horizontal = Horizontal {
        text_width: 0.,
        char_width: 0.,
        max_x: 0.,
    };

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix}{i}\n")).collect()
    }

    fn state(a: &str, b: &str, folds: bool) -> DiffState {
        let diff = FileDiff::new(a, b, ComparisonPolicy::Default).unwrap();
        let folding = if folds {
            Folding::new(&diff)
        } else {
            Folding::none(&diff)
        };
        DiffState::new(Rc::new(diff), Rc::new(folding))
    }

    /// Like filer på 200 linjer, ingen folds.
    fn plain() -> DiffState {
        let a = lines("a", 200);
        state(&a, &a, false)
    }

    /// Endringer på linjene 10, 40 og 90 av 100, én linje hver.
    fn three_changes(folds: bool) -> DiffState {
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [10, 40, 90] {
            b[i] = format!("x{i}");
        }
        state(&a, &(b.join("\n") + "\n"), folds)
    }

    fn run(s: &mut DiffState, cmd: Command) -> Outcome {
        s.execute(cmd, VP, NO_X)
    }

    fn times(s: &mut DiffState, cmd: Command, n: usize) {
        for _ in 0..n {
            run(s, cmd);
        }
    }

    fn top(s: &DiffState) -> f32 {
        s.scroll.y(s.cursor.side)
    }

    /// Cursoren har scrolloff, eller står ved kanten av fila.
    fn assert_scrolloff(s: &DiffState) {
        let (min, max) = s.cursor_range(top(s), VP);
        let cv = s.cursor_visual(VP);
        assert!(min <= cv && cv <= max, "cursor {cv} utenfor {min}..={max}");
    }

    #[test]
    fn j_og_k() {
        let mut s = plain();
        times(&mut s, MoveDown, 3);
        assert_eq!(s.cursor.line, 3);
        times(&mut s, MoveUp, 10);
        assert_eq!(s.cursor.line, 0);
        assert!(!run(&mut s, MoveUp).changed);
        run(&mut s, GoToBottom);
        assert_eq!(s.cursor.line, 200);
        assert!(!run(&mut s, MoveDown).changed);
    }

    #[test]
    fn scrolloff_ved_j() {
        let mut s = plain();
        times(&mut s, MoveDown, 11);
        assert_eq!(top(&s), 0.);
        // Linje 12 er åtte fra bunnen av 20
        run(&mut s, MoveDown);
        assert_eq!(top(&s), 10.);
        assert_scrolloff(&s);
        times(&mut s, MoveUp, 10);
        assert_eq!(s.cursor.line, 2);
        assert_eq!(top(&s), 0.);
    }

    #[test]
    fn scrolloff_klemmes_i_lite_vindu() {
        let small = Viewport {
            height: 50.,
            line_height: LH,
        };
        assert_eq!(scrolloff(small), 2);
        assert_eq!(scrolloff(VP), 8);
        assert_eq!(
            scrolloff(Viewport {
                height: 5.,
                line_height: LH
            }),
            0
        );
        let mut s = plain();
        for _ in 0..30 {
            s.execute(MoveDown, small, NO_X);
            let (min, max) = s.cursor_range(top(&s), small);
            let cv = s.cursor_visual(small);
            assert!(min <= cv && cv <= max);
        }
    }

    #[test]
    fn gg_og_g() {
        let mut s = plain();
        let out = run(&mut s, GoToBottom);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 200);
        assert_eq!(top(&s), s.layout(R, VP).max_scroll(VP.height));
        run(&mut s, GoToTop);
        assert_eq!((s.cursor.line, top(&s)), (0, 0.));
    }

    #[test]
    fn langt_hopp_sentrerer() {
        let mut s = plain();
        s.click(R, 100);
        s.follow(VP, true);
        // 9 linjer over cursoren i et vindu på 20, som vim sin `scroll_cursor_halfway`
        assert_eq!(top(&s), 910.);
        assert_scrolloff(&s);
    }

    #[test]
    fn halv_side_flytter_scroll_og_cursor_likt() {
        let mut s = plain();
        times(&mut s, MoveDown, 10);
        let out = run(&mut s, HalfPageDown);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 20);
        assert_eq!(top(&s), 100.);
        run(&mut s, HalfPageDown);
        assert_eq!((s.cursor.line, top(&s)), (30, 200.));
        run(&mut s, HalfPageUp);
        run(&mut s, HalfPageUp);
        assert_eq!((s.cursor.line, top(&s)), (10, 0.));
        // Øverst kan ikke scroll gå lenger, men cursoren går
        run(&mut s, HalfPageUp);
        assert_eq!((s.cursor.line, top(&s)), (0, 0.));
    }

    #[test]
    fn neste_og_forrige_endring() {
        let mut s = three_changes(false);
        s.init(VP);
        assert_eq!((s.cursor.side, s.cursor.line), (R, 10));
        let out = run(&mut s, NextChange);
        assert!(out.animate);
        assert_eq!(s.cursor.line, 40);
        assert_scrolloff(&s);
        run(&mut s, PrevChange);
        assert_eq!(s.cursor.line, 10);
        times(&mut s, NextChange, 2);
        assert_eq!(s.cursor.line, 90);
        assert!(!run(&mut s, NextChange).changed);
        times(&mut s, PrevChange, 2);
        assert_eq!(s.cursor.line, 10);
        assert!(!run(&mut s, PrevChange).changed);
    }

    #[test]
    fn endring_lander_der_intellij_lander() {
        // Høyt nok vindu til at landingen ikke bryter scrolloff
        let vp = Viewport {
            height: 400.,
            line_height: LH,
        };
        let mut s = three_changes(false);
        s.init(vp);
        s.execute(NextChange, vp, NO_X);
        let t = target_for_block(&s.diff, &s.folding, 1, vp);
        assert_eq!(s.scroll.y, t.scroll_y);
    }

    #[test]
    fn plassering_er_i_nyeste_versjon() {
        let mut s = state("x\nny1\nny2\nfelles\n", "x\nfelles\n", false);
        s.click(L, 3);
        assert_eq!(s.location(), Cursor::new(R, 1));
        s.click(R, 0);
        assert_eq!(s.location(), Cursor::new(R, 0));
        let s = state("a\nb\n", "", false);
        assert_eq!(s.location(), Cursor::new(L, 0));
    }

    #[test]
    fn slettet_fil_vises_alene() {
        let mut s = state("a\nb\n", "", false);
        assert_eq!((s.cursor.side, s.master), (L, L));
        run(&mut s, FocusSide(R));
        assert_eq!(s.cursor.side, L);
    }

    #[test]
    fn bytte_side_overforer_linja() {
        // Venstre har tre linjer mer i endringen
        let a = "x\nny1\nny2\nny3\nfelles æøå\ny\n";
        let b = "x\nfelles æøå\ny\n";
        let mut s = state(a, b, false);
        s.click(L, 4);
        assert!(!run(&mut s, FocusSide(L)).animate);
        let out = run(&mut s, FocusSide(R));
        assert!(out.animate);
        assert_eq!((s.cursor.side, s.cursor.line), (R, 1));
        assert_eq!(s.master, R);
        // Linje i en blokk som er tom på høyre side går til linja etter
        s.click(L, 2);
        run(&mut s, FocusSide(R));
        assert_eq!((s.cursor.side, s.cursor.line), (R, 1));
        // `transferLine` på starten av en tom blokk gir starten av blokken på andre siden
        run(&mut s, FocusSide(L));
        assert_eq!((s.cursor.side, s.cursor.line), (L, 1));
    }

    #[test]
    fn j_hopper_over_kollapset_fold() {
        let mut s = three_changes(true);
        // Første fold er 0..6, så linje 6
        assert_eq!(s.cursor.line, 0);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 6);
        run(&mut s, MoveUp);
        assert_eq!(s.cursor.line, 0);
        // Fra 14 til folden 15..36 og videre til 36
        s.click(R, 14);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 15);
        run(&mut s, MoveDown);
        assert_eq!(s.cursor.line, 36);
        s.click(R, 20);
        // Klikk inne i en fold havner på starten av den
        assert_eq!(s.cursor.line, 15);
    }

    #[test]
    fn fold_kommandoer_holder_cursoren() {
        let mut s = three_changes(true);
        s.click(R, 45);
        let y = top(&s);
        let out = run(&mut s, FoldOpen);
        assert!(out.folding_changed);
        assert_eq!(s.cursor.line, 45);
        assert_eq!(top(&s), y);
        assert!(s.folding.fold_at_line(R, 49).is_some());
        run(&mut s, ToggleFolds);
        assert_eq!(s.folding.visible(R).count(), 0);
        times(&mut s, MoveDown, 20);
        run(&mut s, ToggleFolds);
        assert!(s.folding.visible(R).count() > 0);
        // Cursoren var inne i en fold som lukket seg
        assert_eq!(s.cursor.line, 45);
    }

    #[test]
    fn musehjul_skyver_cursoren() {
        let mut s = plain();
        times(&mut s, MoveDown, 11);
        assert!(!s.wheel(L, 30., VP));
        assert_eq!(s.cursor.line, 11);
        assert!(s.wheel(L, 100., VP));
        assert_eq!(s.cursor.line, 21);
        assert_eq!(s.master, L);
        // Brøkdel av en linje fra trackpad
        s.wheel(R, 3.5, VP);
        assert_scrolloff(&s);
    }

    #[test]
    fn sideveis_scroll() {
        let long = format!("{}\n", "x".repeat(200));
        let mut s = state(&long, &long, false);
        let hx = Horizontal {
            text_width: 100.,
            char_width: 5.,
            max_x: 905.,
        };
        s.execute(ScrollRight, VP, hx);
        assert_eq!(s.scroll.x, 20.);
        s.execute(ScrollToEnd, VP, hx);
        assert_eq!(s.scroll.x, 905.);
        assert!(!s.execute(ScrollRight, VP, hx).changed);
        s.execute(ScrollLeft, VP, hx);
        assert_eq!(s.scroll.x, 885.);
        s.execute(ScrollToStart, VP, hx);
        assert_eq!(s.scroll.x, 0.);
        assert!(!s.execute(ScrollLeft, VP, hx).changed);
        // Linja på bunnen er tom, så `$` går helt til venstre
        run(&mut s, GoToBottom);
        s.execute(ScrollRight, VP, hx);
        s.execute(ScrollToEnd, VP, hx);
        assert_eq!(s.scroll.x, 0.);
    }

    #[test]
    fn zoom_holder_cursorlinja_paa_plass() {
        let mut s = plain();
        times(&mut s, MoveDown, 50);
        let before = s.cursor_visual(VP) as f32 - top(&s) / LH;
        let big = Viewport {
            height: 400.,
            line_height: 20.,
        };
        s.rescale(LH, big, 2.);
        let after = s.cursor_visual(big) as f32 - top(&s) / 20.;
        assert_eq!(before, after);
    }

    #[test]
    fn resize_holder_cursoren_synlig() {
        let mut s = plain();
        times(&mut s, MoveDown, 18);
        let small = Viewport {
            height: 100.,
            line_height: LH,
        };
        s.resize(small);
        let (min, max) = s.cursor_range(top(&s), small);
        let cv = s.cursor_visual(small);
        assert!(min <= cv && cv <= max);
    }

    #[test]
    fn rull_tilbake_beholder_cursor_og_scroll() {
        // Tett nok til at gapet 37..44 ikke foldes etter at 40 er rullet tilbake
        let a = lines("a", 100);
        let mut b: Vec<String> = a.lines().map(str::to_owned).collect();
        for i in [36, 40, 44] {
            b[i] = format!("x{i}");
        }
        let mut s = state(&a, &(b.join("\n") + "\n"), true);
        run(&mut s, NextChange);
        run(&mut s, NextChange);
        assert_eq!(s.cursor, Cursor::new(R, 40));
        assert_eq!(s.cursor_block(), Some(1));
        let y = top(&s);
        let a = s.diff.text(L).to_owned();
        let text = s.diff.text(R).replace("x40\n", "a40\n");
        s.replace_diff(
            Rc::new(FileDiff::new(&a, &text, ComparisonPolicy::Default).unwrap()),
            VP,
        );
        assert_eq!(s.diff.blocks().len(), 2);
        assert_eq!(s.cursor, Cursor::new(R, 40));
        assert_eq!(top(&s), y);
        assert_eq!(s.cursor_block(), None);
    }
}
