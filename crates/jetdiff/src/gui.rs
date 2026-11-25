//! The GPUI window.

use gpui::{
    App, AppContext, Application, Bounds, Context, Focusable, Render, WindowBounds, WindowOptions,
    px, size,
};
use jetdiff_app::Config;
use jetdiff_ui::{Assets, FilePairView, Quit, RepoView, Settings, key_bindings, load_font};

use crate::Content;

pub fn run(content: Content, title: String, config: Config) {
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            bind_keys(cx);
            load_font(cx);
            cx.set_global(Settings::new(&config, env!("CARGO_PKG_VERSION")));
            match content {
                Content::FilePair(file_pair, diff) => {
                    open(cx, title, move |cx| FilePairView::new(file_pair, *diff, cx))
                }
                Content::Repo(repo_source, commits) => open(cx, title, move |cx| {
                    RepoView::new(*repo_source, commits, cx)
                }),
            }
            cx.on_window_closed(|cx| cx.quit()).detach();
            cx.activate(true);
        });
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys(key_bindings());
    cx.on_action(|_: &Quit, cx| cx.quit());
}

fn open<V: Render + Focusable>(
    cx: &mut App,
    title: String,
    build: impl FnOnce(&mut Context<V>) -> V + 'static,
) {
    // Close to the screen size, so the zoom animation on macOS is short
    let screen = cx.primary_display().map(|d| d.bounds().size);
    let start = screen.map_or(size(px(1400.), px(900.)), |s| {
        size(s.width * 0.95, s.height * 0.95)
    });
    let bounds = Bounds::centered(None, start, cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            focus: true,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(build);
            window.focus(&view.focus_handle(cx));
            view
        },
    )
    .expect("could not open window");
}
