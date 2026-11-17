# jetdiff

A diff viewer for local changes, commits and pull requests in a git repo, or for two files on disk.

## Language

### Diff

**Side**:
One of the two versions being compared: Left is the old version, Right is the new one. Applies in Unified too, where they are not shown next to each other. Left and Right with a capital letter are the sides. Not to be confused with left and right as directions on screen.
_Avoid_: old/new, before/after, 1/2

**Hunk**:
A contiguous run of lines that differ between the two sides. Unlike a git hunk, it has no context lines.
_Avoid_: block, chunk, change, fragment

**Hunk kind**:
Added (lines only on Right), Deleted (lines only on Left) or Modified (lines on both sides).
_Avoid_: inserted

**Changed file**:
A file that differs between the two sides in the current source, with its file status.
_Avoid_: file change, change

**File status**:
Added, Deleted, Modified or Renamed. The first three mean the same as for hunk kind, for the whole file.

**Rollback**:
Putting one hunk in Local changes back to how it is in HEAD.
_Avoid_: revert, discard

**Inline highlight**:
The changed words inside a Modified hunk, marked on each side.
_Avoid_: word highlight, word diff, fragment

**Fold**:
A run of unchanged lines hidden behind a single line that can be opened. A fold is either open or closed.
_Avoid_: collapsed region, gap, collapsed, expanded

### Source

**Source**:
What the diff is taken from: Branch, Local changes, Commit, Pull request or File pair.
_Avoid_: target, input

**Branch**:
Everything on the current branch that is not on its base branch, committed or not, including untracked files, compared with the merge base. Local changes and the branch's own commits are slices of it.
_Avoid_: whole branch, total diff, branch diff

**Local changes**:
Everything not yet committed in the repo, staged or not, including untracked files, compared with HEAD.
_Avoid_: working tree, worktree, uncommitted changes

**Commit**:
A single commit compared with its parent.

**Pull request**:
A GitHub pull request compared with the merge base of its base branch.

**Repo source**:
A source read from a git repo: Branch, Local changes, Commit or Pull request. Shown with the side panel.
_Avoid_: changes, multi file

**File pair**:
Two files on disk compared directly, without a repo.
_Avoid_: files, two files

**Source reload**:
Reading the source again, so the diff shows its current state.
_Avoid_: refresh, update

**Source autoreload**:
A source reload of Branch or Local changes that happens by itself when files change on disk.
_Avoid_: refresh, update

### Walkthrough

**Walkthrough**:
The hunks of a source grouped into steps in reading order and explained, written by a model that reads the diff. `w` shows it a page at a time, in place of the side panel and the diff.
_Avoid_: walkthrough mode, tour, guide, review

**Overview**:
The first page of a walkthrough: what the diff does, the notes, and the parts.
_Avoid_: front page, summary page

**Step**:
The hunks that serve one purpose, possibly across files, with a title and an explanation. A page of its own in the walkthrough.
_Avoid_: section, chapter

**Part**:
A group of steps that belong together, with a short title.
_Avoid_: group, chapter

**Key step**:
A step the rest of the walkthrough depends on, or where the main behaviour lives.
_Avoid_: key change, important step

**Skim step**:
A step that is safe to skim, like docs or wiring.
_Avoid_: minor step, context step

**Note**:
Something the reader takes on by accepting the diff, attached to a step: a Decision, Upkeep, a Limitation, a Divergence or something Untested. It states a fact and never judges the code.
_Avoid_: finding, warning, issue, remark

**Not covered**:
The changed files with hunks no step points to, and the files that weren't sent or read, each with a line on what changed there.
_Avoid_: skipped, uncovered, left out

**Contents**:
The list of the overview, the parts with their steps, and Not covered, shown in the side panel during a walkthrough.
_Avoid_: sidebar, table of contents

**Read**:
A step that has been shown.
_Avoid_: seen, visited

**Digest**:
The diff as the model reads it, with each hunk named by an alias and shown with a few unchanged lines around it.
_Avoid_: prompt, dump

**Alias**:
The name a hunk has in the digest, like `h3`, the only way the model points at a hunk.
_Avoid_: hunk id, reference

**Excerpt**:
A hunk, or hunks close together, shown with the unchanged lines around them.
_Avoid_: snippet, context lines, chunk

### User Interface

**Diff mode**:
Split (the two sides next to each other) or Unified (one column). The terminal shows Split as Aligned Split.
_Avoid_: side-by-side, view

**Aligned Split**:
Split where a line and its counterpart are in the same row, and the shorter side of a hunk gets empty rows. Used in the terminal. In the window each side scrolls on its own, held together by sync scroll.
_Avoid_: terminal split, row split

**Cursor**:
The line in the diff, or the row in a list, that you are on. In the diff it is one line on one side, with no column.
_Avoid_: caret, highlighted line

**Mirrored cursor**:
In Split, the cursor's line carried over to the other side and shown more faintly, so you see where the cursor lands if you switch side.
_Avoid_: transferred cursor, caret, other cursor

**Selection**:
A choice that has been made in a list, such as the file or commit being shown. Moving the cursor does not change it.
_Avoid_: open, shown, current

**Cursor location**:
The file path and line number of the cursor's line in the newest version of the file, written as `path:line`.
_Avoid_: location, reference, position

**Side panel**:
The panel on the left, the same in the diff and the walkthrough, with the walkthrough switch at the top. Next to the diff it holds the file tree, and for Branch and Local changes also the commit list. In a walkthrough it holds the contents.
_Avoid_: left panel, sidebar, panel

**Walkthrough switch**:
The Diff | Walkthrough buttons at the top of the side panel. A click on the side not shown does what `w` does.
_Avoid_: mode switch, segmented control

**File tree**:
The files in the current source, grouped in folders, each with its file status.
_Avoid_: changes tree, file list

**Commit list**:
Branch at the top, with Local changes and the branch's own commits under it as its slices, to pick between.

**Diff header**:
The row above the diff with the file's path and the counts of added and deleted lines at the left, and the file controls at the right.
_Avoid_: toolbar, header row, title bar

**File controls**:
The buttons at the right of the diff header that change how the file is shown: the diff mode (Split | Unified) and Fold. A button that can't do anything for the file is dimmed, not hidden.
_Avoid_: segmented control, toolbar

**File header**:
The bar over a file's excerpts in a walkthrough, with its path and file status.
_Avoid_: file bar

**Keymap bar**:
The most important keys, shown at the bottom across the whole window, in the diff and the walkthrough.
_Avoid_: key bar, status bar

**Notification**:
A short Info or Warning text above the keymap bar, either **Pinned** (about what the diff shows, such as an identical file or no changed files, stays until that changes), **Brief** (about something that just happened, disappears by itself) or **Progress** (about something going on in the background, such as a walkthrough being written, with a spinner and the time it has taken). A brief notification lies over a progress, and a progress over the pinned one.
_Avoid_: message, notice, toast

**Diff divider**:
The band between the sides in Split, connecting each hunk on Left to the same hunk on Right.
_Avoid_: funnel

**Fold link**:
The line across the diff divider joining a closed fold on Left to the same fold on Right.
_Avoid_: connector, separator

**Fold wave**:
The wavy line drawn across a closed fold on each side.
_Avoid_: separator

**Hunk minimap**:
Colored marks in the scrollbar showing where the hunks are in the file.
_Avoid_: error stripe, minimap, hunk markers
