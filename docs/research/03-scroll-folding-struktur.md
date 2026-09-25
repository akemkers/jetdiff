# 03 - Scroll, folding og struktur i JetBrains diff-viewer

Kilde: `~/git/_ref/intellij-community`. Stier er relative til
`platform/diff-impl/src/com/intellij/diff/` hvis ikke annet står.

Begreper:
- **Range** = `(start1, end1, start2, end2)` i logiske linjer, halvåpen. Tom side betyr `start == end`.
- **Master** = editoren som genererte scroll-eventet. **Slave** = den andre.
- **Visuell linje** = linje etter folding/soft wrap. Logisk linje = linje i dokumentet.

---

## 1. Sync scroll

Filer: `tools/util/SyncScrollSupport.java`, `tools/util/BaseSyncScrollable.java`,
`tools/simple/SimpleDiffViewer.java`.

### Oppsett

- `TwosideSyncScrollSupport` har to `ScrollHelper`: LEFT->RIGHT og RIGHT->LEFT.
  Den som fyrer `visibleAreaChanged` er master (`SyncScrollSupport.java:~84`).
- Re-entrancy stoppes med en dybdeteller (`enterDisableScrollSection` /
  `exitDisableScrollSection`, `isDuringSyncScroll`). Når vi scroller slave
  ignoreres slave sitt eget event.
- `ScrollHelper.visibleAreaChanged` (`:326`) hopper over hvis slave er midt i en
  batch folding-operasjon. X synkes rått (samme x), Y via linje-mapping.

### Linje-mapping (`BaseSyncScrollable.java`)

`SimpleDiffViewer.MySyncScrollable` (`SimpleDiffViewer.java:875-902`) mater inn
grensepunkter: `(0,0)`, så `(start1,start2)` og `(end1,end2)` for hver change,
til slutt `(lineCount1,lineCount2)`. `getRange(side, line)` (`:31-45`, via
`process` `:64-72`) finner paret av naboliggende grenser som omslutter linjen.
Mellom to grenser er det enten en change eller et likt stykke.

```
transferLine(line, r):                     // BaseSyncScrollable.java:91-97
  if line == r.start1: return r.start2
  if line == r.end1:   return r.end2
  if r.end1 < line:    return line - r.end1 + r.end2     // etter siste grense
  return min(r.start2 + (line - r.start1), r.end2)       // klemt inn i blokken
```

Merk: inne i en change mappes linje for linje fra toppen og klemmes mot
`end2`. En 10-linjers endring mot 2 linjer står altså stille på slave de
siste 8 linjene.

### Visuell transfer, slik håndteres folding

```
transferVisualLine(masterVisual):          // SyncScrollSupport.java:398-412
  logical = master.visualToLogical(masterVisual)
  r = getRange(masterSide, logical)
  vr = r med alle ender konvertert logical->visual i hver sin editor
  return transferLine(masterVisual, vr)
```

Siden grensene konverteres til visuelle linjer blir foldede regioner riktige
uten spesialkode.

### Vertikal sync (`syncVerticalScroll`, `:355-396`)

Ankerpunktet er 1/3 ned i viewporten, ikke toppen.

```
syncVertical(master, slave):
  if forceSyncVerticalScroll:              // aligned mode, :366
    slave.scrollY(master.viewRect.y); return
  h   = master.viewRect.height
  lh  = lineHeight
  anchorY = master.viewRect.y + h/3        // :369
  mv = master.yToVisualLine(anchorY)
  sv = transferVisualLine(mv)
  correction = (anchorY - master.visualLineToY(mv)) % lh   // fase inne i linjen
  target = slave.visualLineToY(sv) - h/3 + correction

  // jitter-vakt: står vi i en tom blokk på slave, flytt bare ved hele linjer
  onlyMajorBackward = correction <  lh/2 && mv > 0 && sv == transferVisualLine(mv-1)
  onlyMajorForward  = correction >= lh/2 &&           sv == transferVisualLine(mv+1)

  if anchor != null:                       // under animert makeVisible, :388-391
    p = (master.y - anchor.masterStart) / (anchor.masterEnd - anchor.masterStart)
    target = lerp(anchor.slaveStart, anchor.slaveEnd, p)

  target += slave.headerHeight - master.headerHeight      // :394
  doScrollVertically(slave, target, onlyMajorForward, onlyMajorBackward)
```

