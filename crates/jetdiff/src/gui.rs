//! The GPUI window.

use gpui::{
    App, AppContext, Application, Bounds, Context, Focusable, KeyBinding, Render, WindowBounds,
    WindowOptions, actions, px, size,
};
use jetdiff_app::walkthrough::Writer;
use jetdiff_app::{AppCommand, BINDINGS};
use jetdiff_ui::{
    Assets, FileDiffView, FocusDown, FocusLeft, FocusRight, FocusUp, HideHelp, NextCommit,
    NextFile, PrevCommit, PrevFile, RepoView, ResetZoom, Settings, ToggleDiffMode, ToggleHelp,
    ToggleSidePanel, ToggleWalkthrough, UiTheme, ZoomIn, ZoomOut, load_font,
};

use crate::Content;
use crate::config::Config;

actions!(jetdiff, [Quit]);

pub fn run(content: Content, title: String, config: Config) {
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            bind_keys(cx);
            load_font(cx);
            cx.set_global(Settings {
                theme: UiTheme::new(&config.theme),
                font_size: config.font_size,
                fold_unchanged: config.fold_unchanged,
                unified: config.unified,
                side_panel_open: config.side_panel.open,
                walkthrough: Writer {
                    command: config.walkthrough.command.clone(),
                    model: config.walkthrough.model.clone(),
                    language: config.walkthrough.language.clone(),
                },
            });
            match content {
                Content::FilePair(diff, paths) => open(cx, title, move |cx| {
                    FileDiffView::standalone(*diff, paths, cx)
                }),
                Content::Repo(repo_source, commits) => open(cx, title, move |cx| {
                    RepoView::new(*repo_source, commits, cx)
                }),
            }
            cx.on_window_closed(|cx| cx.quit()).detach();
            cx.activate(true);
        });
}

fn bind_keys(cx: &mut App) {
    // The rest of the keys go through the vim parser in `FileDiffView`
    cx.bind_keys(BINDINGS.iter().map(|&(keys, cmd)| key_binding(keys, cmd)));
    cx.on_action(|_: &Quit, cx| cx.quit());
}

fn key_binding(keys: &str, cmd: AppCommand) -> KeyBinding {
    match cmd {
        AppCommand::ZoomIn => KeyBinding::new(keys, ZoomIn, None),
        AppCommand::ZoomOut => KeyBinding::new(keys, ZoomOut, None),
        AppCommand::ResetZoom => KeyBinding::new(keys, ResetZoom, None),
        AppCommand::FocusLeft => KeyBinding::new(keys, FocusLeft, None),
        AppCommand::FocusDown => KeyBinding::new(keys, FocusDown, None),
        AppCommand::FocusUp => KeyBinding::new(keys, FocusUp, None),
        AppCommand::FocusRight => KeyBinding::new(keys, FocusRight, None),
        AppCommand::ToggleSidePanel => KeyBinding::new(keys, ToggleSidePanel, None),
        AppCommand::ToggleDiffMode => KeyBinding::new(keys, ToggleDiffMode, None),
        AppCommand::NextFile => KeyBinding::new(keys, NextFile, None),
        AppCommand::PrevFile => KeyBinding::new(keys, PrevFile, None),
        AppCommand::NextCommit => KeyBinding::new(keys, NextCommit, None),
        AppCommand::ToggleWalkthrough => KeyBinding::new(keys, ToggleWalkthrough, None),
        AppCommand::PrevCommit => KeyBinding::new(keys, PrevCommit, None),
        AppCommand::ToggleHelp => KeyBinding::new(keys, ToggleHelp, None),
        AppCommand::HideHelp => KeyBinding::new(keys, HideHelp, None),
        AppCommand::Quit => KeyBinding::new(keys, Quit, None),
    }
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
