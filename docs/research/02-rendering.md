# 02 - Visuell rendering i IntelliJ side-by-side diff

Kilde: `~/git/_ref/intellij-community`. Stier under er relative til
`platform/diff-impl/src/com/intellij/diff/` hvis ikke annet står.
Alle pikselverdier er "unscaled" (1x). IntelliJ ganger med `JBUIScale.scale()`.

## 0. Oversikt og layout

```
[ L-scrollbar | L-tekst | L-gutter ] [ divider 24px ] [ R-gutter | R-tekst | R-scrollbar ]
```

- Venstre editor får `VERTICAL_SCROLLBAR_LEFT` (`tools/util/side/TwosideTextDiffViewer.java:112`).
  Da blir hele gutteren speilet (`EditorGutterComponentImpl.java:2188-2206`, `setMirrorTransformIfNeeded`
  kalles på hele gutteren i paint, linje 520). Resultat: venstre gutter ligger inntil divideren,
  scrollbar/error stripe ligger ytterst. Layouten er speilsymmetrisk om divideren.
- Linjenumre speiles to ganger (gutter + linjenummer-område, linje 912), så glyfene er normale,
  men i venstre editor er de venstrejustert i sitt område (`textOffset = offset - areaWidth - 1`, linje 975-977).
  Høyre editor: høyrejustert.
- Standardinnstillinger (`tools/util/base/TextDiffSettingsHolder.kt:38-60`):
  `HIGHLIGHT_POLICY = BY_WORD` (inner fragments på), `ENABLE_ALIGNING_CHANGES_MODE = false`,
  `CONTEXT_RANGE = 4` (linjer kontekst rundt kollapsede områder), `SHOW_LINE_NUMBERS = true`.
- Caret row er slått av i diff-editorer (`util/DiffUtil.java:363`).
- Det finnes ingen "valgt change"-utheving i SimpleDiffViewer. Ingen hover-effekt på changes.
  Eneste hover er på kollapsede separatorer (tykkere bølge, se 3).

## 1. Divideren

### 1.1 Maling-rekkefølge

`tools/simple/SimpleDiffViewer.java:904-918` (`MyDividerPainter`):

```
gg = getDividerGraphics(g, divider, editor1.component)   // y=0 == toppen av editor1-komponenten
gg.fill(clip, dividerColor)                              // bakgrunn
paintPolygons(gg, width, editor1, editor2)               // change-bånd (kurvet)
foldingModel.paintOnDivider(gg)                          // bølge-connectors for kollapsede områder
```

- `getDividerGraphics` (`util/DiffDividerDrawUtil.java:43-49`): klipper/translaterer slik at divideren
  deler y-akse med editor1-komponenten (titler over editorene regnes ikke med).
- `dividerColor` = gutter-bakgrunn (`util/DiffDrawUtil.java:95-110` -> `EditorGutterColor.java:16-28`):
  - Classic UI: `GUTTER_BACKGROUND` fra scheme (f0f0f0 / 313335).
  - New UI: `EDITOR_GUTTER_BACKGROUND` (null som standard) -> editor-bakgrunn (ffffff / 1e1f22).
  Divideren flyter altså visuelt sammen med gutterne.
- Både polygoner og separatorer males med `setupAAPainting`:
  `ANTIALIAS_ON` + `STROKE_NORMALIZE` (`platform/util/ui/.../GraphicsConfig.java:67-70`,
  brukt i `DiffDividerDrawUtil.java:58,81`). Ingen alpha, fargene er opake.

### 1.2 Bredde

`diff.divider.width=24` (`platform/util/resources/misc/registry.properties:1079`),
satt i `tools/util/DiffSplitter.java:47`. Kan bli bredere hvis top/bottom-action-knapper
(f.eks. swap) er bredere (linje 66-73). Bruk 24px.

### 1.3 Line -> y

