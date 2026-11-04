//! Reading a walkthrough: the page being shown, which steps have been read, and the rows in the
//! contents. The overview comes first, then a page per step, and after the last one the files no
//! step covers.

use super::answer::{NotCovered, Note, Part, Step, Walkthrough, Weight};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    /// Indexes in `parts` and its `steps`.
    Step {
        part: usize,
        step: usize,
    },
    NotCovered,
}

pub enum ContentsRow<'a> {
    Overview {
        selected: bool,
    },
    Part(&'a Part),
    Step {
        page: usize,
        /// Counted from 1, across the parts.
        number: usize,
        step: &'a Step,
        read: bool,
        selected: bool,
    },
    NotCovered {
        page: usize,
        files: usize,
        selected: bool,
    },
}

pub struct Reading {
    walkthrough: Walkthrough,
    pages: Vec<Page>,
    page: usize,
    read: Vec<bool>,
}

impl Reading {
    /// Starts on the overview.
    pub fn new(walkthrough: Walkthrough) -> Self {
        let steps = walkthrough
            .parts
            .iter()
            .enumerate()
            .flat_map(|(part, written)| {
                (0..written.steps.len()).map(move |step| Page::Step { part, step })
            });
        let mut pages: Vec<Page> = std::iter::once(Page::Overview).chain(steps).collect();
        if !walkthrough.not_covered.is_empty() {
            pages.push(Page::NotCovered);
        }
        let mut read = vec![false; pages.len()];
        read[0] = true;
        Self {
            walkthrough,
            pages,
            page: 0,
            read,
        }
    }

    pub fn walkthrough(&self) -> &Walkthrough {
        &self.walkthrough
    }

    pub fn page(&self) -> Page {
        self.pages[self.page]
    }

    /// The step being shown, with its number and how many there are. The overview is page 0, so
    /// a step's page is its number.
    pub fn step(&self) -> Option<(&Step, usize, usize)> {
        let Page::Step { part, step } = self.page() else {
            return None;
        };
        let step = &self.walkthrough.parts[part].steps[step];
        Some((step, self.page, self.step_count()))
    }

    pub fn part(&self) -> Option<&Part> {
        let Page::Step { part, .. } = self.page() else {
            return None;
        };
        Some(&self.walkthrough.parts[part])
    }

    pub fn not_covered(&self) -> &[NotCovered] {
        &self.walkthrough.not_covered
    }

    pub fn step_count(&self) -> usize {
        self.pages.len() - 1 - usize::from(!self.walkthrough.not_covered.is_empty())
    }

    pub fn read_count(&self) -> usize {
        let steps = &self.read[1..=self.step_count()];
        steps.iter().filter(|&&read| read).count()
    }

    pub fn is_read(&self, page: usize) -> bool {
        self.read[page]
    }

    /// The first step not read yet, where reading goes on. `None` when all are read.
    pub fn first_unread_step(&self) -> Option<usize> {
        (1..=self.step_count()).find(|&page| !self.read[page])
    }

    /// The notes of every step, with the step's page, which is its number.
    pub fn notes(&self) -> Vec<(usize, &Note)> {
        let steps = self.walkthrough.parts.iter().flat_map(|part| &part.steps);
        let pages = steps.enumerate().map(|(i, step)| (i + 1, step));
        pages
            .flat_map(|(page, step)| step.notes.iter().map(move |note| (page, note)))
            .collect()
    }

    pub fn is_last(&self) -> bool {
        self.page + 1 == self.pages.len()
    }

    /// `false` at the ends.
    pub fn step_by(&mut self, forward: bool) -> bool {
        let page = if forward {
            self.page + 1
        } else {
            match self.page.checked_sub(1) {
                Some(page) => page,
                None => return false,
            }
        };
        self.go_to(page)
    }

    /// `false` when there is no such page.
    pub fn go_to(&mut self, page: usize) -> bool {
        if page >= self.pages.len() {
            return false;
        }
        self.page = page;
        self.read[page] = true;
        true
    }

    pub fn contents(&self) -> Vec<ContentsRow<'_>> {
        let mut rows = vec![ContentsRow::Overview {
            selected: self.page == 0,
        }];
        let mut page = 1;
        for part in &self.walkthrough.parts {
            rows.push(ContentsRow::Part(part));
            for step in &part.steps {
                rows.push(ContentsRow::Step {
                    page,
                    number: page,
                    step,
                    read: self.read[page],
                    selected: self.page == page,
                });
                page += 1;
            }
        }
        if !self.walkthrough.not_covered.is_empty() {
            rows.push(ContentsRow::NotCovered {
                page,
                files: self.walkthrough.not_covered.len(),
                selected: self.page == page,
            });
        }
        rows
    }
}

impl Step {
    pub fn is_key(&self) -> bool {
        self.weight == Weight::Key
    }

