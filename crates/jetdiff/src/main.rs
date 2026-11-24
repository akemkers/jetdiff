//! jetdiff [--tui] [<folder> | <commit> | <pr-link> | <left-side> <right-side>]
//!
//! Without arguments, Branch is shown: everything on the branch against where it branched off the
//! base branch, with Local changes and the branch's commits in the commit list. A folder does the
//! same there. With one commit, it is shown against its parent.
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

use jetdiff_app::{Commits, FilePair, RepoSource};
use jetdiff_git::{PullRequest, Repo};
use jetdiff_viewmodel::FileDiff;

enum Source {
    Branch(PathBuf),
    Commit(String),
    PullRequest(PullRequest),
    FilePair(String, String),
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
        [] => Ok(Source::Branch(".".into())),
        [url] if url.starts_with("https://") => PullRequest::parse(url)
            .map(Source::PullRequest)
            .ok_or_else(|| format!("not a PR link: {url}")),
        [dir] if Path::new(dir).is_dir() => Ok(Source::Branch(dir.into())),
        [rev] => Ok(Source::Commit(rev.clone())),
        [left_side, right_side] => Ok(Source::FilePair(left_side.clone(), right_side.clone())),
        _ => Err(
            "usage: jetdiff [--tui] [<folder> | <commit> | <pr-link> | <left-side> <right-side>]"
                .into(),
        ),
    }
}

/// What the window should show, ready before the app starts so errors can be written to the terminal.
pub enum Content {
    FilePair(FilePair, Box<FileDiff>),
    /// The commit list only for a folder, not for a commit or a PR.
    Repo(Box<RepoSource>, Option<Commits>),
}

fn read_source(source: &Source) -> Result<(Content, String), String> {
    let (repo_source, commits, title) = match source {
        Source::FilePair(left_side, right_side) => {
            let file_pair = FilePair::new(left_side.clone(), right_side.clone());
            let diff = file_pair.read()?;
            let title = format!("{left_side} ↔ {right_side}");
            return Ok((Content::FilePair(file_pair, Box::new(diff)), title));
        }
        Source::Commit(rev) => {
            let repo_source = RepoSource::commit(Path::new("."), rev)?;
            (repo_source, None, rev.clone())
        }
        Source::PullRequest(pr) => {
            let text = format!("Fetching PR #{} from {}", pr.number, pr.slug);
            let repo = with_progress(&text, || Repo::pull_request(Path::new("."), pr))?;
            let pull_request_title = format!("PR #{}", pr.number);
            (RepoSource::pull_request(repo)?, None, pull_request_title)
        }
        Source::Branch(dir) => {
            let (repo_source, commits) = RepoSource::open(dir)?;
            let title = commits.branch_label().to_owned();
            (repo_source, Some(commits), title)
        }
    };
    // Several windows open at the same time must be told apart in the title
    let title = format!("{}: {title}", repo_source.repo().name());
    Ok((Content::Repo(Box::new(repo_source), commits), title))
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

fn main() -> ExitCode {
    if std::env::args().any(|arg| arg == "--version") {
        println!("jetdiff {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let ((source, frontend), config) = match parse_args().and_then(|a| Ok((a, config::load()?))) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let (content, title) = match read_source(&source) {
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
