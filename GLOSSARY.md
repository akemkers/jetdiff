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
What the diff is taken from: Local changes, Commit, Pull request or File pair.
_Avoid_: target, input

**Local changes**:
Everything not yet committed in the repo, staged or not, including untracked files, compared with HEAD.
_Avoid_: working tree, worktree, uncommitted changes

**Commit**:
A single commit compared with its parent.

**Pull request**:
A GitHub pull request compared with the merge base of its base branch.

**Repo source**:
A source read from a git repo: Local changes, Commit or Pull request. Shown with the side panel.
_Avoid_: changes, multi file

**File pair**:
Two files on disk compared directly, without a repo.
_Avoid_: files, two files

**Source autoreload**:
Local changes read again automatically when files change on disk.
_Avoid_: reload, refresh, update

### User Interface

**Diff mode**:
Split (the two sides next to each other) or Unified (one column).
_Avoid_: side-by-side, view

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
The panel left of the diff, holding the file tree, and for Local changes also the commit list.
_Avoid_: left panel, sidebar, panel

**File tree**:
The files in the current source, grouped in folders, each with its file status.
_Avoid_: changes tree, file list

**Commit list**:
Local changes followed by commits, to pick between.

**Keymap bar**:
The most important keys, shown at the bottom.
_Avoid_: key bar, status bar

**Notification**:
A short Info or Warning text above the keymap bar, either **Pinned** (about what the diff shows, such as an identical file or no changed files, stays until that changes) or **Brief** (about something that just happened, disappears by itself). A brief notification lies over the pinned one while it lasts.
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