`DiffDrawUtil.lineToY` (`util/DiffDrawUtil.java:240-260`):
- `lineStart=true`: `visualLineToY(visualLine(lineStartOffset))` (topp av linja).
- `lineStart=false`: topp + `lineHeight` (bunn, eksklusiv).
- `line >= lineCount`: y av dokument-slutt + `lineHeight * tailLines`.

Scroll: `getEditorTopOffset = -verticalScrollOffset + headerHeight` (`DiffDividerDrawUtil.java:135-139`).
Hver side bruker sin egen editor og sin egen scroll-offset.

`getGutterMarkerPaintRange(editor, start, end)` (`DiffDrawUtil.java:262-279`):

```
if start == end:                       // tom side (ren insert/delete)
    y1 = (start == 0) ? top(0) + 1 : bottom(start-1)   // = linjegrensen
    y2 = y1
else:
    y1 = top(start)
    y2 = bottom(end-1)                 // eksklusiv
```

Polygonet bygges i `createPolygon` (`DiffDividerDrawUtil.java:273-297`):
`start1 = range1.y1 + topOffset1`, `end1 = range1.y2 + topOffset1`, tilsvarende for side 2.

### 1.4 Tomme sider og pikseljustering

`DividerPolygon.paint` (`DiffDividerDrawUtil.java:462-494`):

```
if end1 - start1 < 2:  sY1 = start1 - 1; eY1 = start1      // tom: 2px bånd rundt grensen
else:                  sY1 = start1;     eY1 = end1 - 1
(samme for side 2)
drawCurveTrapezium(g, 0, width, sY1, eY1, sY2, eY2, fill, border)
```

`drawCurveTrapezium` fyller med `eY + 1` som bunn (`DiffDrawUtil.java:159`). Effektivt dekker hver side:
- ikke-tom: `[top(start), bottom(end-1))` - nøyaktig linjebåndet.
- tom: `[grense-1, grense+1)` - 2px, sentrert på linjegrensen. For linje 0: `[0, 2)`.

Dette matcher 2px-linja inne i editoren (se 2.3), så båndet flyter sømløst over.

### 1.5 Kurven (kubisk Bezier)

`DiffDrawUtil.java:76` `CTRL_PROXIMITY_X = 0.3`, `makeCurve` linje 212-226, `makeCurvePath` 203-210:

```
fn curve(x1, x2, yL, yR) -> Cubic:
    w = x2 - x1
    P0 = (x1,          yL)
    P1 = (x1 + 0.3*w,  yL)       // horisontal tangent ut fra venstre
    P2 = (x1 + 0.7*w,  yR)       // horisontal tangent inn til høyre
    P3 = (x2,          yR)

fn band_path(x1, x2, top1, top2, bot1, bot2) -> Path:
    p = Path()
    p.move_to(x1, top1)
    p.cubic_to((x1+0.3w, top1), (x1+0.7w, top2), (x2, top2))     // øvre kant L->R
    p.line_to(x2, bot2)
    p.cubic_to((x1+0.7w, bot2), (x1+0.3w, bot1), (x1, bot1))     // nedre kant R->L
    p.close()

fn draw_curve_trapezium(sY1, eY1, sY2, eY2, fill, border):
    if fill:
        fill_aa(band_path(0, W, sY1, sY2, eY1 + 1, eY2 + 1), fill)
        // sikrer min 1px tykkelse ved bratte kurver (DiffDrawUtil.java:161-165)
        stroke_aa(curve(0, W, (sY1+eY1)/2, (sY2+eY2)/2), width = 1px, fill)   // int-divisjon
    if border:                                                  // kun excluded/resolved
        draw_curve_line(sY1 -> sY2); draw_curve_line(eY1 -> eY2)
```

`drawCurveLine` (linje 184-192): hvis kurven er flat (`|dx| >= |dy|`) og stroke er tynn på HiDPI,
fylles den som et 1px bånd `band_path(y, y+1)` i stedet for stroke, for å matche horisontale
linjer i editoren. Ellers vanlig `draw`.

