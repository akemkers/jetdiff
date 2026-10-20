//! Endringene i et repo eller en commit, og fila som vises. Git leses i en `ReloadJob` som UI-et
//! kjører i bakgrunnen, og svaret gis tilbake til `reloaded`.

use std::path::PathBuf;
use std::rc::Rc;

use jetdiff_changes::{ChangeTree, FileChange};
use jetdiff_git::{Commit, Repo};
use jetdiff_view::FileDiff;

use crate::actions::RollbackRequest;
use crate::content::{Notice, build_diff};

/// Det en `ReloadJob` har hentet.
pub struct Loaded {
    commit: Option<String>,
    repo: Repo,
    changes: Vec<FileChange>,
    commits: Vec<Commit>,
}

/// Leser git, uten tilgang til tilstanden. Kan kjøres på en annen tråd.
pub struct ReloadJob {
    root: PathBuf,
    commit: Option<String>,
}

impl ReloadJob {
    /// Commit-lista hentes på nytt hver gang, siden working tree bestemmer hvilke commits den har.
    pub fn run(self) -> Result<Loaded, String> {
        let home = Repo::work_tree(&self.root)?;
        let commits = home.commits()?;
        let repo = match &self.commit {
            Some(hash) => Repo::commit(&self.root, hash)?,
            None => home,
        };
        let changes = repo.changes()?;
        Ok(Loaded {
            commit: self.commit,
            repo,
            changes,
            commits,
        })
    }
}

pub enum Reloaded {
    /// Commit-lista skal vise `shown`, det som faktisk vises, og feilen som advarsel.
    Failed {
        shown: Option<String>,
        error: String,
    },
    Done {
        commits: Vec<Commit>,
        /// Bare når lista er endret.
        changes: Option<Rc<[FileChange]>>,
        /// Fila som skal velges.
        file: Option<usize>,
        notice: Option<Notice>,
    },
}

/// Det diffen skal vise etter at en fil er valgt.
pub enum Show {
    /// Ingen fil. Tom diff, og beskjeden står fast.
    Empty(Notice),
    /// Samme fil med samme innhold. Ingenting å gjøre.
    Unchanged,
    Diff(Box<Shown>),
}

pub struct Shown {
    pub diff: FileDiff,
    /// Står fast til neste fil. `None` fjerner den forrige.
    pub notice: Option<Notice>,
    /// Venstre og høyre sti, for `y`.
    pub paths: [String; 2],
    /// Bare working tree kan skrives til, ikke en commit.
    pub can_rollback: bool,
    /// Samme fil med nytt innhold. Cursor og scroll beholdes.
    pub same_file: bool,
}

pub struct Changes {
    repo: Repo,
    /// Commiten som skal vises. `None` er working tree.
    commit: Option<String>,
    /// Commiten `changes` er fra. Blir lik `commit` når lastingen er ferdig.
    loaded_commit: Option<String>,
    changes: Rc<[FileChange]>,
    /// Det diffen viser nå. Samme innhold etter en reload lar posisjonen stå.
    shown: Option<(FileChange, Result<[String; 2], String>)>,
}

