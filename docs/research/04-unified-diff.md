# 04 - Unified diff in IntelliJ

Source: `~/git/_ref/intellij-community`. Paths are relative to
`platform/diff-impl/src/com/intellij/diff/` unless stated otherwise. The Unified code
is in `tools/fragmented/`.

Terms:
- **The unified document** = the single text the unified editor shows. Rebuilt on every rediff.
- **Unified line** = a line in the unified document. **Side line** = a line in the left or right file.
- **Master** = the side equal lines are taken from, and which can be edited. Default `RIGHT`
  (`UnifiedDiffViewer.java:155`). Becomes `LEFT` only if only the left side is writable (`:176-179`),
  or via `DiffUserDataKeys.MASTER_SIDE` (`processContextHints`, `:253-259`).
- **Change** = `UnifiedDiffChange`: `line1..line2` in unified lines, split into `getDeletedRange()`
  and `getInsertedRange()` (`UnifiedDiffChange.java:29-42`).

---

## 1. How the document is built

Files: `UnifiedFragmentBuilder.kt`, `UnifiedDiffViewer.computeDifferences` (`:417-432`).

The diff is the same `List<LineFragment>` that side-by-side uses. The builder walks through
the fragments in order:

```
exec(fragments):                                  // SimpleUnifiedFragmentBuilder :160-171
  for f in fragments:
    processChanged(f)
  processEquals(lineCount1 - 1, lineCount2 - 1)   // finishDocuments :53-55

processChanged(f):                                // :67-100
  processEquals(f.start1 - 1, f.start2 - 1)       // equal lines before the change
  blockStart = totalLines
  appendText(LEFT,  f.start1..f.end1)             // deleted first
  insertedStart = totalLines
  appendText(RIGHT, f.start2..f.end2)             // then added
  return (blockStart, insertedStart, totalLines)

processEquals -> appendTextMaster(...)            // :109-121
  text only from the master, but both converters get a row
```

- Deleted lines always come above added ones in the same change. No line-by-line interleaving.
- Equal lines are taken from the master (right). The left side's version of equal lines is never shown.
- Each piece is added as `text.subSequence(start, lineEnd)` + `'\n'` (`appendText`, `:144-145`).
  That means the document always ends with an extra, fake line after the last real line.
  The viewer code knows this: `"there are 'fake' newline at the very end"` (`UnifiedDiffViewer.java:531`).
- A last line without `'\n'` is not special-cased. `LineOffsets` gives an empty last line when the text
  ends with `'\n'`, just as in side-by-side. If only one side ends with `'\n'`, the diff itself
  produces an inserted or deleted empty line.
- `HighlightRange(side, base, changed)` (`HighlightRange.java`, made in `appendText` `:139-142`)
  remembers where each piece came from: offsets in the unified document and in the original text. Used for syntax
  and markup (see 3). The `newline` adjustment at `:139` includes the line break when the original has one.

### Ignore whitespace and inner fragments

- Unified uses `TwosideTextDiffProvider.NoIgnore` (`UnifiedDiffViewer.java:205`). The same
  ignore policies as side-by-side, but `DO_NOT_HIGHLIGHT` is removed
  (`tools/util/text/SmartTextDiffProvider.java:244`). Unified can't turn highlighting off.
- Changes that are whitespace only and are ignored don't exist in `fragments`. Those lines become equal lines
  and are taken from the master. The whitespace difference on the left side is then invisible in Unified.
- `innerFragments == null`: a whole line in full color, no inline highlights.
- `innerFragments` empty list: the ignored color (lighter) on the lines and no inline highlights.
- `innerFragments` with content: the ignored color and inline highlights.
- The rule is `ignored = innerFragments != null` (`util/DiffDrawUtil.java:317`), the same as
  `SimpleDiffChangeUi` (`tools/simple/SimpleDiffChangeUi.java:90`).

---

## 2. Line numbers and gutter

File: `LineNumberConvertor.java`.

- Two converters, one per side. `Builder.put(masterStart, slaveStart, masterLen, slaveLen)`
  (`:121-124`) fills both `myFragments` (unified -> side) and `myInvertedFragments` (side -> unified).
  "Master" in the converter is the unified document, not the master side.