Ikke-kurvet variant (`drawTrapezium`, linje 130-149) er en rett firkant
`(x1,start1) (x2,start2) (x2,end2+1) (x1,end1+1)`. SimpleDiffViewer bruker alltid `curved=true`
(`DiffDividerDrawUtil.java:70`).

### 1.6 Farger på divideren

`SimpleDiffChangeUi.drawDivider` (`tools/simple/SimpleDiffChangeUi.java:145-159`) -> `processExcludable`
-> `ExcludablePainter` (`DiffDividerDrawUtil.java:377-414`):
- fill = `type.getColor()` (full farge, også når editoren bruker lysere "ignored"-farge).
- border = null for vanlige changes.
- Excluded: fill = ignored-farge, border = full farge.
- Hvis begge sider er foldet og type er INSERTED/DELETED, brukes MODIFIED (linje 299-302).

### 1.7 Synlighetsfilter

`DividerPaintableHandlerImpl.process` (linje 259-271): changes itereres i rekkefølge,
hoppes over hvis begge sider er over synlig område, stopper når begge er under.

## 2. Highlights inne i editoren

### 2.1 Lag (z-orden)

`DiffDrawUtil.java:61-69`, relativt til `HighlighterLayer.SELECTION`:
```
DEFAULT_LAYER     = SELECTION - 102   // linje-bakgrunn
INLINE_LAYER      = SELECTION - 101   // ord-bakgrunn (over linja)
LINE_MARKER_LAYER = SELECTION - 100   // separator-linjer
STRIPE_LAYER      = ERROR - 1         // error stripe
```
Seleksjon males over alt diff-relatert.

### 2.2 Linjebakgrunn

`LineHighlighterBuilder.done` (`DiffDrawUtil.java:570-642`) + `SimpleDiffChangeUi.createHighlighter` (linje 84-99):
- `HighlighterTargetArea.LINES_IN_RANGE`: hele linjebredden til høyre kant av viewport.
- `ignored = innerFragments != null` (`SimpleDiffChangeUi.java:90`). Med BY_WORD er det nesten alltid sant
  for MODIFIED. Da er editor-modus `IGNORED`: linjebakgrunn = `getIgnoredColor` (lys),
  mens ordene får full farge.
- PaintMode (`DiffDrawUtil.java:777-782`):
  ```
  DEFAULT         = bg DEFAULT, border NONE
  IGNORED         = bg IGNORED, border NONE
  RESOLVED        = bg NONE,    border DOTTED
  EXCLUDED_EDITOR = bg NONE,    border LINE
  EXCLUDED_GUTTER = bg IGNORED, border LINE
  ```
  Vanlige changes har altså ingen topp/bunn-kantlinjer. Kun excluded/skipped/resolved får kant.

### 2.3 Tom side (ren insert/delete)

To biter tegner en 2px horisontal linje i type-fargen (full farge, ikke mørkere):
- Tekstområdet: `LineMarkerBuilder` på linje `start-1` med `BOTTOM` (eller linje 0 `TOP`),
  `doubleLine=true` (`DiffDrawUtil.java:619-625`). `DiffDefaultBorderRenderer.drawLine`
  (linje 948-954) tegner fra x1 til høyre kant av clip, `TOP` flytter y en ned.
- Gutter: `DiffLineMarkerRenderer.drawMarker` (`util/DiffLineMarkerRenderer.kt:97-112`):
  `y2 - y1 <= 2` -> `drawChunkBorderLine(y1 - 1, doubleLine=true)`.
- `drawChunkBorderLine` (`DiffDrawUtil.java:112-128`): double = linjer på `y` og `y+1`.

Pseudo:
```
boundary = (start == 0) ? top(0) + 1 : bottom(start - 1)
fill_rect(x0, boundary - 1, full_width, 2, type.color)     // gutter + tekst
```

