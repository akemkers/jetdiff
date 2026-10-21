# 03 - Scroll, folding and structure in the JetBrains diff viewer

Source: `~/git/_ref/intellij-community`. Paths are relative to
`platform/diff-impl/src/com/intellij/diff/` unless stated otherwise.

Terms:
- **Range** = `(start1, end1, start2, end2)` in logical lines, half-open. An empty side means `start == end`.
- **Master** = the editor that generated the scroll event. **Slave** = the other one.
- **Visual line** = a line after folding/soft wrap. Logical line = a line in the document.

---

## 1. Sync scroll

Files: `tools/util/SyncScrollSupport.java`, `tools/util/BaseSyncScrollable.java`,
`tools/simple/SimpleDiffViewer.java`.

### Setup

- `TwosideSyncScrollSupport` has two `ScrollHelper`s: LEFT->RIGHT and RIGHT->LEFT.
  Whichever fires `visibleAreaChanged` is the master (`SyncScrollSupport.java:~84`).
- Re-entrancy is stopped with a depth counter (`enterDisableScrollSection` /
  `exitDisableScrollSection`, `isDuringSyncScroll`). When we scroll the slave,
  the slave's own event is ignored.
- `ScrollHelper.visibleAreaChanged` (`:326`) skips if the slave is in the middle of a
  batch folding operation. X is synced raw (the same x), Y via line mapping.

### Line mapping (`BaseSyncScrollable.java`)

`SimpleDiffViewer.MySyncScrollable` (`SimpleDiffViewer.java:875-902`) feeds in
boundary points: `(0,0)`, then `(start1,start2)` and `(end1,end2)` for each change,
and finally `(lineCount1,lineCount2)`. `getRange(side, line)` (`:31-45`, via
`process` `:64-72`) finds the pair of neighboring boundaries that enclose the line.
Between two boundaries there is either a change or an equal stretch.

```
transferLine(line, r):                     // BaseSyncScrollable.java:91-97
  if line == r.start1: return r.start2
  if line == r.end1:   return r.end2
  if r.end1 < line:    return line - r.end1 + r.end2     // after the last boundary
  return min(r.start2 + (line - r.start1), r.end2)       // clamped into the block
```

Note: inside a change, lines map one by one from the top and are clamped against
`end2`. So a 10-line change against 2 lines stands still on the slave for the
last 8 lines.

### Visual transfer: how folding is handled

```
transferVisualLine(masterVisual):          // SyncScrollSupport.java:398-412
  logical = master.visualToLogical(masterVisual)
  r = getRange(masterSide, logical)
  vr = r with all ends converted logical->visual in their own editor
  return transferLine(masterVisual, vr)
```

Since the boundaries are converted to visual lines, folded regions come out right
without special code.

### Vertical sync (`syncVerticalScroll`, `:355-396`)

The anchor point is 1/3 down the viewport, not the top.

```
syncVertical(master, slave):
  if forceSyncVerticalScroll:              // aligned mode, :366
    slave.scrollY(master.viewRect.y); return
  h   = master.viewRect.height
  lh  = lineHeight
  anchorY = master.viewRect.y + h/3        // :369
  mv = master.yToVisualLine(anchorY)
  sv = transferVisualLine(mv)
  correction = (anchorY - master.visualLineToY(mv)) % lh   // the phase inside the line
  target = slave.visualLineToY(sv) - h/3 + correction

  // jitter guard: if we are in an empty block on the slave, only move by whole lines
  onlyMajorBackward = correction <  lh/2 && mv > 0 && sv == transferVisualLine(mv-1)
  onlyMajorForward  = correction >= lh/2 &&           sv == transferVisualLine(mv+1)

  if anchor != null:                       // during an animated makeVisible, :388-391
    p = (master.y - anchor.masterStart) / (anchor.masterEnd - anchor.masterStart)
    target = lerp(anchor.slaveStart, anchor.slaveEnd, p)

  target += slave.headerHeight - master.headerHeight      // :394
  doScrollVertically(slave, target, onlyMajorForward, onlyMajorBackward)
```