- `appendText` (`UnifiedFragmentBuilder.kt:123-133`):
  - equal lines: a row in both converters,
  - deleted: only `convertor1`,
  - added: only `convertor2`.
- `convertRaw` (`:89-111`):
  - strict: -1 outside a segment, and -1 if the segment has a different length on the two sides,
  - approximate: `floorEntry`, then `min(otherStart - start + value, otherStart + otherLength)`.
    A deleted line then maps to `start2` on the right side, not line by line.
- Gutter (`UnifiedDiffViewer.java:503-509`): `setLineNumberConverter(primary, additional)` with
  strict converters, combined with any content converter and hiding in folds
  (`DiffLineNumberConverter`, `util/DiffLineNumberConverter.java:39-48`).

| Line | Left column | Right column |
|---|---|---|
| Equal | old number | new number |
| Deleted | old number | empty |
| Added | empty | new number |
| Fake last line | empty | empty |
| Inside a collapsed fold | empty | empty |

Layout (`platform/platform-impl/src/com/intellij/openapi/editor/impl/EditorGutterComponentImpl.java`):
- One editor, no mirroring. The scrollbar and error stripe on the right, as in an ordinary editor.
- `paintLineNumbers` (`:863-871`): the old column first, then `GAP_BETWEEN_AREAS` = 5px (`:265`),
  then the new column. Both right-aligned in their area.
- `calcLineNumberAreaWidth` (`:2091-2102`): the left column is
  `max(getInitialLineNumberWidth, the width of the largest number)`, the right column only the width of the largest
  number. The largest number is found by walking backwards to the first line with a number
  (`DiffLineNumberConverter.getMaxLineNumber`, `:29-37`).
- Otherwise the same gutter as side-by-side: the diff color covers the whole gutter
  (`util/DiffLineMarkerRenderer.kt:33-79`).
- The titles for both sides are stacked on top of each other above the editor
  (`createTitles`, `UnifiedDiffViewer.java:269-277`).

---

## 3. Highlighting

### Background and words

`UnifiedDiffChangeUi.doInstallHighlighters` (`:74-81`) calls
`DiffDrawUtil.createUnifiedChunkHighlighters` (`util/DiffDrawUtil.java:311-347`):

```
if deleted and inserted are not empty:
  LineHighlighter(deleted,  DELETED,  ignored)
  LineHighlighter(inserted, INSERTED, ignored)
else only the one that exists

for frag in innerFragments (if not null):
  Inline(deletedStart  + frag.start1 .. frag.end1, DELETED)
  Inline(insertedStart + frag.start2 .. frag.end2, INSERTED)
```

- Unified never uses `MODIFIED` (blue). A modified block is gray above green.
- Inline highlights have a fixed type per part: gray on deleted lines, green on added ones. Side-by-side uses
  `DiffUtil.getDiffType(innerFragment)` per word (`SimpleDiffChangeUi.java:101-114`), so there a
  word can turn blue.
- Word offsets are relative to the line start of the first deleted and the first added unified line.
- The empty side of a word fragment gets the ordinary `installEmptyRangeRenderer` (a 2px stroke), in the DELETED or
  INSERTED color.
- No separator or border line between the deleted and added part. `PaintMode.DEFAULT` and `IGNORED` have
  `BorderType.NONE` (`DiffDrawUtil.java:778`), and no range is empty, so the 2px marker for empty
  sides is never used. `createNonSquashedChangesSeparator` only exists in side-by-side.

### Syntax

`UnifiedEditorHighlighter` (`:37-127`):
- Lexes the whole left and the whole right file with an ordinary `EditorHighlighter`.
- `init` walks through the `HighlightRange`s in order, takes tokens from the right side and moves them to
  unified offsets. Holes are filled with empty elements. Equal neighboring elements are merged (`addElement`).
- Deleted lines thus get the right lexer state from the old file, also inside block comments.
- `UnifiedEditorRangeHighlighter.processRange` (`:49`) does the same for markup from
  the original documents (inspections, breakpoints).

