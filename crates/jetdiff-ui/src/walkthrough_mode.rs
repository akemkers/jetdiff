//! The walkthrough mode: the change read as steps, one page each, with the step's code under its
//! explanation and the contents on the left. `w` opens it from the diff, and nothing is written until
//! Enter is pressed on the intro. Writing goes on while the diff is shown.

mod contents;
mod intro;
mod overview;
mod page;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// How often the time on the progress is updated.
const TICK: Duration = Duration::from_secs(1);
/// Space above a hunk the page scrolls to, so its file header shows.
const HUNK_MARGIN: f32 = 48.;

pub enum WalkthroughEvent {
    /// Back to the diff.
    Close,
    /// Back to the diff, on this hunk. `None` is the first.
    Open { path: String, hunk: Option<usize> },
}

enum State {
    /// The files are read in the background.
    Loading,
    Intro,
    Writing {
        started: Instant,
        cancel: Arc<AtomicBool>,
    },
    Failed(String),
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
    /// An index in the walkthrough's not covered files.
    NotCovered(usize),
}

impl Item {
    /// `n` and `p` go between these, and Enter opens them in the diff.
    fn can_open(&self) -> bool {
        matches!(self, Self::Excerpt { .. } | Self::NotCovered(_))
    }
}

pub struct WalkthroughMode {
    /// What is read, in words for the model.
    what: String,
    digest: Option<Rc<Digest>>,
    /// Each digest file, ready to draw.
    diffs: Vec<Option<Rc<UnifiedDiff>>>,
    state: State,
    items: Vec<Item>,
    /// The item `n` and `p` are on.
    current: Option<usize>,
    scroll: ScrollHandle,
    animator: ScrollAnimator,
    style: Option<TextStyle>,
    scale: f32,
    focus: FocusHandle,
    _load: Task<()>,
    _write: Option<Task<()>>,
    _tick: Option<Task<()>>,
}

impl EventEmitter<WalkthroughEvent> for WalkthroughMode {}

impl WalkthroughMode {
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
            this.update(cx, |mode, cx| mode.loaded(digest, cx)).ok();
        });
        Self {
            what,
            digest: None,
            diffs: Vec::new(),
            state: State::Loading,
            items: Vec::new(),
            current: None,
            scroll: ScrollHandle::new(),
            animator: ScrollAnimator::default(),
            style: None,
            scale,
            focus: cx.focus_handle(),
            _load: load,
            _write: None,
            _tick: None,
        }
    }

    pub fn is_writing(&self) -> bool {
        matches!(self.state, State::Writing { .. })
    }

    pub fn set_scale(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.scale = scale;
        cx.notify();
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
        let key = cx.global::<Settings>().walkthrough.key(&digest);
        let cached = Cache::new()
            .and_then(|cache| cache.read(&key))
            .and_then(|answer| Walkthrough::from_answer(&answer, &digest).ok());
        self.digest = Some(digest);
        match cached {
            Some(walkthrough) => self.show(walkthrough, cx),
            None => self.state = State::Intro,
        }
        cx.notify();
    }

    /// Enter on the intro. The answer is cached on the other thread, so it is kept if the mode is
    /// closed before it comes.
    fn write(&mut self, cx: &mut Context<Self>) {
        let Some(digest) = self.digest.clone() else {
            return;
        };
        if digest.hunk_count() == 0 || digest.is_too_large() || self.is_writing() {
            return;
        }
        let job = Job::new(&digest, &self.what, &cx.global::<Settings>().walkthrough);
        let cancel = job.canceller();
        self.state = State::Writing {
            started: Instant::now(),
            cancel,
        };
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
            this.update(cx, |mode, cx| mode.written(answer, cx)).ok();
        }));
        self._tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn written(&mut self, answer: Result<String, String>, cx: &mut Context<Self>) {
        self._tick = None;
        let Some(digest) = self.digest.clone() else {
            return;
        };
        match answer.and_then(|answer| Walkthrough::from_answer(&answer, &digest)) {
            Ok(walkthrough) => self.show(walkthrough, cx),
            Err(error) => self.state = State::Failed(error),
        }
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let State::Writing { cancel, .. } = &self.state {
            cancel.store(true, Ordering::Relaxed);
            self.state = State::Intro;
            self._tick = None;
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
        self.current = None;
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
                    let Some(diff) = &self.diffs[file] else {
                        continue;
                    };
                    items.push(Item::File(file));
                    for rows in diff.excerpts(&hunks, CONTEXT) {
                        let in_rows = |hunk: &usize| rows.contains(&diff.hunk_rows()[*hunk].start);
                        items.push(Item::Excerpt {
                            file,
                            hunks: hunks.iter().copied().filter(in_rows).collect(),
                            excerpt: Rc::new(Excerpt::new(diff.clone(), rows, style)),
                        });
                    }
                }
            }
            None if reading.page() == Page::NotCovered => {
                items.extend((0..reading.not_covered().len()).map(Item::NotCovered));
            }
            None => {}
        }
        items
    }

    /// `n` and `p`.
    fn step_item(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(current) = self.current else {
            return;
        };
        let next = if forward {
            (current + 1..self.items.len()).find(|&i| self.items[i].can_open())
        } else {
            (0..current).rev().find(|&i| self.items[i].can_open())
        };
        if let Some(next) = next {
            self.current = Some(next);
            self.scroll_to_item(next, cx);
        }
    }

    fn open_current(&mut self, cx: &mut Context<Self>) {
        let (Some(current), Some(digest), State::Ready(reading)) =
            (self.current, &self.digest, &self.state)
        else {
            return;
        };
        let (file, hunk) = match &self.items[current] {
            Item::Excerpt { file, hunks, .. } => (*file, hunks.first().copied()),
            Item::NotCovered(i) => {
                let not_covered = &reading.not_covered()[*i];
                (not_covered.file, not_covered.hunks.first().copied())
            }
            Item::Heading | Item::File(_) => return,
        };
        let path = digest.files()[file].file.path.clone();
        cx.emit(WalkthroughEvent::Open { path, hunk });
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
            (State::Intro | State::Failed(_), "enter") => {
                self.write(cx);
                true
            }
            (State::Ready(_), "enter") => {
                self.open_current(cx);
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
    /// since the excerpts are shaped for the old one.
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
            self.current = self.items.iter().position(Item::can_open);
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

impl Drop for WalkthroughMode {
    /// A walkthrough no one will see is not paid for.
    fn drop(&mut self) {
        if let State::Writing { cancel, .. } = &self.state {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Focusable for WalkthroughMode {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WalkthroughMode {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_items(window);
        self.animate_scroll(window);
        let theme = cx.global::<Settings>().theme;
        let body = match &self.state {
            State::Ready(_) => self.ready(cx).into_any_element(),
            _ => self.intro(cx).into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.editor_bg)
            .text_color(theme.editor_fg)
            .font_family(FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(|this, _: &NextFile, _, cx| this.step_by(true, cx)))
            .on_action(cx.listener(|this, _: &PrevFile, _, cx| this.step_by(false, cx)))
            .on_action(cx.listener(|_, _: &HideHelp, _, cx| cx.emit(WalkthroughEvent::Close)))
            .child(body)
    }
}