`doScrollVertically` (`:429-443`): with `onlyMajor*`, movements smaller
than `lineHeight` in that direction are ignored. This is what keeps the slave from
"vibrating" when the master scrolls through a pure insert. Animation is turned off for
the slave scroll (`disableAnimation`/`enableAnimation`); the slave follows the master
directly every frame.

### makeVisible: the animated jump to a change (`doMakeVisible`, `:232-293`)

```
makeVisible(masterSide, ranges):
  targets = getTargetOffsets(editors, ranges)     // one y per editor
  for helper: helper.anchor = (currentY, targetY) for master and slave
  master.scrollX(0)
  master.scrollY(targets[master], animated)
  onScrollingFinished:
    remove the anchors
    for each slave where y != target: scroll (animated only if the master didn't move)
```

During the animation, the slave interpolates linearly by the master's progress, so
both sides land at the same time even if the distances differ.

### getTargetOffsets (`:467-529`): placing a change in the viewport

```
gap = 2 * lh
for each editor:
  rangeH = y(end) - y(start)
  if 2*gap + rangeH <= editorH:  shift = min(editorH - gap - rangeH, editorH/3)
  else:                          shift = gap       // a large change: top + 2 lines of space
shift = min over editors
target = max(0, y(start) - shift)
if some editor passes max scroll: a fallback that keeps the top of the range visible
```

Small changes end up about 1/3 down, the same point as the sync anchor. That means
sync scroll doesn't move anything after the jump.

### Aligned mode (`tools/simple/AlignedDiffModel.kt`)

- Off by default (`TextDiffSettingsHolder.kt:38`). Forced on in the combined view
  (see 6).
- Block inlays above and below each change pad out the shorter side
  (`updateInlayHeights`, `:328-370`, height = the delta in visual lines * lh).
  Mirror inlays on the opposite side.
- Then both documents are equally tall, and sync is just `slave.y = master.y`.
- Realign is debounced 300 ms (`:115`) and preserves the scroll (`:171-195`).

---

## 2. Line transfer for caret and scroll

- The caret is not synced continuously. Only on explicit actions.
- `transferPosition` (`tools/util/side/TwosideTextDiffViewer.java:258-262`):
  the same `transferLine`; the column is kept.
- `scrollToLine` (`:265-271`): moves the caret on the other side to the transferred line
  and scrolls CENTER.
- Switching focus between sides (`FocusOppositePane`, `:300-323`): transfer the caret,
  `ScrollType.MAKE_VISIBLE`.
- Unified: `transferLineFromOneside` / `transferLineToOneside`
  (`tools/fragmented/UnifiedDiffViewer.java:606-638`) via `LineNumberConvertor`
  (see 5). The caret is preserved across a rediff by going via the two-sided position
  (`:491-539`).
- Initial position (`tools/util/base/InitialScrollPositionSupport.java:107-166`):
  priority `scrollToChange > line > context > saved position > first change`.
  Skipped if the user has already scrolled.

---

## 3. Folding of unchanged fragments

File: `tools/util/FoldingModelSupport.java`.

### Building

```
build(changes, lineCount, contextRange):          // :255-282
  previous = MIN_VALUE
  for c in changes:
    addGroup(gap = [previous, c.start], lineCount)
    previous = c.end
  addGroup([previous, MAX_VALUE])

addGroup(gap):                                    // :284-303
  for level in 0..2:
    shift = contextRange << level                 // getRangeShift :1234-1241, 4/8/16
    start = clamp(gap.start + shift, 0, lineCount)
    end   = clamp(gap.end   - shift, 0, lineCount)
    if end - start >= 2: make a fold [start, end)    // :310
```

- Three nested levels per gap. The outermost has the least context. "Expand" opens one
  level at a time, so the user gets 4, then 8, then 16 lines of context.
- The first and last gap have only one side that shrinks (the MIN/MAX sentinel), so
  the start and end of the file are folded all the way down to the context.
- The placeholder is `"     "` (`addFolding`, `:469-489`). Highlighters inside
  the fold are muted.
- Line numbers for folded lines are hidden (`hideLineNumberPredicate` `:547`,
  `util/DiffLineNumberConverter.java:39-48`).