`doScrollVertically` (`:429-443`): med `onlyMajor*` ignoreres bevegelser
mindre enn `lineHeight` i den retningen. Det er dette som gjør at slave ikke
"vibrerer" når master scroller gjennom en ren insert. Animasjon slås av for
slave-scroll (`disableAnimation`/`enableAnimation`), slave følger master
direkte hver frame.

### makeVisible, animert hopp til en change (`doMakeVisible`, `:232-293`)

```
makeVisible(masterSide, ranges):
  targets = getTargetOffsets(editors, ranges)     // en y per editor
  for helper: helper.anchor = (currentY, targetY) for master og slave
  master.scrollX(0)
  master.scrollY(targets[master], animated)
  onScrollingFinished:
    fjern anchors
    for slave der y != target: scroll (animert bare hvis master ikke flyttet seg)
```

Under animasjonen interpolerer slave lineært etter masterens progresjon, så
begge sider lander samtidig selv om distansene er ulike.

### getTargetOffsets (`:467-529`), plassering av change i viewporten

```
gap = 2 * lh
for hver editor:
  rangeH = y(end) - y(start)
  if 2*gap + rangeH <= editorH:  shift = min(editorH - gap - rangeH, editorH/3)
  else:                          shift = gap       // stor change: topp + 2 linjer luft
shift = min over editors
target = max(0, y(start) - shift)
if noen editor passerer maks scroll: fallback som holder toppen av range synlig
```

Små changes havner ca. 1/3 ned, samme punkt som sync-ankeret. Det gjør at
sync scroll ikke flytter noe etter hoppet.

### Aligned mode (`tools/simple/AlignedDiffModel.kt`)

- Av som default (`TextDiffSettingsHolder.kt:38`). Tvinges på i combined view
  (se 6).
- Block inlays over og under hver change fyller opp den korteste siden
  (`updateInlayHeights`, `:328-370`, høyde = delta i visuelle linjer * lh).
  Speil-inlays på motsatt side.
- Da er begge dokumentene like høye, og sync er bare `slave.y = master.y`.
- Realign debounces 300 ms (`:115`) og bevarer scroll (`:171-195`).

---

## 2. Line transfer for caret og scroll

- Caret synkes ikke kontinuerlig. Bare ved eksplisitte handlinger.
- `transferPosition` (`tools/util/side/TwosideTextDiffViewer.java:258-262`):
  samme `transferLine`, kolonne beholdes.
- `scrollToLine` (`:265-271`): flytter caret på andre side til overført linje
  og scroller CENTER.
- Bytt fokus mellom sider (`FocusOppositePane`, `:300-323`): overfør caret,
  `ScrollType.MAKE_VISIBLE`.
- Unified: `transferLineFromOneside` / `transferLineToOneside`
  (`tools/fragmented/UnifiedDiffViewer.java:606-638`) via `LineNumberConvertor`
  (se 5). Caret bevares over rediff ved å gå via to-sidig posisjon
  (`:491-539`).
- Initial posisjon (`tools/util/base/InitialScrollPositionSupport.java:107-166`):
  prioritet `scrollToChange > linje > context > lagret posisjon > første change`.
  Hoppes over hvis bruker allerede har scrollet.

---

## 3. Folding av uendrede fragmenter

Fil: `tools/util/FoldingModelSupport.java`.

### Bygging

```
build(changes, lineCount, contextRange):          // :255-282
  forrige = MIN_VALUE
  for c in changes:
    addGroup(gap = [forrige, c.start], lineCount)
    forrige = c.end
  addGroup([forrige, MAX_VALUE])

addGroup(gap):                                    // :284-303
  for level in 0..2:
    shift = contextRange << level                 // getRangeShift :1234-1241, 4/8/16
    start = clamp(gap.start + shift, 0, lineCount)
    end   = clamp(gap.end   - shift, 0, lineCount)
    if end - start >= 2: lag fold [start, end)    // :310
```

- Tre nestede nivåer per gap. Ytterst har minst kontekst. "Expand" åpner ett
  nivå om gangen, så brukeren får 4, så 8, så 16 linjer kontekst.
- Første og siste gap har bare én side som krymper (MIN/MAX-sentinel), så
  starten og slutten av filen foldes helt ned til kontekst.
- Placeholder er `"     "` (`addFolding`, `:469-489`). Highlightere inne i
  folden dempes.
- Linjenummer for foldede linjer skjules (`hideLineNumberPredicate` `:547`,
  `util/DiffLineNumberConverter.java:39-48`).