impl Changes {
    pub fn new(repo: Repo, changes: Vec<FileChange>) -> Self {
        Self {
            repo,
            commit: None,
            loaded_commit: None,
            changes: changes.into(),
            shown: None,
        }
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn changes(&self) -> &Rc<[FileChange]> {
        &self.changes
    }

    /// Fila som vises.
    pub fn shown(&self) -> Option<&FileChange> {
        self.shown.as_ref().map(|(c, _)| c)
    }

    /// Første fil i treet, den som velges ved oppstart.
    pub fn first(&self) -> Option<usize> {
        ChangeTree::new(&self.changes).first()
    }

    /// Vises etter neste `reload`.
    pub fn select_commit(&mut self, hash: Option<String>) {
        self.commit = hash;
    }

    pub fn reload(&self) -> ReloadJob {
        ReloadJob {
            root: self.repo.root().to_owned(),
            commit: self.commit.clone(),
        }
    }

    /// `paths` er filene watcheren så endret. Tom når reloaden kom av noe annet.
    pub fn reloaded(&mut self, loaded: Result<Loaded, String>, paths: &[String]) -> Reloaded {
        let Loaded {
            commit,
            repo,
            changes,
            commits,
        } = match loaded {
            Ok(l) => l,
            Err(error) => {
                self.commit = self.loaded_commit.clone();
                return Reloaded::Failed {
                    shown: self.commit.clone(),
                    error,
                };
            }
        };
        // Bare working tree mot working tree er en oppdatering. En commit endrer seg ikke.
        let same_work_tree = commit.is_none() && self.loaded_commit.is_none();
        let touched = if same_work_tree && !paths.is_empty() {
            jetdiff_changes::touched(&self.changes, &changes, paths)
        } else {
            0
        };
        let selected = self.shown().map(|c| c.path.clone());
        self.repo = repo;
        self.loaded_commit = commit;
        let changed = (*changes != *self.changes).then(|| {
            self.changes = changes.into();
            self.changes.clone()
        });
        // Valgt fil beholdes så lenge den er med
        let file = selected
            .and_then(|p| self.changes.iter().position(|c| c.path == p))
            .or_else(|| self.first());
        let notice = match touched {
            0 => None,
            1 => Some(Notice::info("Oppdatert: 1 fil endret")),
            n => Some(Notice::info(format!("Oppdatert: {n} filer endret"))),
        };
        Reloaded::Done {
            commits,
            changes: changed,
            file,
            notice,
        }
    }

    /// Leser fila på nytt fra git, så en reload med nytt innhold gir ny diff.
    pub fn load(&mut self, file: Option<usize>) -> Show {
        let Some(change) = file.map(|i| self.changes[i].clone()) else {
            self.shown = None;
            return Show::Empty(Notice::info("Ingen endringer"));
        };
        let content = self.repo.read(&change);
        self.show(change, content)
    }

    fn show(&mut self, change: FileChange, content: Result<[String; 2], String>) -> Show {
        let shown = Some((change.clone(), content.clone()));
        if shown == self.shown {
            return Show::Unchanged;
        }
        let same_file = self.shown().is_some_and(|c| c.path == change.path);
        self.shown = shown;
        let (diff, notice) = build_diff(&change, content);
        Show::Diff(Box::new(Shown {
            diff,
            notice,
            paths: [change.left_path().to_owned(), change.path.clone()],
            can_rollback: self.repo.is_work_tree(),
            same_file,
        }))
    }

    /// Skriver fila og viser resultatet med en gang, uten å vente på watcheren. Kjør en
    /// `reload` etterpå, så lista følger med. `None` når ingen fil vises.
    pub fn rollback(&mut self, request: &RollbackRequest) -> Option<Result<Show, String>> {
        let Some((change, Ok([left, _]))) = self.shown.clone() else {
            return None;
        };
        Some(
            self.repo
                .rollback(&change, request.lines.clone(), &request.before)
                .map(|after| self.show(change, Ok([left, after]))),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    struct TempRepo(PathBuf);

    impl TempRepo {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("jetdiff-app-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self(dir);
            repo.git(&["init", "-q", "-b", "main"]);
            repo.git(&["config", "user.email", "t@t"]);
            repo.git(&["config", "user.name", "t"]);
            repo
        }

        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .args(args)
                .current_dir(&self.0)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn write(&self, path: &str, text: &str) {
            std::fs::write(self.0.join(path), text).unwrap();
        }

        fn changes(&self) -> Changes {
            let repo = Repo::work_tree(Path::new(&self.0)).unwrap();
            let changes = repo.changes().unwrap();
            Changes::new(repo, changes)
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn reload(c: &mut Changes, paths: &[&str]) -> Reloaded {
        let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let loaded = c.reload().run();
        c.reloaded(loaded, &paths)
    }

    fn path(c: &Changes, file: Option<usize>) -> &str {
        &c.changes()[file.unwrap()].path
    }

    #[test]
    fn reload_beholder_valgt_fil() {
        let r = TempRepo::new("beholder");
        r.write("a.rs", "a\n");
        r.write("b.rs", "b\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "c"]);
        r.write("a.rs", "a2\n");
        r.write("b.rs", "b2\n");
        let mut c = r.changes();
        let b = c.changes().iter().position(|c| c.path == "b.rs");
        assert!(matches!(c.load(b), Show::Diff(s) if !s.same_file && s.can_rollback));
        // Samme innhold gir ingenting å gjøre
        assert!(matches!(c.load(b), Show::Unchanged));

        r.write("b.rs", "b3\n");
        r.write("c.rs", "c\n");
        let Reloaded::Done {
            changes,
            file,
            notice,
            ..
        } = reload(&mut c, &["b.rs", "c.rs"])
        else {
            panic!("reload feilet");
        };
        assert!(changes.is_some());
        assert_eq!(path(&c, file), "b.rs");
        assert_eq!(notice, Some(Notice::info("Oppdatert: 2 filer endret")));
        assert!(matches!(c.load(file), Show::Diff(s) if s.same_file));
    }

    #[test]
    fn ukjent_commit_viser_det_som_vises() {
        let r = TempRepo::new("ukjent");
        r.write("a.rs", "a\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "c"]);
        let mut c = r.changes();
        c.select_commit(Some("finnes-ikke".into()));
        assert!(matches!(
            reload(&mut c, &[]),
            Reloaded::Failed { shown: None, .. }
        ));
        // Neste reload er working tree igjen
        assert!(matches!(
            reload(&mut c, &[]),
            Reloaded::Done { file: None, .. }
        ));
        assert!(matches!(c.load(None), Show::Empty(_)));
    }
}
