# 04 - Unified diff i IntelliJ

Kilde: `~/git/_ref/intellij-community`. Stier er relative til
`platform/diff-impl/src/com/intellij/diff/` hvis ikke annet står. Unified-koden
ligger i `tools/fragmented/`.

Begreper:
- **Unified-dokumentet** = den ene teksten unified-editoren viser. Bygges på nytt ved hver rediff.
- **Unified-linje** = linje i unified-dokumentet. **Sidelinje** = linje i venstre eller høyre fil.
- **Master** = siden like linjer hentes fra og som kan redigeres. Default `RIGHT`
  (`UnifiedDiffViewer.java:155`). Blir `LEFT` bare hvis bare venstre er skrivbar (`:176-179`),
  eller via `DiffUserDataKeys.MASTER_SIDE` (`processContextHints`, `:253-259`).
- **Change** = `UnifiedDiffChange`: `line1..line2` i unified-linjer, delt i `getDeletedRange()`
  og `getInsertedRange()` (`UnifiedDiffChange.java:29-42`).

---

## 1. Hvordan dokumentet bygges

Filer: `UnifiedFragmentBuilder.kt`, `UnifiedDiffViewer.computeDifferences` (`:417-432`).

Diffen er den samme `List<LineFragment>` som side-by-side bruker. Byggeren går gjennom
fragmentene i rekkefølge:

```
exec(fragments):                                  // SimpleUnifiedFragmentBuilder :160-171
  for f in fragments:
    processChanged(f)
  processEquals(lineCount1 - 1, lineCount2 - 1)   // finishDocuments :53-55

processChanged(f):                                // :67-100
  processEquals(f.start1 - 1, f.start2 - 1)       // like linjer før endringen
  blockStart = totalLines
  appendText(LEFT,  f.start1..f.end1)             // slettede først
  insertedStart = totalLines
  appendText(RIGHT, f.start2..f.end2)             // så nye
  return (blockStart, insertedStart, totalLines)

processEquals -> appendTextMaster(...)            // :109-121
  tekst kun fra master, men begge convertere får en rad
```

- Slettede linjer kommer alltid over nye i samme change. Ingen fletting linje for linje.
- Like linjer tas fra master (høyre). Venstre sin versjon av like linjer vises aldri.
- Hver bit legges til som `text.subSequence(start, lineEnd)` + `'\n'` (`appendText`, `:144-145`).
  Det betyr at dokumentet alltid slutter med en ekstra, falsk linje etter siste ekte linje.
  Viewer-koden vet det: `"there are 'fake' newline at the very end"` (`UnifiedDiffViewer.java:531`).
- Siste linje uten `'\n'` er ikke spesialbehandlet. `LineOffsets` gir en tom siste linje når teksten
  slutter med `'\n'`, akkurat som i side-by-side. Er bare den ene siden avsluttet med `'\n'`, blir
  det en innsatt eller slettet tom linje fra diffen selv.
- `HighlightRange(side, base, changed)` (`HighlightRange.java`, laget i `appendText` `:139-142`)
  husker hvor hver bit kom fra: offsets i unified-dokumentet og i originalteksten. Brukes til syntaks
  og markup (se 3). `newline`-justeringen på `:139` tar med linjeskiftet når originalen har et.

### Ignore whitespace og inner fragments

- Unified bruker `TwosideTextDiffProvider.NoIgnore` (`UnifiedDiffViewer.java:205`). Samme
  ignore-policies som side-by-side, men `DO_NOT_HIGHLIGHT` er fjernet
  (`tools/util/text/SmartTextDiffProvider.java:244`). Unified kan ikke skru av highlighting.
- Endringer som bare er whitespace og ignoreres, finnes ikke i `fragments`. De linjene blir like linjer
  og tas fra master. Whitespace-forskjellen på venstre side er da usynlig i unified.
- `innerFragments == null`: hel linje i full farge, ingen ord-markering.
- `innerFragments` tom liste: ignored-farge (lysere) på linjene og ingen ord-markering.
- `innerFragments` med innhold: ignored-farge og ord-markering.
- Regelen er `ignored = innerFragments != null` (`util/DiffDrawUtil.java:317`), samme som
  `SimpleDiffChangeUi` (`tools/simple/SimpleDiffChangeUi.java:90`).

---

## 2. Linjenummer og gutter

Fil: `LineNumberConvertor.java`.

- To convertere, én per side. `Builder.put(masterStart, slaveStart, masterLen, slaveLen)`
  (`:121-124`) fyller både `myFragments` (unified -> side) og `myInvertedFragments` (side -> unified).
  "Master" i convertoren er unified-dokumentet, ikke master-siden.
