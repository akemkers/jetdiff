//! smudiff [<mappe> | <commit> | <venstre> <høyre>] [--theme catppuccin-mocha|jetbrains-dark|jetbrains-light]
//!
//! Uten argumenter vises lokale endringer, på en branch mot der den gikk ut fra main.
//! En mappe viser lokale endringer der. Med én commit vises den mot forelderen.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gpui::{
    App, AppContext, Application, Bounds, Context, Focusable, KeyBinding, Render, WindowBounds,
    WindowOptions, actions, px, size,
};
use smudiff_git::Repo;
use smudiff_ui::{
    Assets, ChangesView, FileDiffView, FocusDown, FocusLeft, FocusRight, FocusUp, Message,
    NextContextRange, ResetZoom, UiTheme, ZoomIn, ZoomOut, load_font,
};
use smudiff_view::{ComparisonPolicy, FileDiff, Language};

actions!(smudiff, [Quit]);

enum Source {
    WorkTree(PathBuf),
    Commit(String),
    Files(String, String),
}

struct Args {
    source: Source,
    theme: UiTheme,
}

fn parse_args() -> Result<Args, String> {
    let mut theme = UiTheme::mocha();
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => {
                theme = match args.next().as_deref() {
                    Some("catppuccin-mocha") => UiTheme::mocha(),
                    Some("jetbrains-dark") => UiTheme::dark(),
                    Some("jetbrains-light") => UiTheme::light(),
                    _ => {
                        return Err(
                            "--theme må være catppuccin-mocha, jetbrains-dark eller jetbrains-light".into(),
                        );
                    }
                }
            }
            s if s.starts_with("--") => return Err(format!("ukjent flagg: {s}")),
            _ => paths.push(arg),
        }
    }
    let source = match <[String; 2]>::try_from(paths) {
        Ok([left, right]) => Source::Files(left, right),
        Err(paths) => match <[String; 1]>::try_from(paths) {
            Ok([dir]) if Path::new(&dir).is_dir() => Source::WorkTree(dir.into()),
            Ok([rev]) => Source::Commit(rev),
            Err(paths) if paths.is_empty() => Source::WorkTree(".".into()),
            Err(_) => {
                return Err(
                    "bruk: smudiff [<mappe> | <commit> | <venstre> <høyre>] [--theme catppuccin-mocha|jetbrains-dark|jetbrains-light]".into(),
                );
            }
        },
    };
    Ok(Args { source, theme })
}

fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn bind_keys(cx: &mut App) {
    // Resten av tastene går gjennom vim-parseren i `FileDiffView`
    cx.bind_keys([
        // GPUI matcher på tegnet som skrives: Cmd+Pluss er `+` på norsk og `=` på US
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
        // IntelliJ har ingen snarvei, bare en meny i verktøylinja
        KeyBinding::new("cmd-alt-c", NextContextRange, None),
        KeyBinding::new("ctrl-h", FocusLeft, None),
        KeyBinding::new("ctrl-j", FocusDown, None),
        KeyBinding::new("ctrl-k", FocusUp, None),
        KeyBinding::new("ctrl-l", FocusRight, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Det vinduet skal vise, klart før appen starter så feil kan skrives til terminalen.
enum Content {
    File(Box<FileDiff>, [String; 2]),
    Changes(Repo, Vec<smudiff_changes::FileChange>),
}

fn load(source: &Source) -> Result<(Content, String), String> {
    let (repo, title) = match source {
        Source::Files(left, right) => {
            let diff = file_diff(left, right)?;
            let title = format!("{left} ↔ {right}");
            let paths = [left.clone(), right.clone()];
            return Ok((Content::File(Box::new(diff), paths), title));
        }
        Source::Commit(rev) => (Repo::commit(Path::new("."), rev)?, rev.clone()),
        Source::WorkTree(dir) => {
            let repo = Repo::work_tree(dir)?;
            let title = repo.label();
            (repo, title)
        }
    };
    // Flere vinduer åpne samtidig må kunne skilles i tittelen
    let title = format!("{}: {title}", repo.name());
    let changes = repo.changes()?;
    Ok((Content::Changes(repo, changes), title))
}

fn file_diff(left: &str, right: &str) -> Result<FileDiff, String> {
    let (l, r) = (read(left)?, read(right)?);
    let mut diff = FileDiff::new(&l, &r, ComparisonPolicy::Default)
        .map_err(|_| "filene er for store til å diffes".to_owned())?;
    // Høyre side først, venstre kan være en midlertidig fil fra git difftool
    if let Some(lang) = Language::from_path(right).or_else(|| Language::from_path(left)) {
        diff.highlight_syntax(lang);
    }
    Ok(diff)
}

fn open<V: Render + Focusable>(
    cx: &mut App,
    title: String,
    build: impl FnOnce(&mut Context<V>) -> V + 'static,
) {
    let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
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
    .expect("kunne ikke åpne vindu");
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let (content, title) = match load(&args.source) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let theme = args.theme;

    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            bind_keys(cx);
            let family = load_font(cx);
            match content {
                Content::File(diff, paths) => open(cx, title, move |cx| {
                    let message = cx.new(|_| Message::new(theme, family));
                    FileDiffView::new(*diff, paths, message, theme, family, cx)
                }),
                Content::Changes(repo, changes) => open(cx, title, move |cx| {
                    ChangesView::new(repo, changes, theme, family, cx)
                }),
            }
            cx.on_window_closed(|cx| cx.quit()).detach();
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
