//! The walkthrough view: the diff read as steps, one page each, with the step's code under its
//! explanation. Its contents are drawn in the side panel. It reads the diff, and writes the
//! walkthrough when asked to, while the diff is shown. Whoever shows it asks first and tells how it
//! goes.

mod contents;
mod notes;
mod overview;
mod page;
mod prose;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Render, ScrollHandle, Task,
    Window, div, point, prelude::*, px,
};
use jetdiff_app::walkthrough::cache::Cache;
use jetdiff_app::walkthrough::digest::CONTEXT;
use jetdiff_app::walkthrough::{Digest, FileContent, Job, Page, Reading, Walkthrough};
use jetdiff_files::ChangedFile;
use jetdiff_git::Repo;
use jetdiff_viewmodel::metrics::FONT_SIZE;
use jetdiff_viewmodel::{ScrollState, UnifiedDiff};

use crate::code_viewer::Excerpt;
use crate::help::HideHelp;
use crate::repo_view::{NextFile, PrevFile};
use crate::scroll_animator::ScrollAnimator;
use crate::settings::Settings;
use crate::text::{FAMILY, TextStyle};

/// Space above a hunk the page scrolls to, so its file header shows.
const HUNK_MARGIN: f32 = 48.;
/// A file with more rows than this on the Not covered page is only a card, so the page stays short.
const MAX_NOT_COVERED_ROWS: usize = 200;

pub enum WalkthroughEvent {
    /// The diff is read, so `status` says what can be done.
    Loaded,
    Written,
    Failed(String),
    /// Back to the diff.
    Close,
    /// Back to the diff, on this hunk. `None` is the first.
    Open {
        path: String,
        hunk: Option<usize>,
    },
}

/// What the view can do now.
pub enum Status {
    Loading,
    /// Why there can be no walkthrough of this diff.
    CantWrite(&'static str),
    CanWrite,
    Writing,
    Ready,
}

enum State {
    /// The files are read in the background.
    Loading,
    Unwritten,
    Writing {
        cancel: Arc<AtomicBool>,
    },
    Ready(Reading),
}

/// What a page shows, from the top. The indexes are the children of the page's scroll.
enum Item {
    Heading,
    File(usize),
    Excerpt {
        file: usize,
        hunks: Vec<usize>,
        excerpt: Rc<Excerpt>,
    },
    /// An index in the walkthrough's not covered files. Its hunks follow, unless they are too long to
    /// show here.
    NotCovered {
        index: usize,
        too_long: bool,
    },
}

impl Item {
    fn row_count(&self) -> usize {
        match self {
            Self::Excerpt { excerpt, .. } => excerpt.row_count(),
            Self::Heading | Self::File(_) | Self::NotCovered { .. } => 0,
        }
    }

    /// `n` and `p` go between these, and Enter opens them in the diff.
    fn can_open(&self) -> bool {
        matches!(self, Self::Excerpt { .. } | Self::NotCovered { .. })
    }
}

pub struct WalkthroughView {
    /// What is read, in words for the model.
    what: String,
    digest: Option<Rc<Digest>>,
    /// Each digest file, ready to draw.
    diffs: Vec<Option<Rc<UnifiedDiff>>>,
    /// Split or Unified, as in the diff. Set when the walkthrough is shown.
    unified: bool,
    state: State,
    items: Vec<Item>,
    /// The item `n` and `p` are on.
    cursor: Option<usize>,
    scroll: ScrollHandle,
    animator: ScrollAnimator,
    style: Option<TextStyle>,
    scale: f32,
    focus: FocusHandle,
    _load: Task<()>,
    _write: Option<Task<()>>,
}

impl EventEmitter<WalkthroughEvent> for WalkthroughView {}

impl WalkthroughView {
    /// Reads `changed_files` in the background. A walkthrough written before for the same code is
    /// shown at once.
    pub fn new(
        repo: Repo,
        changed_files: Vec<ChangedFile>,
        what: String,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        let load = cx.spawn(async move |this, cx| {
            let digest = cx
                .background_executor()
                .spawn(async move { Digest::read(&repo, &changed_files) })
                .await;
            this.update(cx, |view, cx| view.loaded(digest, cx)).ok();
        });
        Self {
            what,
            digest: None,
            diffs: Vec::new(),
            unified: cx.global::<Settings>().unified,
            state: State::Loading,
            items: Vec::new(),
            cursor: None,
            scroll: ScrollHandle::new(),
            animator: ScrollAnimator::default(),
            style: None,
            scale,
            focus: cx.focus_handle(),
            _load: load,
            _write: None,
        }
    }