- `appendText` (`UnifiedFragmentBuilder.kt:123-133`):
  - like linjer: rad i begge convertere,
  - slettede: bare `convertor1`,
  - nye: bare `convertor2`.
- `convertRaw` (`:89-111`):
  - strict: -1 utenfor et segment, og -1 hvis segmentet har ulik lengde på sidene,
  - approximate: `floorEntry`, så `min(otherStart - start + value, otherStart + otherLength)`.
    En slettet linje mappes da til `start2` på høyre side, ikke linje for linje.
- Gutter (`UnifiedDiffViewer.java:503-509`): `setLineNumberConverter(primary, additional)` med
  strict convertere, slått sammen med en eventuell content-convertor og skjuling i folds
  (`DiffLineNumberConverter`, `util/DiffLineNumberConverter.java:39-48`).

| Linje | Venstre kolonne | Høyre kolonne |
|---|---|---|
| Lik | gammelt nummer | nytt nummer |
| Slettet | gammelt nummer | tom |
| Ny | tom | nytt nummer |
| Falsk siste linje | tom | tom |
| Inne i kollapset fold | tom | tom |

Layout (`platform/platform-impl/src/com/intellij/openapi/editor/impl/EditorGutterComponentImpl.java`):
- Én editor, ingen speiling. Scrollbar og error stripe til høyre som i en vanlig editor.
- `paintLineNumbers` (`:863-871`): gammel kolonne først, så `GAP_BETWEEN_AREAS` = 5px (`:265`),
  så ny kolonne. Begge høyrejustert i sitt område.
- `calcLineNumberAreaWidth` (`:2091-2102`): venstre kolonne er
  `max(getInitialLineNumberWidth, bredde av største nummer)`, høyre kolonne bare bredden av største
  nummer. Største nummer finnes ved å gå bakover til første linje med nummer
  (`DiffLineNumberConverter.getMaxLineNumber`, `:29-37`).
- Ellers samme gutter som side-by-side: diff-fargen dekker hele gutteren
  (`util/DiffLineMarkerRenderer.kt:33-79`).
- Titlene for begge sider stables over hverandre over editoren
  (`createTitles`, `UnifiedDiffViewer.java:269-277`).

---

## 3. Highlighting

### Bakgrunn og ord

`UnifiedDiffChangeUi.doInstallHighlighters` (`:74-81`) kaller
`DiffDrawUtil.createUnifiedChunkHighlighters` (`util/DiffDrawUtil.java:311-347`):

```
if deleted og inserted ikke tomme:
  LineHighlighter(deleted,  DELETED,  ignored)
  LineHighlighter(inserted, INSERTED, ignored)
else bare den som finnes

for frag in innerFragments (hvis ikke null):
  Inline(deletedStart  + frag.start1 .. frag.end1, DELETED)
  Inline(insertedStart + frag.start2 .. frag.end2, INSERTED)
```

- Unified bruker aldri `MODIFIED` (blå). En endret blokk er grå over grønn.
- Ord-markering har fast type per del: grå på slettede linjer, grønn på nye. Side-by-side bruker
  `DiffUtil.getDiffType(innerFragment)` per ord (`SimpleDiffChangeUi.java:101-114`), så der kan et
  ord bli blått.
- Ord-offsets er relative til linjestart for første slettede og første nye unified-linje.
- Tom side av et ord-fragment får vanlig `installEmptyRangeRenderer` (2px strek), i DELETED- eller
  INSERTED-farge.
- Ingen separator eller kantlinje mellom slettet og ny del. `PaintMode.DEFAULT` og `IGNORED` har
  `BorderType.NONE` (`DiffDrawUtil.java:778`), og ingen range er tom, så 2px-markøren for tomme
  sider brukes aldri. `createNonSquashedChangesSeparator` finnes bare i side-by-side.

### Syntaks

`UnifiedEditorHighlighter` (`:37-127`):
- Lexer hele venstre og hele høyre fil med vanlig `EditorHighlighter`.
- `init` går gjennom `HighlightRange` i rekkefølge, henter tokens fra riktig side og flytter dem til
  unified-offsets. Hull fylles med tomme elementer. Like naboelementer slås sammen (`addElement`).
- Slettede linjer får dermed riktig lexer-tilstand fra gammel fil, også inne i blokkommentarer.
- `UnifiedEditorRangeHighlighter.processRange` (`:49`) gjør det samme for markup fra
  originaldokumentene (inspeksjoner, breakpoints).