    /// The step's hunks per file, the files in the order they come up and the hunks in the order
    /// they are in the file.
    pub fn hunks_by_file(&self) -> Vec<(usize, Vec<usize>)> {
        let mut files: Vec<(usize, Vec<usize>)> = Vec::new();
        for hunk in &self.hunks {
            match files.iter_mut().find(|(file, _)| *file == hunk.file) {
                Some((_, hunks)) => hunks.push(hunk.hunk),
                None => files.push((hunk.file, vec![hunk.hunk])),
            }
        }
        for (_, hunks) in &mut files {
            hunks.sort_unstable();
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walkthrough::{HunkRef, Note, NoteKind};

    fn step(title: &str, hunks: &[(usize, usize)]) -> Step {
        Step {
            title: title.into(),
            points: Vec::new(),
            weight: Weight::Normal,
            notes: Vec::new(),
            hunks: hunks
                .iter()
                .map(|&(file, hunk)| HunkRef { file, hunk })
                .collect(),
        }
    }

    fn part(title: &str, steps: Vec<Step>) -> Part {
        Part {
            title: title.into(),
            steps,
        }
    }

    fn reading(not_covered: usize) -> Reading {
        Reading::new(Walkthrough {
            title: "T".into(),
            summary: String::new(),
            parts: vec![
                part("Core", vec![step("One", &[(0, 0)]), step("Two", &[(1, 0)])]),
                part("UI", vec![step("Three", &[(2, 0)])]),
            ],
            not_covered: (0..not_covered)
                .map(|file| NotCovered {
                    file,
                    hunks: vec![1],
                    note: String::new(),
                })
                .collect(),
        })
    }

    fn contents(reading: &Reading) -> Vec<String> {
        reading
            .contents()
            .iter()
            .map(|row| match row {
                ContentsRow::Overview { selected } => {
                    format!("Overview{}", if *selected { " <" } else { "" })
                }
                ContentsRow::Part(part) => part.title.clone(),
                ContentsRow::Step {
                    number,
                    step,
                    read,
                    selected,
                    ..
                } => format!(
                    "{number} {}{}{}",
                    step.title,
                    if *read { " read" } else { "" },
                    if *selected { " <" } else { "" }
                ),
                ContentsRow::NotCovered {
                    files, selected, ..
                } => {
                    format!("Not covered {files}{}", if *selected { " <" } else { "" })
                }
            })
            .collect()
    }

    #[test]
    fn the_overview_then_steps_then_not_covered() {
        let mut reading = reading(2);
        assert_eq!(
            contents(&reading),
            [
                "Overview <",
                "Core",
                "1 One",
                "2 Two",
                "UI",
                "3 Three",
                "Not covered 2"
            ]
        );
        assert_eq!(reading.page(), Page::Overview);
        assert!(reading.step().is_none());
        assert!(!reading.step_by(false));
        assert!(reading.step_by(true));
        assert_eq!(reading.step().unwrap().1, 1);
        assert!(reading.step_by(true));
        assert!(reading.step_by(true));
        assert_eq!(reading.part().unwrap().title, "UI");
        assert!(reading.step_by(true));
        assert_eq!(reading.page(), Page::NotCovered);
        assert!(reading.step().is_none() && reading.is_last());
        assert!(!reading.step_by(true));
        assert_eq!(reading.read_count(), 3);
    }

    #[test]
    fn a_row_goes_to_its_page() {
        let mut reading = reading(0);
        assert!(reading.go_to(3));
        let (step, number, of) = reading.step().unwrap();
        assert_eq!((step.title.as_str(), number, of), ("Three", 3, 3));
        assert_eq!(
            contents(&reading),
            ["Overview", "Core", "1 One", "2 Two", "UI", "3 Three read <"]
        );
        assert!(!reading.go_to(4));
        assert!(reading.is_last());
    }

    #[test]
    fn the_notes_of_all_steps_with_their_page() {
        let mut walkthrough = reading(0).walkthrough;
        let note = |text: &str| Note {
            kind: NoteKind::Decision,
            text: text.into(),
        };
        walkthrough.parts[0].steps[1].notes = vec![note("a"), note("b")];
        walkthrough.parts[1].steps[0].notes = vec![note("c")];
        let reading = Reading::new(walkthrough);
        let notes: Vec<_> = reading
            .notes()
            .into_iter()
            .map(|(page, note)| (page, note.text.as_str()))
            .collect();
        assert_eq!(notes, [(2, "a"), (2, "b"), (3, "c")]);
    }

    #[test]
    fn where_to_continue() {
        let mut reading = reading(1);
        assert_eq!(reading.first_unread_step(), Some(1));
        reading.go_to(1);
        reading.go_to(3);
        assert_eq!(reading.first_unread_step(), Some(2));
        assert!(reading.is_read(3) && !reading.is_read(2));
        reading.go_to(2);
        assert_eq!(reading.first_unread_step(), None);
    }

    #[test]
    fn hunks_by_file_in_the_order_files_come_up() {
        let step = step("S", &[(2, 5), (0, 1), (2, 1), (0, 0)]);
        assert_eq!(step.hunks_by_file(), [(2, vec![1, 5]), (0, vec![0, 1])]);
    }
}