    pub fn status(&self) -> Status {
        match &self.state {
            State::Loading => Status::Loading,
            State::Writing { .. } => Status::Writing,
            State::Ready(_) => Status::Ready,
            State::Unwritten => match &self.digest {
                Some(digest) if digest.hunk_count() == 0 => {
                    Status::CantWrite("Nothing to walk through")
                }
                Some(digest) if digest.is_too_large() => Status::CantWrite(
                    "Too large for one walkthrough. Read a part of it, like a commit.",
                ),
                _ => Status::CanWrite,
            },
        }
    }

    pub fn is_writing(&self) -> bool {
        matches!(self.state, State::Writing { .. })
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
    }

    /// The page stays on the same hunk. Its items are made again, since an excerpt is drawn for one
    /// of them.
    pub fn set_unified(&mut self, unified: bool, cx: &mut Context<Self>) {
        if unified != self.unified {
            self.unified = unified;
            self.items = Vec::new();
            cx.notify();
        }
    }

    fn loaded(&mut self, digest: Digest, cx: &mut Context<Self>) {
        self.diffs = digest
            .files()
            .iter()
            .map(|file| match &file.content {
                FileContent::Diff(diff) => {
                    Some(Rc::new(UnifiedDiff::new(Rc::new(diff.as_ref().clone()))))
                }
                FileContent::Generated | FileContent::NotRead(_) => None,
            })
            .collect();
        let digest = Rc::new(digest);
        let key = cx.global::<Settings>().walkthrough.key(&digest, &self.what);
        let cached = Cache::new()
            .and_then(|cache| cache.read(&key))
            .and_then(|answer| Walkthrough::from_answer(&answer, &digest).ok());
        self.digest = Some(digest);
        match cached {
            Some(walkthrough) => self.show(walkthrough, cx),
            None => self.state = State::Unwritten,
        }
        cx.emit(WalkthroughEvent::Loaded);
        cx.notify();
    }

    /// The answer is cached on the other thread, so it is kept if the view is gone before it comes.
    pub fn write(&mut self, cx: &mut Context<Self>) {
        let Some(digest) = self.digest.clone() else {
            return;
        };
        if !matches!(self.status(), Status::CanWrite) {
            return;
        }
        let job = Job::new(&digest, &self.what, &cx.global::<Settings>().walkthrough);
        let cancel = job.canceller();
        self.state = State::Writing { cancel };
        self._write = Some(cx.spawn(async move |this, cx| {
            let answer = cx
                .background_executor()
                .spawn(async move {
                    let key = job.key.clone();
                    let answer = job.run()?;
                    if let Some(cache) = Cache::new() {
                        let _ = cache.write(&key, &answer);
                    }
                    Ok::<_, String>(answer)
                })
                .await;
            this.update(cx, |view, cx| view.written(answer, cx)).ok();
        }));
        cx.notify();
    }

    fn written(&mut self, answer: Result<String, String>, cx: &mut Context<Self>) {
        let Some(digest) = self.digest.clone() else {
            return;
        };
        match answer.and_then(|answer| Walkthrough::from_answer(&answer, &digest)) {
            Ok(walkthrough) => {
                self.show(walkthrough, cx);
                cx.emit(WalkthroughEvent::Written);
            }
            Err(error) => {
                self.state = State::Unwritten;
                cx.emit(WalkthroughEvent::Failed(error));
            }
        }
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let State::Writing { cancel } = &self.state {
            cancel.store(true, Ordering::Relaxed);
            self.state = State::Unwritten;
            self._write = None;
            cx.notify();
        }
    }

    fn show(&mut self, walkthrough: Walkthrough, cx: &mut Context<Self>) {
        self.state = State::Ready(Reading::new(walkthrough));
        self.page_changed(cx);
    }