---

## 4. Folding

Fil: `UnifiedFoldingModel.java`, kalt fra `apply` (`UnifiedDiffViewer.java:482-484`).

- Samme `FoldingBuilderBase` som side-by-side, men med én side:
  `super(new int[]{lineCount}, ...)` (`:64`).
- Endringene er `LineRange(change.line1, change.line2)` i unified-linjer, altså hele blokken med
  slettet og ny del (`UnifiedFragmentBuilder.reportChange`, `:102-107`).
- Samme kontekst: `CONTEXT_RANGE` 4, nivåer 4/8/16, minst 2 linjer, første og siste gap foldes
  ut til kanten (se 03, avsnitt 3). Siden like linjer er 1:1, havner foldene på de samme like
  linjene som i side-by-side.
- `lineCount` er `countNewLines(text) + 1`, med den falske siste linja.
- Beskrivelsen i placeholderen hentes fra master-dokumentet via master-convertoren
  (`getDescription`, `:70-74`).
- Linjenummer skjules i begge kolonner for foldede linjer (`hideLineNumberPredicate(0)`).
- `CONTEXT_RANGE` og `EXPAND_BY_DEFAULT` er felles `TextDiffSettings` per sted, ikke per viewer.
- Åpne folds overlever ikke et bytte av viewer. `ExpandSuggester.isExpanded` faller tilbake til
  default når cachen har annet antall editorer (`tools/util/FoldingModelSupport.java:763-764`).
  Unified har 1, side-by-side 2.

---

## 5. Navigasjon, caret og bytte av viewer

### Neste og forrige endring

- `MyPrevNextDifferenceIterable` (`UnifiedDiffViewer.java:1062-1082`) bruker `line1` og `line2`.
  Samme regler som i side-by-side (`tools/util/PrevNextDifferenceIterableBase.java`), men på
  unified-linjer.
- Landing: `scrollEditor(line1, animated = true)` (`PrevNextDifferenceIterableBase.java:26`).
  Caret til første linje i blokken, altså toppen av slettet del. Så `ScrollType.CENTER`
  (`util/DiffUtil.java:504-529`). Ikke 1/3 som `getTargetOffsets` i side-by-side.
- Initial posisjon: `scrollToChange(FIRST_CHANGE)` (`:923-933`), samme landing uten animasjon.

### Current side

Unified har ingen fokus-side. Siden avledes per linje i `transferLineFromOneside` (`:606-638`):

```
lines = [convertor1.convert(line), convertor2.convert(line)]   // strict
side = master
if begge -1:   approximate på begge         // falsk siste linje
elif lines[0] == -1: lines[0] = approx; side = RIGHT   // ny linje
elif lines[1] == -1: lines[1] = approx; side = LEFT    // slettet linje
return (lines, side)
```

Siden brukes til hopp til kildefil (`getNavigatable`, `:1038-1048`, velger `pair.second`),
breadcrumbs og imaginær editor for intentions.

### Caret over rediff

`apply` lagrer caret som to-sidig posisjon før ny tekst settes (`:491-492`) og legger den tilbake
med `transferLineToOneside(side, line)` etterpå (`:537-539`). Kolonne beholdes.

### Bytte mellom unified og side-by-side

Et bytte lager en ny viewer på samme `DiffRequest`. Gammel viewer skriver hint til requesten i
`updateContextHints` ved dispose, ny leser dem i `processContextHints`
(`tools/util/base/InitialScrollPositionSupport.java:46-59`):

- `EDITORS_CARET_POSITION`: én `LogicalPosition` per side.
- `EditorsVisiblePositions`: caret og scroll-punkt per editor.

Unified til side-by-side (`MyInitialScrollHelper.getCaretPositions`, `:1248-1255`):
- Begge carets settes: den eksakte siden og den approximate andre siden. Står caret på en slettet
  linje, får venstre riktig linje og høyre `start2`. På en ny linje får venstre `end1`.
- Hvilken side som får fokus kommer fra `PREFERRED_FOCUS_SIDE`, som bare side-by-side skriver
  (`tools/util/FocusTrackerSupport.java:56-63`). Unified sier altså ikke om caret sto i slettet
  eller ny del. Default er `RIGHT`.

Side-by-side til unified (`doScrollToPosition`, `:1258-1274`):
- Bare master-sidens caret brukes: `myMasterSide.selectNotNull(myCaretPosition)`, så
  `transferLineToOneside(RIGHT, line)`. Sto brukeren i venstre editor, ignoreres det.

