//! jetdiff [--tui] [<mappe> | <commit> | <pr-lenke> | <venstre> <høyre>]
//!
//! Uten argumenter vises lokale endringer, på en branch mot der den gikk ut fra main.
//! En mappe viser lokale endringer der. Med én commit vises den mot forelderen.
//! En PR-lenke hentes inn i repoet du står i. Innstillingene
//! ligger i config-fila, se `config.rs`. `--tui` viser diffen i terminalen.

mod config;
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "tui")]
mod tui;

#[cfg(not(any(feature = "gui", feature = "tui")))]
compile_error!("jetdiff trenger minst én av featurene gui og tui");

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use jetdiff_app::LOCAL_CHANGES;
use jetdiff_git::{PullRequest, Repo};
use jetdiff_viewmodel::{ComparisonPolicy, FileDiff, Language};

enum Source {
    WorkTree(PathBuf),
    Commit(String),
    PullRequest(PullRequest),
    Files(String, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frontend {
    #[cfg(feature = "gui")]
    Gui,
    #[cfg(feature = "tui")]
    Tui,
}

fn parse_args() -> Result<(Source, Frontend), String> {
    let mut tui = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--tui" => tui = true,
            flag if flag.starts_with("--") => return Err(format!("ukjent flagg: {flag}")),
            _ => paths.push(arg),
        }
    }
    Ok((parse_source(&paths)?, frontend(tui)?))
}

/// Vinduet når GPUI er bygd inn, ellers terminalen.
fn frontend(tui: bool) -> Result<Frontend, String> {
    match tui {
        #[cfg(feature = "tui")]
        true => Ok(Frontend::Tui),
        #[cfg(not(feature = "tui"))]
        true => Err("bygd uten featuren tui".into()),
        #[cfg(feature = "gui")]
        false => Ok(Frontend::Gui),
        #[cfg(not(feature = "gui"))]
        false => Ok(Frontend::Tui),
    }
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
        _ => Err(
            "bruk: jetdiff [--tui] [<mappe> | <commit> | <pr-lenke> | <venstre> <høyre>]".into(),
        ),
    }
}

fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Det vinduet skal vise, klart før appen starter så feil kan skrives til terminalen.
pub enum Content {
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

fn main() -> ExitCode {
    let ((source, frontend), config) = match parse_args().and_then(|a| Ok((a, config::load()?))) {
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

    match frontend {
        #[cfg(feature = "gui")]
        Frontend::Gui => gui::run(content, title, config),
        #[cfg(feature = "tui")]
        Frontend::Tui => {
            if let Err(e) = tui::run(content, &title, config) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}
