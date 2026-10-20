//! jetdiff [<mappe> | <commit> | <pr-lenke> | <venstre> <høyre>]
//!
//! Uten argumenter vises lokale endringer, på en branch mot der den gikk ut fra main.
//! En mappe viser lokale endringer der. Med én commit vises den mot forelderen.
//! En PR-lenke hentes inn i repoet du står i. Innstillingene
//! ligger i config-fila, se `config.rs`.

mod config;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use gpui::{
    App, AppContext, Application, Bounds, Context, Focusable, KeyBinding, Render, WindowBounds,
    WindowOptions, actions, px, size,
};
use jetdiff_git::{PullRequest, Repo};
use jetdiff_ui::{
    Assets, ChangesView, FileDiffView, FocusDown, FocusLeft, FocusRight, FocusUp, HideHelp,
    LOCAL_CHANGES, NextFile, PrevFile, ResetZoom, Settings, ToggleHelp, ToggleSidePanel,
    ToggleView, ZoomIn, ZoomOut, load_font,
};
use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Language};

actions!(jetdiff, [Quit]);

enum Source {
    WorkTree(PathBuf),
    Commit(String),
    PullRequest(PullRequest),
    Files(String, String),
}

fn parse_args() -> Result<Source, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(flag) = args.iter().find(|a| a.starts_with("--")) {
        return Err(format!("ukjent flagg: {flag}"));
    }
    parse_source(&args)
}

fn parse_source(paths: &[String]) -> Result<Source, String> {
    match paths {
        [] => Ok(Source::WorkTree(".".into())),
        [url] if url.starts_with("https://") => PullRequest::parse(url)
            .map(Source::PullRequest)
            .ok_or_else(|| format!("ikke en PR-lenke: {url}")),
        [dir] if Path::new(dir).is_dir() => Ok(Source::WorkTree(dir.into())),
        [rev] => Ok(Source::Commit(rev.clone())),
        [left, right] => Ok(Source::Files(left.clone(), right.clone())),
        _ => Err("bruk: jetdiff [<mappe> | <commit> | <pr-lenke> | <venstre> <høyre>]".into()),
    }
}

fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn bind_keys(cx: &mut App) {
    // Resten av tastene går gjennom vim-parseren i `FileDiffView`
    cx.bind_keys([
        // `secondary` er Cmd på macOS og Ctrl ellers. GPUI matcher på tegnet som skrives,
        // så Pluss er `+` på norsk og `=` på US
        KeyBinding::new("secondary-+", ZoomIn, None),
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ResetZoom, None),
        KeyBinding::new("ctrl-h", FocusLeft, None),
        KeyBinding::new("ctrl-j", FocusDown, None),
        KeyBinding::new("ctrl-k", FocusUp, None),
        KeyBinding::new("ctrl-l", FocusRight, None),
        KeyBinding::new("b", ToggleSidePanel, None),
        KeyBinding::new("v", ToggleView, None),
        KeyBinding::new("tab", NextFile, None),
        KeyBinding::new("J", NextFile, None),
        KeyBinding::new("shift-tab", PrevFile, None),
        KeyBinding::new("K", PrevFile, None),
        KeyBinding::new("?", ToggleHelp, None),
        KeyBinding::new("escape", HideHelp, None),
        KeyBinding::new("q", Quit, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-w", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Det vinduet skal vise, klart før appen starter så feil kan skrives til terminalen.
enum Content {
    File(Box<FileDiff>, [String; 2]),
    Changes(Repo, Vec<jetdiff_changes::FileChange>),
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
        Source::PullRequest(pr) => {
            let text = format!("Henter PR #{} fra {}", pr.number, pr.slug);
            let repo = with_progress(&text, || Repo::pull_request(Path::new("."), pr))?;
            (repo, format!("PR #{}", pr.number))
        }
        Source::WorkTree(dir) => (Repo::work_tree(dir)?, LOCAL_CHANGES.to_owned()),
    };
    // Flere vinduer åpne samtidig må kunne skilles i tittelen
    let title = format!("{}: {title}", repo.name());
    let changes = repo.changes()?;
    Ok((Content::Changes(repo, changes), title))
}

/// Viser at `work` jobber, med spinner og tid i en terminal.
fn with_progress<T>(text: &str, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let start = Instant::now();
    let seconds = || format!("{:.1} s", start.elapsed().as_secs_f32()).replace('.', ",");
    if !std::io::stderr().is_terminal() {
        eprintln!("{text} ...");
        let out = work()?;
        eprintln!("Ferdig på {}", seconds());
        return Ok(out);
    }
    let done = AtomicBool::new(false);
    let out = thread::scope(|s| {
        s.spawn(|| {
            for frame in "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().cycle() {
                if done.load(Ordering::Relaxed) {
                    break;
                }
                eprint!(
                    "\r\x1b[2K\x1b[36m{frame}\x1b[0m {text} \x1b[2m{}\x1b[0m",
                    seconds()
                );
                thread::sleep(Duration::from_millis(80));
            }
            eprint!("\r\x1b[2K");
        });
        let out = work();
        done.store(true, Ordering::Relaxed);
        out
    });
    let mark = if out.is_ok() {
        "\x1b[32m✓"
    } else {
        "\x1b[31m✗"
    };
    eprintln!("{mark}\x1b[0m {text} \x1b[2m{}\x1b[0m", seconds());
    out
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
    // Nær skjermstørrelsen, så zoom-animasjonen på macOS blir kort
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
    .expect("kunne ikke åpne vindu");
}

fn main() -> ExitCode {
    let (source, config) = match parse_args().and_then(|s| Ok((s, config::load()?))) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let (content, title) = match load(&source) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            bind_keys(cx);
            load_font(cx);
            cx.set_global(Settings {
                theme: config.theme,
                font_size: config.font_size,
                collapse_unchanged: config.collapse_unchanged,
                unified: config.unified,
                side_panel_open: config.side_panel.open,
            });
            match content {
                Content::File(diff, paths) => open(cx, title, move |cx| {
                    FileDiffView::standalone(*diff, paths, cx)
                }),
                Content::Changes(repo, changes) => {
                    open(cx, title, move |cx| ChangesView::new(repo, changes, cx))
                }
            }
            cx.on_window_closed(|cx| cx.quit()).detach();
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
