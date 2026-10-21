//! jetdiff [--tui] [<folder> | <commit> | <pr-link> | <left> <right>]
//!
//! Without arguments, Local changes are shown, on a branch against where it branched off main.
//! A folder shows Local changes there. With one commit, it is shown against its parent.
//! A PR link is fetched into the repo you are in. The settings
//! are in the config file; see `config.rs`. `--tui` shows the diff in the terminal.

mod config;
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "tui")]
mod tui;

#[cfg(not(any(feature = "gui", feature = "tui")))]
compile_error!("jetdiff needs at least one of the features gui and tui");

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
            flag if flag.starts_with("--") => return Err(format!("unknown flag: {flag}")),
            _ => paths.push(arg),
        }
    }
    Ok((parse_source(&paths)?, frontend(tui)?))
}

/// The window when GPUI is built in, otherwise the terminal.
fn frontend(tui: bool) -> Result<Frontend, String> {
    match tui {
        #[cfg(feature = "tui")]
        true => Ok(Frontend::Tui),
        #[cfg(not(feature = "tui"))]
        true => Err("built without the tui feature".into()),
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
            .ok_or_else(|| format!("not a PR link: {url}")),
        [dir] if Path::new(dir).is_dir() => Ok(Source::WorkTree(dir.into())),
        [rev] => Ok(Source::Commit(rev.clone())),
        [left, right] => Ok(Source::Files(left.clone(), right.clone())),
        _ => {
            Err("usage: jetdiff [--tui] [<folder> | <commit> | <pr-link> | <left> <right>]".into())
        }
    }
}

fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binary files and wrong encoding are shown with replacement characters instead of failing
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// What the window should show, ready before the app starts so errors can be written to the terminal.
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
            let text = format!("Fetching PR #{} from {}", pr.number, pr.slug);
            let repo = with_progress(&text, || Repo::pull_request(Path::new("."), pr))?;
            (repo, format!("PR #{}", pr.number))
        }
        Source::WorkTree(dir) => (Repo::work_tree(dir)?, LOCAL_CHANGES.to_owned()),
    };
    // Several windows open at the same time must be told apart in the title
    let title = format!("{}: {title}", repo.name());
    let changes = repo.changes()?;
    Ok((Content::Changes(repo, changes), title))
}

/// Shows that `work` is working, with a spinner and time in a terminal.
fn with_progress<T>(text: &str, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let start = Instant::now();
    let seconds = || format!("{:.1} s", start.elapsed().as_secs_f32());
    if !std::io::stderr().is_terminal() {
        eprintln!("{text} ...");
        let out = work()?;
        eprintln!("Done in {}", seconds());
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
        .map_err(|_| "the files are too big to diff".to_owned())?;
    // Right first; Left can be a temporary file from git difftool
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