Scroll i begge retninger:
- `EditorsVisiblePositions` har feil lengde (1 mot 2 editorer), så `isSame` feiler
  (`InitialScrollPositionSupport.java:298-306`). Scroll-posisjonen gjenopprettes ikke. Ny viewer
  sentrerer på caret (`scrollToCaret`, `CENTER`).
- Rekkefølgen er `scrollToChange > linje > context > caret-posisjon > første endring`
  (`performDelayedSyncScroll`, `:145-154`). Ved bytte er de tre første nullet, så det er caret som
  bestemmer.
- Har brukeren bare scrollet med hjulet uten å flytte caret, hopper visningen til der caret står.

---

## 6. Error stripe

- Én stripe til høyre. Markørene kommer fra `LineHighlighterBuilder.done`
  (`DiffDrawUtil.java:604-637`), én per highlighter.
- En endret blokk gir to markører: DELETED-farge for slettet del og INSERTED-farge for ny del.
  Ingen blå markører.

---

## 7. Rollback og kopier plassering

- Gutter-ikon per change på `line1`, altså første linje i blokken (`createOperation`,
  `UnifiedDiffChangeUi.java:93-97`). Med høyre skrivbar vises `Revert` (`AllIcons.Diff.Revert`),
  som kopierer venstre side inn (`doInstallActionHighlighters`, `:60-72`).
- `replaceChange` (`UnifiedDiffViewer.java:852-870`) bruker `LineFragment`-linjene direkte. Hele
  changen rulles tilbake, uansett om caret står i slettet eller ny del.
- Tastaturvarianten (`ReplaceSelectedChangesAction`, `:817-832`) velger changes der caret eller
  markering treffer `line1..line2` (`getSelectedChanges`, `:889-897`).
- Etter apply kjøres rediff synkront (`handleMouseClick`, `UnifiedDiffChangeUi.java:130-145`).
- Ingen egen "kopier sti:linje". Hopp til kilde (F4) og tilsvarende bruker `getNavigatable`:
  slettet linje gir venstre fil, ny og lik linje gir høyre (se 5). Har venstre ingen navigatable
  (gammel revisjon), brukes høyre med approximate linje.

---

## 8. Nye og slettede filer

- `UnifiedDiffTool.createComponent` (`UnifiedDiffTool.java:16-20`) velger `SimpleOnesideDiffViewer`
  når én side er `EmptyContent` (`tools/util/side/OnesideDiffViewer.java:127-145`). Det gjør
  `SimpleDiffTool` også.
- Nye og slettede filer ser altså helt like ut i begge moduser. Velgeren vises likevel, siden begge
  verktøyene kan vise requesten.
- En tom fil som finnes er ikke `EmptyContent`. Den går til vanlig unified med alt som slettet
  eller nytt.

---

## 9. Bytte mellom viewere

- Velgeren er en `SegmentedButtonComponent` i diff-toolbaren (`impl/DiffRequestProcessor.java:241-250`,
  `createDiffToolChooser` `:915-929`). To ikonknapper, `AllIcons.Diff.SideBySide` og
  `AllIcons.Diff.Unified` (`FrameDiffTool.getIcon`), med tooltip "Side-by-side viewer" og
  "Unified viewer".
- Skjult hvis bare ett verktøy passer eller verktøyet er tvunget (`updateDiffToolChooser`, `:931-953`).
- Samme valg ligger i innstillings-popupen, Ctrl+Shift+D i default-keymap
  (`MyChangeDiffToolActionGroup`, `:956-985`, `Diff.ShowSettingsPopup`). Ingen egen snarvei for bytte.
- `switchToDiffTool` (`:379-389`) flytter valgt verktøy først i `myToolOrder`, lagrer rekkefølgen og
  laster requesten på nytt.
- Valget er persistent per sted, ikke globalt: `DiffSettingsHolder.PlaceSettings.DIFF_TOOLS_ORDER`
  i `diff.xml` (`impl/DiffSettingsHolder.kt:31-34`). Stedet er `DiffUserDataKeys.PLACE`
  (`DiffRequestProcessor.java:202`).
- Default per sted (`defaultPlaceSettings`, `DiffSettingsHolder.kt:94-106`): `ChangesView` og
  `VcsFileHistoryView` starter i unified, `VcsLogView` i side-by-side. Andre steder bruker
  registreringsrekkefølgen, der `SimpleDiffTool` kommer før `UnifiedDiffTool`
  (`DiffManagerImpl.kt:95-101`).