### Sync mellom sider

- Hver fold har en parret region på andre side. `MyFoldingListener`
  (`:610-651`) speiler expand/collapse i `onFoldProcessingEnd`.
- Alle folding-endringer kjøres i `runBatchOperation` (`:491`). Sync scroll
  ignorerer events under batch (se 1), og scroll bevares rundt.

### Tilstand over rediff

- `ExpandSuggester` + cache (`:742-830`) lagrer hvilke regioner som var åpne
  per request (`CACHE_KEY`). Etter rediff matches nye folds mot gamle, så
  åpne regioner forblir åpne.

### Innstillinger (`tools/util/base/TextDiffSettingsHolder.kt`)

- `CONTEXT_RANGE_MODES = {1,2,4,8,-1}` (`:30`), -1 = aldri fold.
- Default `CONTEXT_RANGE=4` (`:60`), `EXPAND_BY_DEFAULT=true` (`:59`).
- Per sted (`:200-225`): commit-dialog, VCS log og changes view har
  `EXPAND_BY_DEFAULT=false`, altså foldet. Det er der man ser mange filer.

---

## 4. Navigasjon next/prev og neste fil

- `PrevNextDifferenceIterableBase.java:38-103`:
  next = første change med `start > caretLine`. prev = første change der neste
  change har `end > caretLine` eller `start >= caretLine`, altså siste change
  som ligger helt over caret (`:86-103`). canGoPrev er false hvis caret står i
  eller over første change.
- `SimpleDiffViewer.scrollToChange` (`:451-466`): flytt begge carets til
  change-start, så `makeVisible` (animert, se 1). Unified bruker
  `DiffUtil.scrollEditor` med `ScrollType.CENTER` (`util/DiffUtil.java:497-535`).
- Neste fil (`actions/impl/DiffDifferenceNavigationAction.kt:74-95`,
  `impl/DiffRequestProcessor.java` `DiffNavigator` `:1547-1600`):

```
onNext():
  if viewer.canGoNext(): viewer.goNext(); state = DEFAULT; return
  if !GO_TO_NEXT_FILE_ON_NEXT_DIFFERENCE: return      // default true, DiffSettingsHolder.kt:28
  if state == NEXT_PRESSED_ONCE: goToNextFile(scrollTo = FIRST_CHANGE); state = DEFAULT
  else: showHint("Press again to go to the next file")  // under siste change, :1604-1640
        state = NEXT_PRESSED_ONCE
```

  Ny fil åpnes med `ScrollToPolicy.FIRST_CHANGE`, prev gir `LAST_CHANGE`
  (`DiffRequestProcessor.java:1078-1082`). State (`NEXT | PREV | NONE`)
  nullstilles etter vanlig goNext/goPrev og ved `reset()`.

---

## 5. Unified viewer, hvordan dokumentet bygges

Filer: `tools/fragmented/UnifiedFragmentBuilder.kt`,
`LineNumberConvertor.java`, `UnifiedEditorHighlighter.java`,
`UnifiedDiffViewer.java`.

```
build(changes, left, right, master = RIGHT):
  last = (0,0)
  for c in changes:
    processEquals(last -> c.start)   // like linjer skrives én gang, fra master
    blockStart    = curLine
    appendText(LEFT,  c.start1..c.end1)   // slettede
    insertedStart = curLine
    appendText(RIGHT, c.start2..c.end2)   // innsatte
    blocks += BlockLineRange(blockStart, insertedStart, curLine)
    last = c.end
  processEquals(last -> EOF)

appendText(side, lines):                  // :123-149
  convertor[side].put(unifiedLine, originalLine, len)
  highlightRanges += HighlightRange(side, unifiedOffsets, originalOffsets)
```

- Master er den redigerbare siden, default RIGHT (`UnifiedDiffViewer.java:155-179`).
  Ikke-master-linjer er read-only via guarded blocks (`:523-533`).
- `LineNumberConvertor`: `TreeMap<unifiedStart, (origStart, len)>` per side
  pluss invers. `convertRaw` (`:89-111`): strict gir -1 utenfor et segment,
  approximate bruker `floorEntry` og klemmer. Strict brukes til gutter,
  approximate til caret/scroll-transfer.
- Gutter: to linjenummer-kolonner, hver med sin konverter
  (`UnifiedDiffViewer.java:503-509`). Slettet linje har bare venstre nummer.