### 2.4 Inner (ord) highlights

`SimpleDiffChangeUi.createInlineHighlighter` (linje 101-114) + `InlineHighlighterBuilder` (`DiffDrawUtil.java:645-679`):
- Type per fragment: `DiffUtil.getDiffType(fragment)` (`util/DiffUtil.java:1555-1581`):
  begge sider ikke-tomme = MODIFIED, kun venstre = DELETED, kun høyre = INSERTED.
  Så i en blå MODIFIED-blokk kan et ord være grønt (INSERTED) på høyre side.
- Farge: `type.getColor()` (full farge). Ikke mørkere-derivert. Kontrasten kommer av at linja
  rundt er blandet mot bakgrunn (2.5).
- `EXACT_RANGE`: kun tegnene, ingen avrunding/padding.
- Tom side av et ord-fragment: `DiffEmptyHighlighterRenderer` (`util/DiffEmptyHighlighterRenderer.java:42`):
  ```
  fill_rect(x_of_offset - 1, line_top, 2, lineHeight, type.color)   // 2px vertikal strek
  ```

### 2.5 "Ignored"-farge (avledet)

`TextDiffTypeFactory.getIgnoredColor` (`util/TextDiffTypeFactory.java:65-75`, faktor linje 91):
```
if scheme har FOREGROUND for nøkkelen: bruk den
else: ignored = mix(color, editorBg, 0.6)
mix(a, b, t) per kanal = a + round(t * (b - a))       // MixedColorProducer.mix, Java Math.round
```
Dvs. 40% diff-farge, 60% editor-bakgrunn. Ingen av standard-schemaene setter FOREGROUND, så
den er alltid avledet.

### 2.6 Gutter vs tekst ved ulike modi

`DiffLineMarkerRenderer.paint` (linje 33-80): hvis `editorMode != gutterMode` deles gutteren ved
`whitespaceSeparatorOffset`:
- `[0, xOutline)` males med gutterMode.
- `[xOutline, gutterWidth)` males med editorMode.

Med BY_WORD er editorMode=IGNORED og gutterMode=DEFAULT. Resultat: linjenummer-området har
full (mettet) farge, mens tekstområdet og en smal stripe ytterst i gutteren har den lyse
ignored-fargen. New UI: `whitespaceSeparatorOffset = gutterWidth - 3` (`EditorGutterComponentImpl.java:1804-1810`).

### 2.7 Andre varianter

- Nabo-changes uten mellomrom: 1px MODIFIED-linje på toppen av den andre (`SimpleDiffChangeUi.java:116-130`,
  `DiffDrawUtil.createLineMarker` linje 394-401).
- Resolved/excluded border: 1px linje (eller "bold dotted") på første og siste piksel-rad
  (`DiffLineMarkerRenderer.kt:103-106`). `BOLD_DOTTED_STROKE` = 2.3px, dash [2,2] (`DiffDividerDrawUtil.java:37-38`).
- Whitespace-only endring med BY_WORD: inner fragments er tom liste -> linja er kun lys ignored-farge.

## 3. Separatorer for kollapsede områder

Folding: `tools/util/FoldingModelSupport.java`. Kollapset område blir én visuell linje med
placeholder `"     "` (5 mellomrom, linje 85, 480). `FOLDED_TEXT_ATTRIBUTES` settes til null (`DiffUtil.java:368-371`),
så ingen fold-boks. Separator-renderer settes med `SeparatorPlacement.TOP` (`DiffDrawUtil.java:420-433`).

### 3.1 Parametre (`util/DiffLineSeparatorRenderer.java:326-367`)

```
step     = max(dividerWidth / 6, 2) = 4px          // kvart-periode
height   = 3px                                     // halv amplitude
aaGap    = 1px
period   = 4 * step = 16px
vOffset  = (lineHeight - 2*height - 2*aaGap) / 2   // sentrer i linja
color    = DIFF_SEPARATOR_WAVE ?: DIFF_SEPARATORS_BACKGROUND ?: #808080
stroke   = 1px (2px når hovered), AA
```

