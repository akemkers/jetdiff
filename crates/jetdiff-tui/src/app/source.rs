//! The source being shown: a repo source with its side panel, watcher and reads in the background,
//! or a File pair.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use jetdiff_app::{
    Applied, Commits, FilePair, Files, Notification, ReadJob, RepoRead, RepoSource,
    RollbackRequest, Selected, Show, Watcher,
};
use jetdiff_files::ChangedFile;

use crate::Settings;
use crate::side_panel::SidePanel;

pub(super) enum Source {
    RepoSource(Box<Repository>),
    FilePair(FilePair),
}

impl Source {
    pub(super) fn repository(&self) -> Option<&Repository> {
        match self {
            Self::RepoSource(repository) => Some(repository),
            Self::FilePair(_) => None,
        }
    }

    pub(super) fn repository_mut(&mut self) -> Option<&mut Repository> {
        match self {
            Self::RepoSource(repository) => Some(repository),
            Self::FilePair(_) => None,
        }
    }
}

/// The answer from a read in the background.
struct ReadAnswer {
    generation: u64,
    read: Result<RepoRead, String>,
}

pub(super) struct Repository {
    repo_source: RepoSource,
    side_panel: SidePanel,
    selected: Option<usize>,
    side_panel_open: bool,
    watcher: Option<Watcher>,
    read_sender: Sender<ReadAnswer>,
    read_receiver: Receiver<ReadAnswer>,
    /// Only the answer to the last read is used.
    generation: u64,
}

impl Repository {
    /// With a warning to show if the watcher couldn't start. Only a folder is watched.
    pub(super) fn new(
        repo_source: RepoSource,
        commits: Option<Commits>,
        settings: &Settings,
    ) -> (Self, Option<Notification>) {
        let (watcher, warning) = match commits.is_some().then(|| Watcher::new(repo_source.repo())) {
            Some(Ok(started)) => (Some(started), None),
            Some(Err(e)) => (
                None,
                Some(Notification::warning(format!(
                    "Can't watch for file changes: {e}"
                ))),
            ),
            None => (None, None),
        };
        let files = Files::new(repo_source.changed_files().clone());
        let side_panel = SidePanel::new(files, commits, settings.icons);
        let selected = repo_source.first();
        let (read_sender, read_receiver) = channel();
        let repository = Self {
            repo_source,
            side_panel,
            selected,
            side_panel_open: settings.side_panel_open,
            watcher,
            read_sender,
            read_receiver,
            generation: 0,
        };
        (repository, warning)
    }

    pub(super) fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub(super) fn shown(&self) -> Option<&ChangedFile> {
        self.repo_source.shown()
    }

    pub(super) fn side_panel(&self) -> &SidePanel {
        &self.side_panel
    }

    pub(super) fn side_panel_mut(&mut self) -> &mut SidePanel {
        &mut self.side_panel
    }

    pub(super) fn side_panel_open(&self) -> bool {
        self.side_panel_open
    }

    /// Whether it is open now.
    pub(super) fn toggle_side_panel(&mut self) -> bool {
        self.side_panel_open = !self.side_panel_open;
        self.side_panel_open
    }

    /// Selects the file, also in the tree, and gives what the diff should show.
    pub(super) fn load(&mut self, file: Option<usize>) -> Show {
        self.selected = file;
        if let Some(file) = file {
            self.side_panel.reveal_file(file);
        }
        self.repo_source.load(file)
    }

    /// Starts a read when the watcher has seen a change, and applies the answer to the last read
    /// once it has arrived.
    pub(super) fn background(&mut self) -> Option<Applied> {
        if let Some(change) = self.watcher.as_ref().and_then(|watcher| watcher.take()) {
            let job = self.repo_source.changed_on_disk(change);
            self.read(job);
        }
        let answer = self.latest_read()?;
        Some(self.repo_source.apply(answer.read))
    }

    pub(super) fn source_reload(&mut self) {
        let job = self.repo_source.source_reload();
        self.read(job);
    }

    pub(super) fn select_commit(&mut self, selected: Selected) {
        let job = self.repo_source.select(selected);
        self.read(job);
    }

    /// Reads the repo source again afterwards, so the list keeps up.
    pub(super) fn rollback(&mut self, request: &RollbackRequest) -> Option<Result<Show, String>> {
        let rolled_back = self.repo_source.rollback(request);
        if let Some(Ok(_)) = rolled_back {
            let job = self.repo_source.rolled_back();
            self.read(job);
        }
        rolled_back
    }

    fn read(&mut self, job: ReadJob) {
        self.generation += 1;
        let generation = self.generation;
        let sender = self.read_sender.clone();
        thread::spawn(move || {
            let read = job.run();
            let _ = sender.send(ReadAnswer { generation, read });
        });
    }

    /// The answer to the last read, if it has arrived. Answers to earlier reads are dropped.
    fn latest_read(&mut self) -> Option<ReadAnswer> {
        let mut latest = None;
        while let Ok(answer) = self.read_receiver.try_recv() {
            if answer.generation == self.generation {
                latest = Some(answer);
            }
        }
        latest
    }
}