- Syntax highlighting (`UnifiedEditorHighlighter.java:40-120`): begge
  originaldokumenter lexes i sin helhet, og tokens klippes inn i unified
  dokumentet via `HighlightRange`-offsetene. Slettede linjer får dermed
  korrekt lexer-state fra gammel fil.
- Hele byggingen skjer i bakgrunn. Resultatet er en Runnable som setter tekst,
  highlighter og folds på EDT (`apply`, `:457-554`).
- Folding er ensidig over unified-linjer (`UnifiedFoldingModel.java:25-92`),
  samme algoritme som i 3.

---

## 6. Combined diff viewer og change-list tree

### Combined viewer (`tools/combined/`)

Én ytre `JBScrollPane` over `CombinedDiffBlocksPanel`
(`CombinedDiffViewer.kt:101-108`). Hver fil er en blokk med header og en full
diff viewer der editorene har full høyde (ingen intern scroll,
`additionalLines=0`, ingen ekstra side nederst, `:951-967`). Siste blokk er
minst viewport-høy (`:102`) så man kan scrolle den til toppen.

Blokk-holdere (`CombinedDiffBlocksPanel.kt:25-187`):
- `null`: default placeholder-høyde 150 px (`CombinedDiffLoadingBlock.kt:35`)
- `Placeholder(height)`: husket høyde fra forrige gang den var lastet
- `RealComponent`: faktisk viewer

Layout er egen vertikal stabling. `getBlockBounds` summerer høyder.

Lazy loading:
```
onViewportChanged:                         // :302-312
  updateStickyHeader()                     // :443-478
  visibleBlocksQueue.queue(100ms merge)    // :169
processBlocksState():                      // :366-432
  keep = blokker i viewport ± 3            // combined.diff.visible.viewport.delta
       + blokker rundt current
  last inn keep (bakgrunn, CombinedDiffModel.kt:83-150)
  last ut resten over limit (combined.diff.loaded.content.limit), behold høyde
```
- Loading-indikator vises først etter 200 ms (`CombinedDiffUI.kt`).
- Når høyder endres holdes innhold stabilt ved å ankre på en blokk
  (`runPreservingViewportContent`, `:885-933`).
- Side-by-side-blokker bygges alltid med `ALIGNED_TWO_SIDED_DIFF=true`
  (`CombinedDiffComponentProcessorImpl.kt:217`). Begge sider er like høye,
  så ingen sync scroll trengs inne i en blokk.
- Navigasjon (`:314-362`): next innen blokk, ellers neste blokk med
  `goFirst`. Ytre `ScrollingModelImpl` oversetter editor-posisjon til ytre
  viewport (`:852-881`).
- Blokk-id er `CombinedPathBlockId(path, fileStatus)` (`CombinedDiffBlocks.kt:204`).
  Header er sticky og body kan kollapses (`:206-298`).

### Change-list tree

Filer i `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/`.

```
build(changes):
  sorter changes på path-lengde           // TreeModelBuilder.java:88, foreldre først
  for c: insertChangeNode(c)              // :376-398
    parent = grouping.getParentNode(c)    // policy-kjede, ChangesGroupingSupport.kt:72-80
  sorter hele treet (BROWSER_NODE_COMPARATOR: sortWeight, klasse, navn)  // :405-427
  collapseDirectories()                   // :442-497

DirectoryPolicy.getParentNode(path):      // DirectoryChangesGroupingPolicy.kt:24-55
  gå opp parent-stier til en node finnes i DIRECTORY_CACHE
  lag manglende mappe-noder top-down, legg i cache

collapseDirectories(node):
  mens mappe har nøyaktig ett barn og barnet er mappe: slå sammen til "a/b/c"
```

- Grouping-nøkler: directory, module, repository, none
  (`ChangesGroupingSupport.kt:92-95`).
- Sort weights (`ChangesBrowserNode.java:58-71`): module 4, directory 5,
  file path 6, change 7, unversioned 9, ignored 11. Mapper før filer.
- Mappe viser antall filer i grått (`getCountText`, `:272-291`, cachet
  `:200-216`). I flat modus vises parent-sti i grått etter filnavnet (`:348`).
- Filnavn farges etter `FileStatus` (`ChangesBrowserChangeNode.java:51-81`).

Statusfarger (`platform/platform-resources/src/themes/expUI/`):