### 3.2 Bølge-tile (16 x 8 px, `createImage` linje 274-310)

```
upper = 1, center = 4, lower = 7, dx = step/2 = 2
move_to(0, upper)
quad_to((2, upper),  (4, center))
quad_to((6, lower),  (8, lower))
quad_to((10, lower), (12, center))
quad_to((14, upper), (16, upper))
```
Tile tegnes hvert 16. px (`draw`, linje 240-271), y = `lineTop + vOffset`.

### 3.3 Fase

- Gutter: `shiftX = step` (linje 148, 343-345). Venstre gutter er speilet.
- Tekst: `shiftX = -period - (gutterWidth % period) - period + step + hScroll` (linje 184-195),
  dvs. kontinuerlig fra gutter inn i tekst, forankret til viewport (flytter seg ikke ved h-scroll).
- Forenklet for smudiff: la `d` = avstand fra dividerkanten (speilet på venstre side).
  `y(d) = tile((d + 12) mod 16)`. Ved dividerkanten er bølgen i center og stiger utover.

### 3.4 Connector over divideren (`drawConnectorLine`, linje 60-126)

`y1/y2` = topp av den foldede linja i hver editor (`DiffDividerDrawUtil.java:155-163`),
pluss `vOffset + height + aaGap` (= bølgens senterlinje).

```
path.move_to(x1 - 0.5, y1)
slope = |y2 - y1| / |x2 - x1|
if slope < 0.2:
    mx, my = mid(x1,x2), mid(y1,y2)
    if x2 - x1 > 5*step:                           // 3 halvperioder
        quad((x1+0.5s, y1+h), (x1+s,   y1+h))
        quad((x1+1.5s, y1+h), (x1+2s,  my))
        quad((x1+2.5s, my-h), (mx,     my-h))
        quad((x2-2.5s, my-h), (x2-2s,  my))
        quad((x2-1.5s, y2+h), (x2-s,   y2+h))
        quad((x2-0.5s, y2+h), (x2,     y2))
    else:
        quad((mx, my + 2h), (x2, y2))
elif y1 > y2:
    cubic((x1+s/8, y1+h/8), (x1+s/8, y1+h/2), (x1+s/2, y1+h/2))
    cubic((x2-2s,  y1+h/2), (x2-2s,  y2+4h),  (x2,     y2))
else:
    cubic((x1+2s,  y1+4h),  (x1+2s,  y2+h/2), (x2-s/2, y2+h/2))
    cubic((x2-s/8, y2+h/2), (x2-s/8, y2+h/8), (x2,     y2))
stroke(path, waveColor, 1px eller 2px hovered)
```

### 3.5 Beskrivelse

Valgfri tekst (f.eks. omsluttende funksjon) tegnes som inlay-hint ved `x1 + 5px`, og bølgen
klippes rundt den (`drawWithDescription`, linje 207-233). Kan droppes i første versjon.

## 4. Gutter

Layout venstre -> høyre (`EditorGutterComponentImpl.java:221-247`), `GAP_BETWEEN_AREAS = 5px` (linje 265):
```
gap | linjenumre | gap | (ekstra linjenumre) | gap | annotations | gap | line markers (ikoner, free painters) | folding
```
- Linjenumre: farge `LINE_NUMBERS_COLOR`, font = editorfont + `editor.gutter.linenumber.font.size.delta`
  (linje 886-891). y-baseline = `lineY + ascent`.
- Diff-markøren males over hele gutterbredden `x = 0..gutterWidth` (unntatt annotations-området),
  `DiffLineMarkerRenderer.kt:38-79`. Linjenumrene står altså på farget bakgrunn.
  AA er av i gutteren på ikke-HiDPI (linje 566-567).