### Sync between the sides

- Each fold has a paired region on the other side. `MyFoldingListener`
  (`:610-651`) mirrors expand/collapse in `onFoldProcessingEnd`.
- All folding changes run in `runBatchOperation` (`:491`). Sync scroll
  ignores events during the batch (see 1), and the scroll is preserved around it.

### State across a rediff

- `ExpandSuggester` + a cache (`:742-830`) store which regions were open
  per request (`CACHE_KEY`). After a rediff, the new folds are matched against the old ones, so
  open regions stay open.

### Settings (`tools/util/base/TextDiffSettingsHolder.kt`)

- `CONTEXT_RANGE_MODES = {1,2,4,8,-1}` (`:30`), -1 = never fold.
- Default `CONTEXT_RANGE=4` (`:60`), `EXPAND_BY_DEFAULT=true` (`:59`).
- Per place (`:200-225`): the commit dialog, the VCS log and the changes view have
  `EXPAND_BY_DEFAULT=false`, i.e. folded. That's where you see many files.

---

## 4. Navigation: next/prev and the next file

- `PrevNextDifferenceIterableBase.java:38-103`:
  next = the first change with `start > caretLine`. prev = the first change where the next
  change has `end > caretLine` or `start >= caretLine`, i.e. the last change
  that lies entirely above the caret (`:86-103`). canGoPrev is false if the caret is in
  or above the first change.
- `SimpleDiffViewer.scrollToChange` (`:451-466`): move both carets to the
  change start, then `makeVisible` (animated, see 1). Unified uses
  `DiffUtil.scrollEditor` with `ScrollType.CENTER` (`util/DiffUtil.java:497-535`).
- Next file (`actions/impl/DiffDifferenceNavigationAction.kt:74-95`,
  `impl/DiffRequestProcessor.java` `DiffNavigator` `:1547-1600`):

```
onNext():
  if viewer.canGoNext(): viewer.goNext(); state = DEFAULT; return
  if !GO_TO_NEXT_FILE_ON_NEXT_DIFFERENCE: return      // default true, DiffSettingsHolder.kt:28
  if state == NEXT_PRESSED_ONCE: goToNextFile(scrollTo = FIRST_CHANGE); state = DEFAULT
  else: showHint("Press again to go to the next file")  // below the last change, :1604-1640
        state = NEXT_PRESSED_ONCE
```

  A new file opens with `ScrollToPolicy.FIRST_CHANGE`; prev gives `LAST_CHANGE`
  (`DiffRequestProcessor.java:1078-1082`). The state (`NEXT | PREV | NONE`)
  is reset after an ordinary goNext/goPrev and on `reset()`.

---

## 5. The Unified viewer: how the document is built

Files: `tools/fragmented/UnifiedFragmentBuilder.kt`,
`LineNumberConvertor.java`, `UnifiedEditorHighlighter.java`,
`UnifiedDiffViewer.java`.

```
build(changes, left, right, master = RIGHT):
  last = (0,0)
  for c in changes:
    processEquals(last -> c.start)   // equal lines are written once, from the master
    blockStart    = curLine
    appendText(LEFT,  c.start1..c.end1)   // deleted
    insertedStart = curLine
    appendText(RIGHT, c.start2..c.end2)   // inserted
    blocks += BlockLineRange(blockStart, insertedStart, curLine)
    last = c.end
  processEquals(last -> EOF)

appendText(side, lines):                  // :123-149
  convertor[side].put(unifiedLine, originalLine, len)
  highlightRanges += HighlightRange(side, unifiedOffsets, originalOffsets)
```

- The master is the editable side, RIGHT by default (`UnifiedDiffViewer.java:155-179`).
  Non-master lines are read-only via guarded blocks (`:523-533`).
- `LineNumberConvertor`: a `TreeMap<unifiedStart, (origStart, len)>` per side
  plus the inverse. `convertRaw` (`:89-111`): strict gives -1 outside a segment;
  approximate uses `floorEntry` and clamps. Strict is used for the gutter,
  approximate for caret/scroll transfer.
