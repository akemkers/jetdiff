//! Runs `jetdiff_app::Watcher` on the GPUI executor.

use std::path::Path;

use gpui::{Context, Task};
use jetdiff_app::watcher::{POLL, Watcher as Inner};

pub struct Watcher {
    _poll: Task<()>,
}

impl Watcher {
    pub fn new<T: 'static>(
        root: &Path,
        cx: &mut Context<T>,
        on_change: impl Fn(&mut T, Vec<String>, &mut Context<T>) + 'static,
    ) -> Result<Self, String> {
        let inner = Inner::new(root)?;
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let Some(paths) = inner.take() else {
                    continue;
                };
                if this.update(cx, |t, cx| on_change(t, paths, cx)).is_err() {
                    break;
                }
            }
        });
        Ok(Self { _poll: poll })
    }
}
