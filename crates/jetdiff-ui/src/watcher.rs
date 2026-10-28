//! Runs `jetdiff_app::Watcher` on the GPUI executor.

use gpui::{Context, Task};
use jetdiff_app::DiskChange;
use jetdiff_app::watcher::{POLL, Watcher as Inner};
use jetdiff_git::Repo;

pub struct Watcher {
    _poll: Task<()>,
}

impl Watcher {
    pub fn new<T: 'static>(
        repo: &Repo,
        cx: &mut Context<T>,
        on_change: impl Fn(&mut T, DiskChange, &mut Context<T>) + 'static,
    ) -> Result<Self, String> {
        let inner = Inner::new(repo)?;
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let Some(change) = inner.take() else {
                    continue;
                };
                if this.update(cx, |t, cx| on_change(t, change, cx)).is_err() {
                    break;
                }
            }
        });
        Ok(Self { _poll: poll })
    }
}