    fn go_to(&mut self, page: usize, cx: &mut Context<Self>) {
        if let State::Ready(reading) = &mut self.state
            && reading.go_to(page)
        {
            self.page_changed(cx);
        }
    }

    fn step_by(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let State::Ready(reading) = &mut self.state
            && reading.step_by(forward)
        {
            self.page_changed(cx);
        }
    }

    /// The top of the page. Its items are made in `render`, which has the font.
    fn page_changed(&mut self, cx: &mut Context<Self>) {
        self.items = Vec::new();
        self.cursor = None;
        self.animator.jump(ScrollState::default());
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn page_items(&self, style: &TextStyle) -> Vec<Item> {
        let State::Ready(reading) = &self.state else {
            return Vec::new();
        };
        let mut items = vec![Item::Heading];
        match reading.step() {
            Some((step, _, _)) => {
                for (file, hunks) in step.hunks_by_file() {
                    if self.diffs[file].is_some() {
                        items.push(Item::File(file));
                        items.extend(self.excerpts(file, &hunks, style));
                    }
                }
            }
            None if reading.page() == Page::NotCovered => {
                for (index, not_covered) in reading.not_covered().iter().enumerate() {
                    let excerpts = self.excerpts(not_covered.file, &not_covered.hunks, style);
                    let rows: usize = excerpts.iter().map(Item::row_count).sum();
                    let too_long = rows > MAX_NOT_COVERED_ROWS;
                    items.push(Item::NotCovered { index, too_long });
                    if !too_long {
                        items.extend(excerpts);
                    }
                }
            }
            None => {}
        }
        items
    }

    /// The hunks of a file with the unchanged lines around them. None for a file without a diff.
    fn excerpts(&self, file: usize, hunks: &[usize], style: &TextStyle) -> Vec<Item> {
        let Some(diff) = &self.diffs[file] else {
            return Vec::new();
        };
        let excerpts = diff.excerpts(hunks, CONTEXT).into_iter().map(|rows| {
            let in_rows = |hunk: &usize| rows.contains(&diff.hunk_rows()[*hunk].start);
            Item::Excerpt {
                file,
                hunks: hunks.iter().copied().filter(in_rows).collect(),
                excerpt: Rc::new(Excerpt::new(diff.clone(), rows, self.unified, style)),
            }
        });
        excerpts.collect()
    }

    /// `n` and `p`.
    fn step_item(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(cursor) = self.cursor else {
            return;
        };
        let next = if forward {
            (cursor + 1..self.items.len()).find(|&i| self.items[i].can_open())
        } else {
            (0..cursor).rev().find(|&i| self.items[i].can_open())
        };
        if let Some(next) = next {
            self.cursor = Some(next);
            self.scroll_to_item(next, cx);
        }
    }

    fn open_at_cursor(&mut self, cx: &mut Context<Self>) {
        let (Some(cursor), Some(digest), State::Ready(reading)) =
            (self.cursor, &self.digest, &self.state)
        else {
            return;
        };
        let (file, hunk) = match &self.items[cursor] {
            Item::Excerpt { file, hunks, .. } => (*file, hunks.first().copied()),
            Item::NotCovered { index, .. } => {
                let not_covered = &reading.not_covered()[*index];
                (not_covered.file, not_covered.hunks.first().copied())
            }
            Item::Heading | Item::File(_) => return,
        };
        let path = digest.files()[file].file.path.clone();
        cx.emit(WalkthroughEvent::Open { path, hunk });
    }

    /// A file's header opens the file on its first hunk on the page.
    fn open_file(&mut self, item: usize, cx: &mut Context<Self>) {
        let Item::File(file) = self.items[item] else {
            return;
        };
        if let Some(Item::Excerpt { file: next, .. }) = self.items.get(item + 1)
            && *next == file
        {
            self.cursor = Some(item + 1);
            return self.open_at_cursor(cx);
        }
        if let Some(digest) = &self.digest {
            let path = digest.files()[file].file.path.clone();
            cx.emit(WalkthroughEvent::Open { path, hunk: None });
        }
    }

    /// Scrolled with smooth scroll from where the page is, also after the wheel moved it.
    fn scroll_to(&mut self, y: f32, cx: &mut Context<Self>) {
        let shown = -f32::from(self.scroll.offset().y);
        let max = f32::from(self.scroll.max_offset().height);
        let at = |y: f32| ScrollState { y: [y, y], x: 0. };
        self.animator.jump(at(shown));
        self.animator.animate(at(y.clamp(0., max)));
        cx.notify();
    }

    fn scroll_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        let from = if self.animator.is_animating() {
            self.animator.shown().y[0]
        } else {
            -f32::from(self.scroll.offset().y)
        };
        self.scroll_to(from + delta, cx);
    }

