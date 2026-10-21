# 00 - Solution overview for jetdiff

A GPU-accelerated diff viewer in Rust + GPUI, modeled on JetBrains. The details are in `research/`:

- `01-diff-algorithm.md` - pipeline, heuristics, word diff
- `02-rendering.md` - diff divider, colors, gutter, folding waves
- `03-scroll-folding-structure.md` - sync scroll, folding, commit view, tree
- `04-unified-diff.md` - the Unified viewer: rows, line numbers, colors and switching from Split

## Constraints

- View only (read-only). No chevrons, no editing.
- macOS first, Linux later. No Windows.
- No 3-way merge.
- Will eventually become a module in a larger tool (project picker, worktrees). That's why the diff view is its own crate with no app assumptions.

## Crates

```
jetdiff-core     pure logic, no GUI. Tested with golden tests.
  lcs            Myers + Reindexer + patience fallback (port from IntelliJ)
  by_line        whitespace-blind match, two-step smart diff, ChunkOptimizer
  by_word        tokenizer, inner fragments, LineFragmentSplitter

jetdiff-syntax   tree-sitter with our own queries; tokens correspond to IntelliJ's DEFAULT_*

jetdiff-viewmodel
                 view model, still without a GUI
  model          FileDiff: hunks, lines, syntax per side
  state          cursor, commands, scroll and folding together
  unified        rows over FileDiff and their own state, no folding
  mode           Split or Unified, and switching between them
  scrolloff      where the cursor can be, shared by both diff modes
  scroll         master scroll_y, the slave computed in the same frame (1/3 anchor)
  folding        shared state for both sides
  geometry       gutter, text and scrollbar per side, and Unified with two number columns
  keys           key sequences to commands

jetdiff-changes  FileChange { path, old_path, status } and the tree model: folders first,
                 merged single-child folders, expanded state

jetdiff-git      git CLI: changes for a commit or Local changes, and file content

jetdiff-app      the state around the diff, still without a GUI. Shared by all UIs
  changes        repo, commit and the file being shown. Reload and the selected file afterwards
  content        from git content to FileDiff, with a message when it can't be shown
  files          the file tree's rows and cursor
  commits        the commit list's rows, cursor and selection
  actions        rollback and file:line for the cursor's line
  help           the keys in the keymap bar and the help
  keys           the keys outside the diff
  watcher        changed paths, collected until it is quiet

jetdiff-ui       GPUI
  code_viewer    picks Split or Unified for drawing and mouse
    split        two sides + divider, and mouse hits
    split_side   one side in Split: backgrounds, folds, text and line numbers
    divider      bezier diff divider and fold links
    unified      one code view with old and new number, and mouse hits
    code_paint   what both draw the same: text, inline highlights, hunk minimap
  file_diff      keys, mouse, zoom and scroll animation for the diff
  changes_tree   the file tree with icons and file status colors
  changes_view   the tree + one file, like the commit dialog in IntelliJ
  theme          color table from JetBrains (light and dark)

jetdiff-tui      the terminal, ratatui
  diff_view      keys, mouse and overlay for the diff
  code_viewer    Split and Unified as rows of cells, without folding
  panel          the file tree and the commit list
  app            the event loop, watcher and reload in a thread

jetdiff          binary: CLI, the gui and tui features
```

Modules don't know each other. Shared state lives in small GPUI entities that everyone observes: `Selection` (the selected file), and eventually the policy. The tree writes to `Selection` on click, and the diff view follows it. `J`/`K` and Tab move the selection in the tree wherever focus is. The shell makes the entities and gives them to the modules. The core never gets paths or git.

The split between `viewmodel`/`app` and `ui` is the most important one. All logic for scrolling, folding and geometry must be testable without a window. `ui` should only draw what `viewmodel` and `app` say.

## Key decisions

| Decision | Choice | Reason |
|---|---|---|
| Diff core | Port IntelliJ's Myers core (about 650 lines) | Gives the same result as JetBrains, and golden tests against IntelliJ. `imara-diff` can be an alternative behind a trait |
| Offsets | UTF-8 bytes | IntelliJ uses UTF-16; we convert in the golden tests |
| Side by side | Not aligned, with a diff divider | The diff divider is the point. A 1/3 anchor and whole-line steps in empty hunks prevent stutter |
| Unified | Rows that point into `FileDiff`, no folding | Syntax and inline highlights are reused. Added and deleted files are always shown Unified, so Split never has to show one side alone |
| Commit view | Tree + one file | Like the commit dialog in IntelliJ. Keeps the diff divider and sync scroll |
| Git | The `git` CLI | Same result as `git diff`, with the user's config and rename detection |
| Syntax | tree-sitter, both files in full | Deleted lines get the right syntax |
| Threads | Diff and highlighting in the background, wait at most about 200 ms before a spinner | No flashing on fast diffs |
| Shaders | GPUI `runtime_shaders` | The Metal Toolchain is missing locally. Can be switched once it's installed |

## Phases

1. **Diff core.** Port LCS and by_line with golden tests. Then by_word. A small tool that runs IntelliJ's algorithm on the same input and writes JSON gives the expected results.
2. **One file, JetBrains quality.** Row model, sync scroll, diff divider with the right curve (0.3/0.7), colors, inline highlights, gutter, hunk minimap. This is the milestone everything depends on.
3. **Folding and navigation.** Collapsed areas with a wave, next/previous hunk with animation.
4. **Syntax highlighting.**
5. **Commit view.** A tree with folders, icons and file status colors, and one file at a time. A commit against its parent and Local changes against HEAD.
6. **Git difftool and folder against folder.**

## From the spike (`spikes/gpui-trakt`, deleted, still in the git history)

Confirmed: GPUI gives sharp text, anti-aliased bezier paths, clipping per side and smooth scroll. Shared horizontal scroll works.

Must be done properly:
- The curve uses control points at 50 %; JetBrains uses 30 % and 70 %.
- Anchor in the middle; JetBrains uses 1/3 down.
- Our own colors, not JetBrains'.
- Lines are shaped again every frame. Needs a cache.
- `imara-diff` with `postprocess_lines`, not the IntelliJ heuristic.
- The mirrored gutter (both gutters toward the divider) is missing.
