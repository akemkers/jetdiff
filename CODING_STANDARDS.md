# Coding standards

How the code in jetdiff is organized. The goal is that a file can be read without knowing the rest, and that you find your way by reading the names.

## Small files with one responsibility

A file does one thing. If a file gathers several types or responsibilities, it is split into a folder with one file per responsibility. The Rust habit is larger files, but here we split early, because large files are hard to get an overview of and give too little abstraction.

A module with submodules is a file and a folder with the same name. Don't use `mod.rs`.

```
code_viewer.rs        puts it together: picks Split or Unified and draws
code_viewer/
  painter.rs          the building blocks: line, line number, fill, cursor row, diff divider
  split.rs            two sides with the diff divider between
  unified.rs          one line with two line numbers
  scrollbar.rs        thumb and hunk minimap marks
  hunk_color.rs       the hunk a row belongs to and its color
```

Group by domain. Don't make files like `utils.rs` or `types.rs`.

## The entry file puts it together

The file that has the module's name says what happens, and leaves the details to the submodules. It should read like a recipe:

```rust
pub fn paint(state: &UnifiedState, focused: bool, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let mut painter = Painter::new(state, theme, focused, buf);
    painter.background(area);
    let scrollbar = match state.diff.row_layout() {
        RowLayout::Split => split::paint(&mut painter, area),
        RowLayout::Unified => unified::paint(&mut painter, area),
    };
    scrollbar::paint(&mut painter, scrollbar, area);
}
```

If a function mixes what happens with how it is done, the steps get their own named functions.

## Small interface

Only what the rest of the crate needs is `pub`. `code_viewer` exposes `paint` and `row_at`, and everything else is private.

Types that the submodules share get `pub(super)`. Then the module and its submodules see them, but not the rest of the crate.

## State behind methods

Fields are private. Other files ask through methods instead of reaching into the structure:

```rust
// Not like this: split.rs knows what the state looks like
let state = painter.state;
let marked = index == state.row;

// Like this: painter.rs is the only one that knows
for visible in painter.visible_rows(area) {
    if visible.marked { ... }
}
```

Then the state can be changed in one place. The price is a few methods that mostly pass things on, and it's worth it.

## Names

- Use the terms in `GLOSSARY.md` in names, comments and UI text. A word under _Avoid_ is not used for the term it is listed under, so `panel` alone is not the side panel, while `SidePanel` is fine. `jetdiff-core` keeps IntelliJ's names, see ADR-0001, and so do IntelliJ names quoted in backticks in comments
- Use the whole glossary term in types, functions, fields and modules: `HunkMinimapMark`, not `MinimapMark`. A local variable can be short when it is part of the term, like `marks` for hunk minimap marks, but not a cut-down word or a word under _Avoid_
- `left` and `right` alone mean directions on screen. A side is taken as a `Side`, or written `left_side`/`right_side` when the name must hold it
- No abbreviations. Use descriptive names: `viewport`, not `vp`, and `anchor_left`, not `al`. This also applies to closure parameters: `|commits|`, not `|c|`. The exceptions are `e` for errors and `i` as a loop counter, which are common in Rust
- The name says what something is or does. `Painter` draws, while `Painted` sounds like something that has already been drawn

## No magic numbers

A number that means something gets a named constant. Say why in a comment when the name doesn't:

```rust
/// The automatic width never takes more than this of the window, as in GPUI.
const MAX_AUTO_FRACTION: f32 = 0.4;

let max = (window_width as f32 * MAX_AUTO_FRACTION) as u16;
```

The constant sits at the top of the file that uses it. If several files use it, it sits in the module above, like `WHEEL_ROWS` in `jetdiff-tui/src/lib.rs`. The exception is numbers that explain themselves where they are, like `0`, `+ 1` or dividing by `2` for the middle.

## Comments

Write code that explains itself. A comment explains why, not what, and is short. No section dividers and no commented-out code.

Doc comments at the top of a file say what the file is responsible for.

## Language

Everything in the repo is in English, with US spelling: code, comments, UI text, docs and test names. Commit messages too, in the imperative, e.g. "Add b to hide the side panel".