| Status | Light | Dark |
|---|---|---|
| ADDED | `#067D17` | `#73BD79` |
| DELETED | `#6C707E` | `#6F737A` |
| MODIFIED / RENAMED | `#0033B3` | `#70AEFF` |
| UNKNOWN (unversioned) | `#B23247` | `#E88F89` |
| IGNORED | `#8C4F00` | `#D69A6B` |
| MERGED | `#643CB8` | `#CF84CF` |
| Konflikt | `#DE1B2E` | `#DE6A66` |

Light: `expUI_lightScheme.xml:51-67`. Dark: `expUI_darkScheme.xml:55-72`.
Statusliste: `platform/editor-ui-api/src/com/intellij/openapi/vcs/FileStatus.java:29-61`.

Tree -> diff preview (`platform/vcs-impl/src/.../TreeHandlerDiffRequestProcessor.kt:76-218`):
seleksjon og modellendringer debounces 100 ms (restart ved hver endring).
I combined-modus velger seleksjon bare en blokk (scroll), ingen ny request.

---

## 7. Hva gjør det smooth

- **Bakgrunn med kort synkron venting.** `DiffViewerBase.rediff`
  (`tools/util/base/DiffViewerBase.java:145-168`) kjører diff i bakgrunn
  og venter synkront opptil `DEFAULT_PROGRESS_DELAY_MILLIS` (ca. 200 ms) hvis
  vinduet har fokus. Rask diff gir ingen flimmer. Treg diff viser busy
  indicator (`onSlowRediff`). Resultatet er en Runnable som påføres på EDT
  i én operasjon.
- **Debounced rediff.** `scheduleRediff` (`:122-130`) via Alarm ved
  dokumentendring. Avbrytes når viewer skjules (`:76-89`).
- **Request-cache.** `impl/CacheDiffRequestProcessor.java:40-41`:
  `SoftHardCacheMap(5, 5)`, 5 hard og 5 soft referanser. Bytte tilbake til
  en nylig fil er gratis. Lasting av ny request venter også kort før
  "Loading..." vises (`:116-124`).
- **Bevar posisjon.** All apply skjer i `runPreservingScrollingPosition`
  (`SimpleDiffViewer.java:347-401`, `EditorScrollingPositionKeeper`). Fold-state
  bevares via `ExpandSuggester`. Caret via transfer.
- **Jitter-vakt og 1/3-anker** i sync scroll (se 1).
- **Animert makeVisible med interpolert slave** (se 1).
- **Combined:** placeholders med husket høyde, lazy load viewport ± 3,
  100 ms merging queue, ankret layout ved høydeendring.
- **Unified highlighting** lexer hele originalfiler, ikke unified-teksten.

---

## Anbefalinger for jetdiff (GPUI)

1. **Én scroll-modell per viewer.** Lagre `scroll_y` for master, beregn slave
   i samme frame fra `transfer_visual_line`. Ingen event-ping-pong, så
   re-entrancy-telleren trengs ikke.
2. **Start med aligned mode som default.** Fyll kortere side med tomme rader
   i layout (padding-rader i stedet for inlays). Da blir sync scroll
   `y_left == y_right`, dividers blir enkle, og samme kode kan brukes i
   combined view. Legg til ikke-aligned med 1/3-anker og onlyMajor-vakt senere.
3. **Visuell radmodell.** Bygg en `Vec<Row>` per side der hver rad er
   `Line(n) | Fold{range, level} | Pad`. Folding, padding og transfer blir
   indeksoppslag. Binærsøk på kumulativ høyde for y->rad.
4. **Folding:** tre nivåer 4/8/16 linjer kontekst, min 2 linjer, felles
   expand-state for begge sider, husk state per fil over rediff.
5. **Navigasjon:** plasser change 1/3 ned (`getTargetOffsets`-formelen),
   animer med lerp. Dobbelttrykk for neste fil.
6. **Commit-view:** én ytre scroll over blokker med kjente eller estimerte
   høyder. Beregn diff lazy for viewport ± 3, behold høyde ved unload.
   Sticky filheader.
7. **Tree:** sorter på path, mapper før filer, slå sammen enkeltbarn-mapper,
   farg filnavn etter status (tabellen over), grått antall.
8. **Bakgrunn:** diff og highlighting på bakgrunnstråd, vent synkront maks
   ~200 ms før spinner, påfør resultat atomisk. LRU-cache på ca. 5-10 filer.