- Gutter: two line number columns, each with its own converter
  (`UnifiedDiffViewer.java:503-509`). A deleted line has only the left number.
- Syntax highlighting (`UnifiedEditorHighlighter.java:40-120`): both
  original documents are lexed in full, and the tokens are clipped into the unified
  document via the `HighlightRange` offsets. Deleted lines thus get the
  correct lexer state from the old file.
- The whole build happens in the background. The result is a Runnable that sets the text,
  highlighter and folds on the EDT (`apply`, `:457-554`).
- Folding is one-sided over the unified lines (`UnifiedFoldingModel.java:25-92`),
  the same algorithm as in 3.

---

## 6. The combined diff viewer and the change list tree

### Combined viewer (`tools/combined/`)

One outer `JBScrollPane` over `CombinedDiffBlocksPanel`
(`CombinedDiffViewer.kt:101-108`). Each file is a block with a header and a full
diff viewer where the editors have full height (no internal scroll,
`additionalLines=0`, no extra page at the bottom, `:951-967`). The last block is
at least as tall as the viewport (`:102`), so it can be scrolled to the top.

Block holders (`CombinedDiffBlocksPanel.kt:25-187`):
- `null`: the default placeholder height of 150 px (`CombinedDiffLoadingBlock.kt:35`)
- `Placeholder(height)`: the remembered height from the last time it was loaded
- `RealComponent`: the actual viewer

The layout is its own vertical stacking. `getBlockBounds` sums the heights.

Lazy loading:
```
onViewportChanged:                         // :302-312
  updateStickyHeader()                     // :443-478
  visibleBlocksQueue.queue(100ms merge)    // :169
processBlocksState():                      // :366-432
  keep = blocks in the viewport ± 3        // combined.diff.visible.viewport.delta
       + blocks around the current one
  load keep (background, CombinedDiffModel.kt:83-150)
  unload the rest above the limit (combined.diff.loaded.content.limit), keep the height
```
- The loading indicator is shown only after 200 ms (`CombinedDiffUI.kt`).
- When heights change, the content is kept stable by anchoring on a block
  (`runPreservingViewportContent`, `:885-933`).
- Side-by-side blocks are always built with `ALIGNED_TWO_SIDED_DIFF=true`
  (`CombinedDiffComponentProcessorImpl.kt:217`). Both sides are equally tall,
  so no sync scroll is needed inside a block.
- Navigation (`:314-362`): next within the block, otherwise the next block with
  `goFirst`. The outer `ScrollingModelImpl` translates the editor position into the outer
  viewport (`:852-881`).
- The block id is `CombinedPathBlockId(path, fileStatus)` (`CombinedDiffBlocks.kt:204`).
  The header is sticky, and the body can be collapsed (`:206-298`).

### Change list tree

Files in `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/`.

```
build(changes):
  sort the changes by path length         // TreeModelBuilder.java:88, parents first
  for c: insertChangeNode(c)              // :376-398
    parent = grouping.getParentNode(c)    // policy chain, ChangesGroupingSupport.kt:72-80
  sort the whole tree (BROWSER_NODE_COMPARATOR: sortWeight, class, name)  // :405-427
  collapseDirectories()                   // :442-497

DirectoryPolicy.getParentNode(path):      // DirectoryChangesGroupingPolicy.kt:24-55
  walk up the parent paths until a node exists in DIRECTORY_CACHE
  create the missing folder nodes top-down, put them in the cache

collapseDirectories(node):
  while a folder has exactly one child and the child is a folder: merge into "a/b/c"
```

- Grouping keys: directory, module, repository, none
  (`ChangesGroupingSupport.kt:92-95`).
- Sort weights (`ChangesBrowserNode.java:58-71`): module 4, directory 5,
  file path 6, change 7, unversioned 9, ignored 11. Folders before files.
- A folder shows the number of files in gray (`getCountText`, `:272-291`, cached
  `:200-216`). In flat mode, the parent path is shown in gray after the file name (`:348`).
- File names are colored by `FileStatus` (`ChangesBrowserChangeNode.java:51-81`).