---

## 4. Folding

File: `UnifiedFoldingModel.java`, called from `apply` (`UnifiedDiffViewer.java:482-484`).

- The same `FoldingBuilderBase` as side-by-side, but with one side:
  `super(new int[]{lineCount}, ...)` (`:64`).
- The changes are `LineRange(change.line1, change.line2)` in unified lines, i.e. the whole block with
  the deleted and added part (`UnifiedFragmentBuilder.reportChange`, `:102-107`).
- The same context: `CONTEXT_RANGE` 4, levels 4/8/16, at least 2 lines, the first and last gap folded
  out to the edge (see 03, section 3). Since equal lines are 1:1, the folds end up on the same equal
  lines as in side-by-side.
- `lineCount` is `countNewLines(text) + 1`, with the fake last line.
- The description in the placeholder is taken from the master document via the master converter
  (`getDescription`, `:70-74`).
- Line numbers are hidden in both columns for folded lines (`hideLineNumberPredicate(0)`).
- `CONTEXT_RANGE` and `EXPAND_BY_DEFAULT` are shared `TextDiffSettings` per place, not per viewer.
- Open folds don't survive switching viewers. `ExpandSuggester.isExpanded` falls back to
  the default when the cache has a different number of editors (`tools/util/FoldingModelSupport.java:763-764`).
  Unified has 1, side-by-side 2.

---

## 5. Navigation, caret and switching viewers

### Next and previous change

- `MyPrevNextDifferenceIterable` (`UnifiedDiffViewer.java:1062-1082`) uses `line1` and `line2`.
  The same rules as in side-by-side (`tools/util/PrevNextDifferenceIterableBase.java`), but on
  unified lines.
- Landing: `scrollEditor(line1, animated = true)` (`PrevNextDifferenceIterableBase.java:26`).
  The caret goes to the first line of the block, i.e. the top of the deleted part. Then `ScrollType.CENTER`
  (`util/DiffUtil.java:504-529`). Not 1/3 like `getTargetOffsets` in side-by-side.
- Initial position: `scrollToChange(FIRST_CHANGE)` (`:923-933`), the same landing without animation.

### Current side

Unified has no focused side. The side is derived per line in `transferLineFromOneside` (`:606-638`):

```
lines = [convertor1.convert(line), convertor2.convert(line)]   // strict
side = master
if both -1:   approximate on both           // the fake last line
elif lines[0] == -1: lines[0] = approx; side = RIGHT   // an added line
elif lines[1] == -1: lines[1] = approx; side = LEFT    // a deleted line
return (lines, side)
```

The side is used for jumping to the source file (`getNavigatable`, `:1038-1048`, picks `pair.second`),
breadcrumbs and the imaginary editor for intentions.

### Caret across a rediff

`apply` saves the caret as a two-sided position before the new text is set (`:491-492`) and puts it back
with `transferLineToOneside(side, line)` afterwards (`:537-539`). The column is kept.

### Switching between Unified and side-by-side

A switch makes a new viewer on the same `DiffRequest`. The old viewer writes hints to the request in
`updateContextHints` on dispose, and the new one reads them in `processContextHints`
(`tools/util/base/InitialScrollPositionSupport.java:46-59`):

- `EDITORS_CARET_POSITION`: one `LogicalPosition` per side.
- `EditorsVisiblePositions`: the caret and scroll point per editor.

Unified to side-by-side (`MyInitialScrollHelper.getCaretPositions`, `:1248-1255`):
- Both carets are set: the exact side and the approximate other side. If the caret is on a deleted
  line, the left side gets the right line and the right side gets `start2`. On an added line, the left side gets `end1`.
- Which side gets focus comes from `PREFERRED_FOCUS_SIDE`, which only side-by-side writes
  (`tools/util/FocusTrackerSupport.java:56-63`). So Unified doesn't say whether the caret was in the deleted
  or the added part. The default is `RIGHT`.

Side-by-side to Unified (`doScrollToPosition`, `:1258-1274`):
- Only the master side's caret is used: `myMasterSide.selectNotNull(myCaretPosition)`, then
  `transferLineToOneside(RIGHT, line)`. If the user was in the left editor, that is ignored.