    fn scroll_to_item(&mut self, item: usize, cx: &mut Context<Self>) {
        // Laid out without the scroll, so it is the item's place on the page
        let Some(bounds) = self.scroll.bounds_for_item(item) else {
            return;
        };
        let top = f32::from(bounds.top() - self.scroll.bounds().top());
        let y = top - HUNK_MARGIN * self.scale;
        self.scroll_to(y, cx);
    }

    fn line_height(&self) -> f32 {
        self.style
            .as_ref()
            .map_or(FONT_SIZE, TextStyle::line_height)
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &ev.keystroke;
        if keystroke.modifiers.modified() {
            return;
        }
        let half_page = f32::from(self.scroll.bounds().size.height) / 2.;
        let handled = match (&self.state, keystroke.key.as_str()) {
            (State::Ready(_), "enter") => {
                self.open_at_cursor(cx);
                true
            }
            (State::Ready(_), "right") => {
                self.step_by(true, cx);
                true
            }
            (State::Ready(_), "left") => {
                self.step_by(false, cx);
                true
            }
            (State::Ready(_), "n") => {
                self.step_item(true, cx);
                true
            }
            (State::Ready(_), "p") => {
                self.step_item(false, cx);
                true
            }
            (_, "j" | "down") => {
                self.scroll_by(self.line_height(), cx);
                true
            }
            (_, "k" | "up") => {
                self.scroll_by(-self.line_height(), cx);
                true
            }
            (_, "d") => {
                self.scroll_by(half_page, cx);
                true
            }
            (_, "u") => {
                self.scroll_by(-half_page, cx);
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    /// The code font follows the zoom in the diff. The page's items are made again for a new size,
    /// since the excerpts are shaped for the old one, and the page stays on the same hunk.
    fn ensure_items(&mut self, window: &Window) {
        let size = FONT_SIZE * self.scale;
        if self
            .style
            .as_ref()
            .is_none_or(|style| style.font_size != px(size))
        {
            self.style = Some(TextStyle::new(size, window));
            self.items = Vec::new();
        }
        if self.items.is_empty()
            && let Some(style) = &self.style
        {
            self.items = self.page_items(style);
            if self.cursor.is_none() {
                self.cursor = self.items.iter().position(Item::can_open);
            }
        }
    }

    /// Moves the page during smooth scroll. Between animations the wheel owns the scroll.
    fn animate_scroll(&mut self, window: &mut Window) {
        if !self.animator.is_animating() {
            return;
        }
        if self.animator.step() {
            window.request_animation_frame();
        }
        let y = self.animator.shown().y[0];
        self.scroll.set_offset(point(px(0.), px(-y)));
    }
}

/// Like "1 file" and "3 files".
fn plural(count: usize, what: &str) -> String {
    if count == 1 {
        format!("1 {what}")
    } else {
        format!("{count} {what}s")
    }
}

impl Drop for WalkthroughView {
    /// A walkthrough no one will see is not paid for.
    fn drop(&mut self) {
        if let State::Writing { cancel } = &self.state {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Focusable for WalkthroughView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WalkthroughView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_items(window);
        self.animate_scroll(window);
        let body = self.ready(cx);
        let palette = cx.global::<Settings>().theme.palette;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.page)
            .text_color(palette.primary)
            .font_family(FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.step_by(true, cx)))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| this.step_by(false, cx)))
            .on_action(cx.listener(|_, _: &HideHelp, _, cx| cx.emit(WalkthroughEvent::Close)))
            .child(body)
    }
}