File status colors (`platform/platform-resources/src/themes/expUI/`):

| Status | Light | Dark |
|---|---|---|
| ADDED | `#067D17` | `#73BD79` |
| DELETED | `#6C707E` | `#6F737A` |
| MODIFIED / RENAMED | `#0033B3` | `#70AEFF` |
| UNKNOWN (unversioned) | `#B23247` | `#E88F89` |
| IGNORED | `#8C4F00` | `#D69A6B` |
| MERGED | `#643CB8` | `#CF84CF` |
| Conflict | `#DE1B2E` | `#DE6A66` |

Light: `expUI_lightScheme.xml:51-67`. Dark: `expUI_darkScheme.xml:55-72`.
The status list: `platform/editor-ui-api/src/com/intellij/openapi/vcs/FileStatus.java:29-61`.

Tree -> diff preview (`platform/vcs-impl/src/.../TreeHandlerDiffRequestProcessor.kt:76-218`):
selection and model changes are debounced 100 ms (restarted on each change).
In combined mode, the selection only picks a block (scroll), no new request.

---

## 7. What makes it smooth

- **Background work with a short synchronous wait.** `DiffViewerBase.rediff`
  (`tools/util/base/DiffViewerBase.java:145-168`) runs the diff in the background
  and waits synchronously up to `DEFAULT_PROGRESS_DELAY_MILLIS` (about 200 ms) if
  the window has focus. A fast diff gives no flicker. A slow diff shows a busy
  indicator (`onSlowRediff`). The result is a Runnable applied on the EDT
  in one operation.
- **Debounced rediff.** `scheduleRediff` (`:122-130`) via an Alarm on
  document change. Cancelled when the viewer is hidden (`:76-89`).
- **Request cache.** `impl/CacheDiffRequestProcessor.java:40-41`:
  `SoftHardCacheMap(5, 5)`, 5 hard and 5 soft references. Switching back to
  a recent file is free. Loading a new request also waits briefly before
  "Loading..." is shown (`:116-124`).
- **Preserve position.** All applying happens in `runPreservingScrollingPosition`
  (`SimpleDiffViewer.java:347-401`, `EditorScrollingPositionKeeper`). The fold state
  is preserved via `ExpandSuggester`. The caret via transfer.
- **The jitter guard and the 1/3 anchor** in sync scroll (see 1).
- **Animated makeVisible with an interpolated slave** (see 1).
- **Combined:** placeholders with remembered heights, lazy load of the viewport ± 3,
  a 100 ms merging queue, an anchored layout on height changes.
- **Unified highlighting** lexes the whole original files, not the unified text.

---

## Recommendations for jetdiff (GPUI)

1. **One scroll model per viewer.** Store `scroll_y` for the master, compute the slave
   in the same frame from `transfer_visual_line`. No event ping-pong, so
   the re-entrancy counter isn't needed.
2. **Start with aligned mode as the default.** Pad the shorter side with empty rows
   in the layout (padding rows instead of inlays). Then sync scroll becomes
   `y_left == y_right`, the dividers become simple, and the same code can be used in the
   combined view. Add non-aligned with the 1/3 anchor and the onlyMajor guard later.
3. **A visual row model.** Build a `Vec<Row>` per side where each row is
   `Line(n) | Fold{range, level} | Pad`. Folding, padding and transfer become
   index lookups. Binary search on the cumulative height for y->row.
4. **Folding:** three levels with 4/8/16 lines of context, at least 2 lines, a shared
   expand state for both sides, remember the state per file across a rediff.
5. **Navigation:** place the change 1/3 down (the `getTargetOffsets` formula),
   animate with lerp. Press twice for the next file.
6. **Commit view:** one outer scroll over blocks with known or estimated
   heights. Compute the diff lazily for the viewport ± 3, keep the height on unload.
   A sticky file header.
7. **Tree:** sort by path, folders before files, merge single-child folders,
   color file names by file status (the table above), gray count.
8. **Background:** diff and highlighting on a background thread, wait synchronously at most
   ~200 ms before a spinner, apply the result atomically. An LRU cache of about 5-10 files.