Scroll in both directions:
- `EditorsVisiblePositions` has the wrong length (1 vs 2 editors), so `isSame` fails
  (`InitialScrollPositionSupport.java:298-306`). The scroll position is not restored. The new viewer
  centers on the caret (`scrollToCaret`, `CENTER`).
- The order is `scrollToChange > line > context > caret position > first change`
  (`performDelayedSyncScroll`, `:145-154`). On a switch the first three are null, so the caret
  decides.
- If the user has only scrolled with the wheel without moving the caret, the view jumps to where the caret is.

---

## 6. Error stripe

- One stripe on the right. The markers come from `LineHighlighterBuilder.done`
  (`DiffDrawUtil.java:604-637`), one per highlighter.
- A modified block gives two markers: the DELETED color for the deleted part and the INSERTED color for the added part.
  No blue markers.

---

## 7. Rollback and copying the location

- A gutter icon per change on `line1`, i.e. the first line of the block (`createOperation`,
  `UnifiedDiffChangeUi.java:93-97`). With the right side writable, `Revert` is shown (`AllIcons.Diff.Revert`),
  which copies the left side in (`doInstallActionHighlighters`, `:60-72`).
- `replaceChange` (`UnifiedDiffViewer.java:852-870`) uses the `LineFragment` lines directly. The whole
  change is rolled back, whether the caret is in the deleted or the added part.
- The keyboard variant (`ReplaceSelectedChangesAction`, `:817-832`) picks the changes where the caret or
  selection hits `line1..line2` (`getSelectedChanges`, `:889-897`).
- After apply, the rediff runs synchronously (`handleMouseClick`, `UnifiedDiffChangeUi.java:130-145`).
- No separate "copy path:line". Jump to source (F4) and the like use `getNavigatable`:
  a deleted line gives the left file; added and equal lines give the right (see 5). If the left side has no navigatable
  (an old revision), the right side is used with the approximate line.

---

## 8. Added and deleted files

- `UnifiedDiffTool.createComponent` (`UnifiedDiffTool.java:16-20`) picks `SimpleOnesideDiffViewer`
  when one side is `EmptyContent` (`tools/util/side/OnesideDiffViewer.java:127-145`). So does
  `SimpleDiffTool`.
- Added and deleted files therefore look exactly the same in both modes. The chooser is still shown, since both
  tools can show the request.
- An empty file that exists is not `EmptyContent`. It goes to the ordinary Unified view with everything deleted
  or added.

---

## 9. Switching between viewers

- The chooser is a `SegmentedButtonComponent` in the diff toolbar (`impl/DiffRequestProcessor.java:241-250`,
  `createDiffToolChooser` `:915-929`). Two icon buttons, `AllIcons.Diff.SideBySide` and
  `AllIcons.Diff.Unified` (`FrameDiffTool.getIcon`), with the tooltips "Side-by-side viewer" and
  "Unified viewer".
- Hidden if only one tool fits or the tool is forced (`updateDiffToolChooser`, `:931-953`).
- The same choice is in the settings popup, Ctrl+Shift+D in the default keymap
  (`MyChangeDiffToolActionGroup`, `:956-985`, `Diff.ShowSettingsPopup`). No dedicated shortcut for switching.
- `switchToDiffTool` (`:379-389`) moves the chosen tool first in `myToolOrder`, saves the order and
  loads the request again.
- The choice is persistent per place, not globally: `DiffSettingsHolder.PlaceSettings.DIFF_TOOLS_ORDER`
  in `diff.xml` (`impl/DiffSettingsHolder.kt:31-34`). The place is `DiffUserDataKeys.PLACE`
  (`DiffRequestProcessor.java:202`).
- The default per place (`defaultPlaceSettings`, `DiffSettingsHolder.kt:94-106`): `ChangesView` and
  `VcsFileHistoryView` start in Unified, `VcsLogView` in side-by-side. Other places use
  the registration order, where `SimpleDiffTool` comes before `UnifiedDiffTool`
  (`DiffManagerImpl.kt:95-101`).
