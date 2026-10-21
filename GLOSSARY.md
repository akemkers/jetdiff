# jetdiff

A read-only diff viewer for files, commits and local changes in a git repo.

## Language

### Diff

**Side**:
One of the two versions being compared: Left is the old version, Right is the new one. Applies in unified mode too, where they are not shown next to each other.
_Avoid_: old/new, before/after, 1/2

**Hunk**:
A contiguous run of lines that differ between the two sides. Unlike a git hunk, it has no context lines.
_Avoid_: block, chunk, change, fragment

**Hunk kind**:
Added (lines only on Right), Deleted (lines only on Left) or Modified (lines on both sides).
_Avoid_: inserted

**File status**:
Added, Deleted, Modified or Renamed.

**Inline highlight**:
The changed words inside a Modified hunk, marked on each side.
_Avoid_: word highlight, word diff, fragment

**Fold**:
A run of unchanged lines hidden behind a single line that can be opened.
_Avoid_: collapsed region, gap

### Source

**Source**:
What the diff is taken from: Local changes, Commit, Branch, Pull request or Files.
_Avoid_: target, input

**Local changes**:
Everything not yet committed in the repo, staged or not, including untracked files, compared with HEAD.
_Avoid_: working tree, worktree, uncommitted changes

**Commit**:
A single commit compared with its parent.

**Branch**:
All commits on the current branch since it forked from main, shown together like a pull request.
_Avoid_: whole branch

**Pull request**:
A GitHub pull request compared with the merge base of its base branch.

**Files**:
Two files on disk compared directly, without a repo.

**Rollback**:
Putting one hunk in Local changes back to how it is in HEAD.
_Avoid_: revert, discard

**Source reload**:
Reading the source again, automatically, when files change on disk.
_Avoid_: reload, refresh, update

### View

**Diff mode**:
Split (the two sides next to each other) or Unified (one column). Files with status Added or Deleted are always shown Unified.
_Avoid_: side-by-side, view

**Cursor**:
The line or row you are on, in the diff or in a list. In the diff it is one line on one side, with no column.
_Avoid_: caret, highlighted line

**Selection**:
A choice that has been made in a list, such as the file or commit being shown. Moving the cursor does not change it.
_Avoid_: open, shown, current

**Cursor location**:
The file path and line number of the cursor's line in the newest version of the file, written as `path:line`.
_Avoid_: location, reference, position

**Side panel**:
The panel left of the diff, holding the file tree and the commit list. It can be hidden.
_Avoid_: left panel, sidebar, panel

**File tree**:
The files in the current source, grouped in folders, each with its file status.
_Avoid_: changes tree, file list

**Commit list**:
Local changes, Branch and the branch's own commits, to pick between. Only shown when the source is Local changes.

**Keymap bar**:
The most important keys, shown at the bottom. The rest are behind `?`.
_Avoid_: key bar, status bar

**Message**:
A short Info or Warning text above the keymap bar. It is either brief and disappears by itself, or pinned until removed.
_Avoid_: notice, toast

**Diff divider**:
The band between the sides in split mode, connecting each hunk on Left to the same hunk on Right.
_Avoid_: divider, funnel

**Hunk minimap**:
Coloured marks in the scrollbar showing where the hunks are in the file.
_Avoid_: error stripe, minimap, hunk markers