- New UI: 1px vertikal `INDENT_GUIDE`-linje ved `whitespaceSeparatorOffset` (linje 584-592), males
  før diff-markørene, så den skjules i changes.
- Rekkefølge i gutter-paint (linje 559-600): bakgrunn -> editor-bakgrunner -> annotations ->
  separator-linje -> line markers (diff) -> folding -> linjenumre.
- Chevrons (>> / <<): `DiffGutterOperation` legger et `GutterIconRenderer` (Alignment.LEFT) på
  startlinja til changen (`util/DiffGutterOperation.java:18-29`, `SimpleDiffChangeUi.java:76-81, 173-187`).
  Vises kun hvis motsatt side er editerbar. Read-only: ingen ikoner. Checkbokser (partial commit)
  kommer fra andre viewere, ikke relevant nå.

## 5. Scrollbar / error stripe

`platform/platform-impl/.../EditorMarkupModelImpl.kt`:
- Diff lager én stripe-highlighter per change med `ERROR_STRIPE_COLOR` fra nøkkelen
  (`DiffDrawUtil.java:635-639`, `DiffStripeMarkerTextAttributes` linje 870-884).
  Tomme sider får stripe via `LineMarkerBuilder` på én offset (linje 744-749).
- Wide mark: `x = minMarkHeight + 2`, bredde `SCROLLBAR_WIDTH = 14px` (linje 407, 1218-1237).
- y-mapping (linje 1471-1503): `y = scrollbarTop + editorY / editorPreferredHeight * trackHeight`
  (1:1 hvis dokumentet er kortere enn tracken). Slutt-y er toppen av siste linje i rangen, og
  høyde < `minMarkHeight` utvides til `minMarkHeight` (maks 4px, linje 1131, 1423-1425).
- Overlapp løses per layer, høyeste layer vises (linje 1113-1192).
- Venstre editor har scrollbaren på venstre side (speilet).

## 6. Fargetabell

Scheme-arv: New UI Light = "Light" (`themes/expUI/expUI_lightScheme.xml`, parent Default).
New UI Dark = "Dark" (`themes/expUI/expUI_darkScheme.xml`, parent Darcula). Dark og Islands Dark
overstyrer ingen `DIFF_*`, så de arver Darcula-verdiene. Kilder: `platform-resources/src/DefaultColorSchemesManager.xml`
Default 489-523, Darcula 2255-2277. New UI Light MODIFIED: `expUI_lightScheme.xml:260-265`.

`bg` = `BACKGROUND` (linje, divider-fyll, ord). `stripe` = `ERROR_STRIPE_COLOR`.
`ignored` = `mix(bg, editorBg, 0.6)`, beregnet.

### Lys

| Type | Classic Default bg | stripe | ignored (på ffffff) | New UI Light bg | stripe | ignored |
|---|---|---|---|---|---|---|
| INSERTED | `#BEE6BE` | `#AADEAA` | `#E5F5E5` | `#BEE6BE` | `#AADEAA` | `#E5F5E5` |
| DELETED  | `#D6D6D6` | `#C8C8C8` | `#EFEFEF` | `#D6D6D6` | `#C8C8C8` | `#EFEFEF` |
| MODIFIED | `#CAD9FA` | `#B8CBF5` | `#EAF0FD` | `#C2D8F2` | `#B6D2F2` | `#E7EFFA` |
| CONFLICT | `#FFD5CC` | `#FFC8BD` | `#FFEEEB` | `#FFD5CC` | `#FFC8BD` | `#FFEEEB` |

### Mørk (samme diff-farger, ulik editor-bakgrunn gir ulik ignored)