- The combined diff has its own Split/Unified chooser with the same mechanism
  (`tools/combined/CombinedDiffMainUI.kt:211-235`).

---

## 10. Easy to miss

- The fake last line. It has no number, is counted in `lineCount` for folding, and makes
  `canGoNext`'s check `line == lineCount - 1` hit it, not the last real line.
- A deleted line maps approximately to `start2`, not line by line like `BaseSyncScrollable`.
  Changes on line 0 with no equal lines before them give -1, which `getPosition` clamps to `(0,0)`
  (`UnifiedDiffViewer.java:1276-1279`).
- Strict conversion gives -1 when an equal segment has a different length on the two sides. Then the slave column becomes
  empty. Only happens with non-fair line fragments (comment in `appendTextMaster`, `:110-111`).
- Equal lines are shown from the master. Ignored whitespace on the left side disappears.
- The colors differ from side-by-side: gray above green instead of blue, also in the words and the stripe.
- The fold state and the scroll position are lost on a switch. The caret is transferred, but only the master side's from
  side-by-side.
- Much of the code exists only because Unified can be edited: guarded blocks for slave lines
  (`:523-533`), `MyOnesideDocumentListener` (`:655-709`) and `LineNumberConvertor.Corrector`
  (`:196-306`). None of that is needed in a read-only viewer.

---

## Mapping to jetdiff

| IntelliJ | jetdiff today | Unified |
|---|---|---|
| `UnifiedFragmentBuilder` | `FileDiff` with hunks and text per side | A row table over `FileDiff`: row = (side, line). Text, syntax and words already exist per side, so we don't need to build new text |
| `LineNumberConvertor` | `LineMapping` (sync scroll) | Per row: `old: Option<usize>`, `new: Option<usize>`. The inverse per side for side -> row |
| `UnifiedEditorHighlighter` | `FileDiff::line_syntax(side, line)` | Reused directly; the row points at the side's line |
| `createUnifiedChunkHighlighters` | `Block::diff_type`, `InlineHighlight::diff_type` | The deleted part is always `Deleted`, the added part always `Inserted`, also for words. `Block::ignored()` as before |
| `UnifiedFoldingModel` | `Folding` with two sides and paired regions | One side over rows, with the hunk's whole row interval as the change. `build_groups` can be used with one side |
| Gutter with two columns | `gutter_layout` with one number area, Left mirrored | Old column, 5px gap, new column. Both right-aligned, not mirrored |
| Error stripe | `stripe_markers` with one mark per hunk | Two marks for Modified hunks |
| `transferLineFromOneside` | `Cursor { side, line }` | The side can be derived per row: deleted = Left, added and equal = Right |
| No divider | `Divider`, `Connector` | Not used |
| `SimpleOnesideDiffViewer` | `FileDiff::single_side` | The same view in both diff modes |

Choices where IntelliJ is not a good model:
- On a switch we can use the cursor's actual side, not just the master. With a single `Cursor` that's free.
- We can keep the cursor line at the same place in the viewport on a switch, as `DiffState::rescale` does on
  zoom. IntelliJ centers on the caret.
- Folds can be kept on a switch. Equal lines are 1:1, so a fold in side-by-side has an unambiguous fold in
  Unified.
- Landing on next/previous hunk: IntelliJ uses `CENTER` in Unified and 1/3 in side-by-side. The same
  1/3 in both makes the switch calmer.
- `y` (`DiffState::location`) works unchanged. A deleted row has the Left side, and `location` transfers
  to Right. IntelliJ gives `start2` here; our `LineMapping::transfer` goes line by line in the hunk.
- `r` (rollback) works unchanged via `cursor_block`, since the whole hunk is rolled back in IntelliJ too.
- Toggle: IntelliJ stores the choice per place. We only have one place, so one key in `config.toml` is enough.

Chosen when Unified was built:
- No folding in Unified, so folds are not kept on a switch. Split builds the folding again.
- Added and deleted files are always shown Unified, and `v` does nothing there. Split therefore never shows one
  side alone.