- Combined diff har sin egen Split/Unified-velger med samme mekanisme
  (`tools/combined/CombinedDiffMainUI.kt:211-235`).

---

## 10. Lett å overse

- Den falske siste linja. Den har ingen nummer, telles i `lineCount` for folding og gjør at
  `canGoNext` sin sjekk `line == lineCount - 1` treffer den, ikke siste ekte linje.
- En slettet linje mappes approximate til `start2`, ikke linje for linje som `BaseSyncScrollable`.
  Changes på linje 0 uten like linjer før gir -1, som `getPosition` klemmer til `(0,0)`
  (`UnifiedDiffViewer.java:1276-1279`).
- Strict convertering gir -1 når et likt segment har ulik lengde på sidene. Da blir slave-kolonnen
  tom. Skjer bare med ikke-fair line fragments (kommentar i `appendTextMaster`, `:110-111`).
- Like linjer vises fra master. Ignorert whitespace på venstre side blir borte.
- Fargene skiller seg fra side-by-side: grå over grønn i stedet for blå, også i ord og stripe.
- Fold-tilstand og scroll-posisjon tapes ved bytte. Caret overføres, men bare master-sidens fra
  side-by-side.
- Mye av koden finnes bare fordi unified kan redigeres: guarded blocks for slave-linjer
  (`:523-533`), `MyOnesideDocumentListener` (`:655-709`) og `LineNumberConvertor.Corrector`
  (`:196-306`). Det trengs ikke i en read-only viewer.

---

## Kobling til jetdiff

| IntelliJ | jetdiff i dag | Unified |
|---|---|---|
| `UnifiedFragmentBuilder` | `FileDiff` med blokker og tekst per side | Radtabell over `FileDiff`: rad = (side, linje). Tekst, syntaks og ord ligger allerede per side, så vi trenger ikke bygge ny tekst |
| `LineNumberConvertor` | `LineMapping` (sync scroll) | Per rad: `old: Option<usize>`, `new: Option<usize>`. Invers per side for side -> rad |
| `UnifiedEditorHighlighter` | `FileDiff::line_syntax(side, line)` | Gjenbrukes direkte, raden peker på sidens linje |
| `createUnifiedChunkHighlighters` | `Block::diff_type`, `InlineHighlight::diff_type` | Slettet del alltid `Deleted`, ny del alltid `Inserted`, også for ord. `Block::ignored()` som før |
| `UnifiedFoldingModel` | `Folding` med to sider og parrede regioner | Én side over rader, med blokkens hele radintervall som endring. `build_groups` kan brukes med én side |
| Gutter med to kolonner | `gutter_layout` med ett nummerområde, speilet venstre | Gammel kolonne, 5px gap, ny kolonne. Begge høyrejustert, ikke speilet |
| Error stripe | `stripe_markers` med én markør per blokk | To markører for endrede blokker |
| `transferLineFromOneside` | `Cursor { side, line }` | Siden kan avledes per rad: slettet = venstre, ny og lik = høyre |
| Ingen divider | `Divider`, `Connector` | Brukes ikke |
| `SimpleOnesideDiffViewer` | `FileDiff::single_side` | Samme visning i begge moduser |

Valg der IntelliJ ikke er et godt forbilde:
- Ved bytte kan vi bruke cursorens faktiske side, ikke bare master. Med én `Cursor` er det gratis.
- Vi kan holde cursorlinja på samme sted i viewporten ved bytte, som `DiffState::rescale` gjør ved
  zoom. IntelliJ sentrerer på caret.
- Folds kan beholdes ved bytte. Like linjer er 1:1, så en fold i side-by-side har en entydig fold i
  unified.
- Landing ved neste/forrige endring: IntelliJ bruker `CENTER` i unified og 1/3 i side-by-side. Samme
  1/3 i begge gjør byttet roligere.
- `y` (`DiffState::location`) virker uendret. En slettet rad har venstre side, og `location` overfører
  til høyre. IntelliJ gir `start2` her, vår `LineMapping::transfer` går linje for linje i blokken.
- `r` (rollback) virker uendret via `cursor_block`, siden hele blokken rulles tilbake i IntelliJ også.
- Toggle: IntelliJ lagrer valget per sted. Vi har bare ett sted, så én nøkkel i `config.toml` holder.

Valgt da unified ble laget:
- Ingen folding i unified, så folds beholdes ikke ved bytte. Split lager foldingen på nytt.
- Nye og slettede filer vises alltid unified, og `v` gjør ingenting der. Split viser derfor aldri én
  side alene.