| Type | bg | stripe | ignored Darcula (2b2b2b) | ignored New UI Dark (1e1f22) | ignored Islands Dark (191a1c) |
|---|---|---|---|---|---|
| INSERTED | `#294436` | `#447152` | `#2A352F` | `#222E2A` | `#1F2B26` |
| DELETED  | `#484A4A` | `#656E76` | `#373737` | `#2F3032` | `#2C2D2E` |
| MODIFIED | `#385570` | `#43698D` | `#303C47` | `#283541` | `#25323E` |
| CONFLICT | `#45302B` | `#8F5247` | `#352D2B` | `#2E2626` | `#2B2322` |

### Omgivende farger

| Nøkkel | Classic Light | New UI Light | Darcula | New UI Dark |
|---|---|---|---|---|
| Editor bg (`TEXT`) | `#FFFFFF` | `#FFFFFF` | `#2B2B2B` | `#1E1F22` (Islands `#191A1C`) |
| Gutter / divider bg | `#F0F0F0` | = editor bg | `#313335` | = editor bg |
| `LINE_NUMBERS_COLOR` | `#999999` | `#AEB3C2` | `#606366` | `#4B5059` |
| `LINE_NUMBER_ON_CARET_ROW_COLOR` | - | `#767A8A` | `#A4A3A3` | `#A1A3AB` |
| Bølge (`DIFF_SEPARATOR_WAVE`) | `#D0D0D0` | `#D0D0D0` (arvet) | `#555555` | `#555555` (arvet) |

Merk: `DIFF_SEPARATORS_BACKGROUND` (e4e6eb / 2b2d30) er satt i New UI, men `DIFF_SEPARATOR_WAVE`
har prioritet (`DiffLineSeparatorRenderer.java:351-358`) og arves fra parent. Fargeblind-varianter
(`deuteranopia`/`protanopia`) finnes i XML, ignoreres her.

Hvor fargen brukes:
```
editor linje-bg      = inner ? ignored : bg
gutter linje-bg      = bg                            (DEFAULT gutterMode)
ytterste 3px gutter  = inner ? ignored : bg          (New UI)
ord-bg               = bg for ordets egen type
tom side (2px linje) = bg
divider-fyll         = bg
error stripe         = stripe
```

## 7. Polish og justering

- Divider og editor deler y-akse via `getDividerGraphics`. Header-komponent i editoren
  (breadcrumbs o.l.) legges til i offset.
- Horisontale kanter av båndene ligger på heltalls-y, så de er skarpe selv med AA. Kun kurvene blir AA-myke.
- Ekstra 1px stroke langs midtkurven hindrer at bratte bånd forsvinner (`DiffDrawUtil.java:161-165`).
- Tom side: 2px bånd `[grense-1, grense+1)` både i editor, gutter og divider, så linja flyter
  gjennom uten hakk. Linje 0 er spesialtilfelle med `+1` så båndet blir `[0,2)`.
- Divideren fylles først med gutter-bg, så polygoner, så bølge-connectors på toppen.
- `STROKE_NORMALIZE` gjør at 1px strokes på heltalls-y dekker nøyaktig én pikselrad.
- Alignment-modus (av som standard) bruker `withAlignedHeight` (`DiffDividerDrawUtil.java:510-538`)
  og tegner ingen 2px-linjer. Ikke relevant før vi støtter inlays for justering.

## 8. Oppskrift for smudiff (per frame)

```
for side in [L, R]:
    paint editor bg
    for change in visible(side):
        if change.empty(side): fill(0..fullW, boundary-1, 2, color)
        else:
            fill(gutter 0..xOutline, top..bottom, color)
            fill(xOutline..fullW,    top..bottom, inner ? ignored : color)
            for word in change.inner: fill(word_rect, word.type.color)
                if word.empty(side): fill(x-1, lineTop, 2, lineH, word.type.color)
    paint fold separators (bølge i collapsed linje)
    paint text, line numbers
    paint error stripe
divider:
    fill(dividerBg)
    for change: fill_aa(band_path(...), color); stroke_aa(mid_curve, 1px, color)
    for fold pair: stroke_aa(connector(...), waveColor, 1px)
```
