# 02 - Visual rendering in IntelliJ's side-by-side diff

Source: `~/git/_ref/intellij-community`. Paths below are relative to
`platform/diff-impl/src/com/intellij/diff/` unless stated otherwise.
All pixel values are "unscaled" (1x). IntelliJ multiplies by `JBUIScale.scale()`.

## 0. Overview and layout

```
[ L-scrollbar | L-text | L-gutter ] [ divider 24px ] [ R-gutter | R-text | R-scrollbar ]
```

- The left editor gets `VERTICAL_SCROLLBAR_LEFT` (`tools/util/side/TwosideTextDiffViewer.java:112`).
  Then the whole gutter is mirrored (`EditorGutterComponentImpl.java:2188-2206`; `setMirrorTransformIfNeeded`
  is called on the whole gutter in paint, line 520). Result: the left gutter sits right against the divider,
  and the scrollbar/error stripe sits on the outside. The layout is mirror-symmetric around the divider.
- Line numbers are mirrored twice (gutter + line number area, line 912), so the glyphs read normally,
  but in the left editor they are left-aligned in their area (`textOffset = offset - areaWidth - 1`, lines 975-977).
  Right editor: right-aligned.
- Default settings (`tools/util/base/TextDiffSettingsHolder.kt:38-60`):
  `HIGHLIGHT_POLICY = BY_WORD` (inner fragments on), `ENABLE_ALIGNING_CHANGES_MODE = false`,
  `CONTEXT_RANGE = 4` (lines of context around collapsed areas), `SHOW_LINE_NUMBERS = true`.
- The caret row is turned off in diff editors (`util/DiffUtil.java:363`).
- There is no "selected change" highlight in SimpleDiffViewer. No hover effect on changes.
  The only hover is on collapsed separators (a thicker wave, see 3).

## 1. The divider

### 1.1 Paint order

`tools/simple/SimpleDiffViewer.java:904-918` (`MyDividerPainter`):

```
gg = getDividerGraphics(g, divider, editor1.component)   // y=0 == the top of the editor1 component
gg.fill(clip, dividerColor)                              // background
paintPolygons(gg, width, editor1, editor2)               // change bands (curved)
foldingModel.paintOnDivider(gg)                          // wave connectors for collapsed areas
```

- `getDividerGraphics` (`util/DiffDividerDrawUtil.java:43-49`): clips/translates so the divider
  shares the y axis with the editor1 component (titles above the editors are not counted).
- `dividerColor` = the gutter background (`util/DiffDrawUtil.java:95-110` -> `EditorGutterColor.java:16-28`):
  - Classic UI: `GUTTER_BACKGROUND` from the scheme (f0f0f0 / 313335).
  - New UI: `EDITOR_GUTTER_BACKGROUND` (null by default) -> the editor background (ffffff / 1e1f22).
  So the divider visually blends into the gutters.
- Both polygons and separators are painted with `setupAAPainting`:
  `ANTIALIAS_ON` + `STROKE_NORMALIZE` (`platform/util/ui/.../GraphicsConfig.java:67-70`,
  used in `DiffDividerDrawUtil.java:58,81`). No alpha; the colors are opaque.

### 1.2 Width

`diff.divider.width=24` (`platform/util/resources/misc/registry.properties:1079`),
set in `tools/util/DiffSplitter.java:47`. It can get wider if the top/bottom action buttons
(e.g. swap) are wider (lines 66-73). Use 24px.

### 1.3 Line -> y

`DiffDrawUtil.lineToY` (`util/DiffDrawUtil.java:240-260`):
- `lineStart=true`: `visualLineToY(visualLine(lineStartOffset))` (the top of the line).
- `lineStart=false`: top + `lineHeight` (the bottom, exclusive).
- `line >= lineCount`: y of the document end + `lineHeight * tailLines`.

Scroll: `getEditorTopOffset = -verticalScrollOffset + headerHeight` (`DiffDividerDrawUtil.java:135-139`).
Each side uses its own editor and its own scroll offset.

`getGutterMarkerPaintRange(editor, start, end)` (`DiffDrawUtil.java:262-279`):

```
if start == end:                       // empty side (pure insert/delete)
    y1 = (start == 0) ? top(0) + 1 : bottom(start-1)   // = the line boundary
    y2 = y1
else:
    y1 = top(start)
    y2 = bottom(end-1)                 // exclusive
```

The polygon is built in `createPolygon` (`DiffDividerDrawUtil.java:273-297`):
`start1 = range1.y1 + topOffset1`, `end1 = range1.y2 + topOffset1`, and the same for side 2.

### 1.4 Empty sides and pixel alignment

`DividerPolygon.paint` (`DiffDividerDrawUtil.java:462-494`):

```
if end1 - start1 < 2:  sY1 = start1 - 1; eY1 = start1      // empty: a 2px band around the boundary
else:                  sY1 = start1;     eY1 = end1 - 1
(the same for side 2)
drawCurveTrapezium(g, 0, width, sY1, eY1, sY2, eY2, fill, border)
```

`drawCurveTrapezium` fills with `eY + 1` as the bottom (`DiffDrawUtil.java:159`). In effect each side covers:
- not empty: `[top(start), bottom(end-1))` - exactly the line band.
- empty: `[boundary-1, boundary+1)` - 2px, centered on the line boundary. For line 0: `[0, 2)`.

This matches the 2px line inside the editor (see 2.3), so the band flows across seamlessly.

### 1.5 The curve (cubic Bezier)

`DiffDrawUtil.java:76` `CTRL_PROXIMITY_X = 0.3`, `makeCurve` lines 212-226, `makeCurvePath` 203-210:

```
fn curve(x1, x2, yL, yR) -> Cubic:
    w = x2 - x1
    P0 = (x1,          yL)
    P1 = (x1 + 0.3*w,  yL)       // horizontal tangent out of the left
    P2 = (x1 + 0.7*w,  yR)       // horizontal tangent into the right
    P3 = (x2,          yR)

fn band_path(x1, x2, top1, top2, bot1, bot2) -> Path:
    p = Path()
    p.move_to(x1, top1)
    p.cubic_to((x1+0.3w, top1), (x1+0.7w, top2), (x2, top2))     // upper edge L->R
    p.line_to(x2, bot2)
    p.cubic_to((x1+0.7w, bot2), (x1+0.3w, bot1), (x1, bot1))     // lower edge R->L
    p.close()

fn draw_curve_trapezium(sY1, eY1, sY2, eY2, fill, border):
    if fill:
        fill_aa(band_path(0, W, sY1, sY2, eY1 + 1, eY2 + 1), fill)
        // ensures at least 1px thickness on steep curves (DiffDrawUtil.java:161-165)
        stroke_aa(curve(0, W, (sY1+eY1)/2, (sY2+eY2)/2), width = 1px, fill)   // int division
    if border:                                                  // excluded/resolved only
        draw_curve_line(sY1 -> sY2); draw_curve_line(eY1 -> eY2)
```

`drawCurveLine` (lines 184-192): if the curve is flat (`|dx| >= |dy|`) and the stroke is thin on HiDPI,
it is filled as a 1px band `band_path(y, y+1)` instead of stroked, to match horizontal
lines in the editor. Otherwise a normal `draw`.

The uncurved variant (`drawTrapezium`, lines 130-149) is a straight quadrilateral
`(x1,start1) (x2,start2) (x2,end2+1) (x1,end1+1)`. SimpleDiffViewer always uses `curved=true`
(`DiffDividerDrawUtil.java:70`).

### 1.6 Colors on the divider

`SimpleDiffChangeUi.drawDivider` (`tools/simple/SimpleDiffChangeUi.java:145-159`) -> `processExcludable`
-> `ExcludablePainter` (`DiffDividerDrawUtil.java:377-414`):
- fill = `type.getColor()` (full color, also when the editor uses the lighter "ignored" color).
- border = null for ordinary changes.
- Excluded: fill = the ignored color, border = full color.
- If both sides are folded and the type is INSERTED/DELETED, MODIFIED is used (lines 299-302).

### 1.7 Visibility filter

`DividerPaintableHandlerImpl.process` (lines 259-271): changes are iterated in order,
skipped if both sides are above the visible area, and it stops when both are below.

## 2. Highlights inside the editor

### 2.1 Layers (z-order)

`DiffDrawUtil.java:61-69`, relative to `HighlighterLayer.SELECTION`:
```
DEFAULT_LAYER     = SELECTION - 102   // line background
INLINE_LAYER      = SELECTION - 101   // word background (above the line)
LINE_MARKER_LAYER = SELECTION - 100   // separator lines
STRIPE_LAYER      = ERROR - 1         // error stripe
```
The selection is painted above everything diff-related.

### 2.2 Line background

`LineHighlighterBuilder.done` (`DiffDrawUtil.java:570-642`) + `SimpleDiffChangeUi.createHighlighter` (lines 84-99):
- `HighlighterTargetArea.LINES_IN_RANGE`: the whole line width, to the right edge of the viewport.
- `ignored = innerFragments != null` (`SimpleDiffChangeUi.java:90`). With BY_WORD that is almost always true
  for MODIFIED. Then the editor mode is `IGNORED`: line background = `getIgnoredColor` (light),
  while the words get full color.
- PaintMode (`DiffDrawUtil.java:777-782`):
  ```
  DEFAULT         = bg DEFAULT, border NONE
  IGNORED         = bg IGNORED, border NONE
  RESOLVED        = bg NONE,    border DOTTED
  EXCLUDED_EDITOR = bg NONE,    border LINE
  EXCLUDED_GUTTER = bg IGNORED, border LINE
  ```
  So ordinary changes have no top/bottom border lines. Only excluded/skipped/resolved get a border.

### 2.3 Empty side (pure insert/delete)

Two pieces draw a 2px horizontal line in the type color (full color, not darker):
- The text area: `LineMarkerBuilder` on line `start-1` with `BOTTOM` (or line 0 `TOP`),
  `doubleLine=true` (`DiffDrawUtil.java:619-625`). `DiffDefaultBorderRenderer.drawLine`
  (lines 948-954) draws from x1 to the right edge of the clip; `TOP` moves y down one.
- Gutter: `DiffLineMarkerRenderer.drawMarker` (`util/DiffLineMarkerRenderer.kt:97-112`):
  `y2 - y1 <= 2` -> `drawChunkBorderLine(y1 - 1, doubleLine=true)`.
- `drawChunkBorderLine` (`DiffDrawUtil.java:112-128`): double = lines at `y` and `y+1`.

Pseudo:
```
boundary = (start == 0) ? top(0) + 1 : bottom(start - 1)
fill_rect(x0, boundary - 1, full_width, 2, type.color)     // gutter + text
```

### 2.4 Inner (word) highlights

`SimpleDiffChangeUi.createInlineHighlighter` (lines 101-114) + `InlineHighlighterBuilder` (`DiffDrawUtil.java:645-679`):
- Type per fragment: `DiffUtil.getDiffType(fragment)` (`util/DiffUtil.java:1555-1581`):
  both sides non-empty = MODIFIED, left only = DELETED, right only = INSERTED.
  So in a blue MODIFIED block, a word can be green (INSERTED) on the right side.
- Color: `type.getColor()` (full color). Not derived darker. The contrast comes from the line
  around it being mixed toward the background (2.5).
- `EXACT_RANGE`: only the characters, no rounding/padding.
- The empty side of a word fragment: `DiffEmptyHighlighterRenderer` (`util/DiffEmptyHighlighterRenderer.java:42`):
  ```
  fill_rect(x_of_offset - 1, line_top, 2, lineHeight, type.color)   // a 2px vertical stroke
  ```

### 2.5 The "ignored" color (derived)

`TextDiffTypeFactory.getIgnoredColor` (`util/TextDiffTypeFactory.java:65-75`, factor at line 91):
```
if the scheme has FOREGROUND for the key: use it
else: ignored = mix(color, editorBg, 0.6)
mix(a, b, t) per channel = a + round(t * (b - a))       // MixedColorProducer.mix, Java Math.round
```
I.e. 40% diff color, 60% editor background. None of the default schemes set FOREGROUND, so
it is always derived.

### 2.6 Gutter vs text in different modes

`DiffLineMarkerRenderer.paint` (lines 33-80): if `editorMode != gutterMode`, the gutter is split at
`whitespaceSeparatorOffset`:
- `[0, xOutline)` is painted with gutterMode.
- `[xOutline, gutterWidth)` is painted with editorMode.

With BY_WORD, editorMode=IGNORED and gutterMode=DEFAULT. Result: the line number area has
full (saturated) color, while the text area and a narrow stripe on the outside of the gutter have the light
ignored color. New UI: `whitespaceSeparatorOffset = gutterWidth - 3` (`EditorGutterComponentImpl.java:1804-1810`).

### 2.7 Other variants

- Neighboring changes without space between: a 1px MODIFIED line on top of the second one (`SimpleDiffChangeUi.java:116-130`,
  `DiffDrawUtil.createLineMarker` lines 394-401).
- Resolved/excluded border: a 1px line (or "bold dotted") on the first and last pixel row
  (`DiffLineMarkerRenderer.kt:103-106`). `BOLD_DOTTED_STROKE` = 2.3px, dash [2,2] (`DiffDividerDrawUtil.java:37-38`).
- A whitespace-only change with BY_WORD: inner fragments is an empty list -> the line is only the light ignored color.

## 3. Separators for collapsed areas

Folding: `tools/util/FoldingModelSupport.java`. A collapsed area becomes one visual line with
the placeholder `"     "` (5 spaces, lines 85, 480). `FOLDED_TEXT_ATTRIBUTES` is set to null (`DiffUtil.java:368-371`),
so there's no fold box. The separator renderer is set with `SeparatorPlacement.TOP` (`DiffDrawUtil.java:420-433`).

### 3.1 Parameters (`util/DiffLineSeparatorRenderer.java:326-367`)

```
step     = max(dividerWidth / 6, 2) = 4px          // a quarter period
height   = 3px                                     // half the amplitude
aaGap    = 1px
period   = 4 * step = 16px
vOffset  = (lineHeight - 2*height - 2*aaGap) / 2   // center in the line
color    = DIFF_SEPARATOR_WAVE ?: DIFF_SEPARATORS_BACKGROUND ?: #808080
stroke   = 1px (2px when hovered), AA
```

### 3.2 The wave tile (16 x 8 px, `createImage` lines 274-310)

```
upper = 1, center = 4, lower = 7, dx = step/2 = 2
move_to(0, upper)
quad_to((2, upper),  (4, center))
quad_to((6, lower),  (8, lower))
quad_to((10, lower), (12, center))
quad_to((14, upper), (16, upper))
```
The tile is drawn every 16th px (`draw`, lines 240-271), y = `lineTop + vOffset`.

### 3.3 Phase

- Gutter: `shiftX = step` (lines 148, 343-345). The left gutter is mirrored.
- Text: `shiftX = -period - (gutterWidth % period) - period + step + hScroll` (lines 184-195),
  i.e. continuous from the gutter into the text, anchored to the viewport (doesn't move on h-scroll).
- Simplified for jetdiff: let `d` = the distance from the divider edge (mirrored on the left side).
  `y(d) = tile((d + 12) mod 16)`. At the divider edge the wave is at center and rises outwards.

### 3.4 The connector across the divider (`drawConnectorLine`, lines 60-126)

`y1/y2` = the top of the folded line in each editor (`DiffDividerDrawUtil.java:155-163`),
plus `vOffset + height + aaGap` (= the wave's center line).

```
path.move_to(x1 - 0.5, y1)
slope = |y2 - y1| / |x2 - x1|
if slope < 0.2:
    mx, my = mid(x1,x2), mid(y1,y2)
    if x2 - x1 > 5*step:                           // 3 half periods
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
stroke(path, waveColor, 1px or 2px hovered)
```

In jetdiff this is the **fold link**.

### 3.5 Description

Optional text (e.g. the enclosing function) is drawn as an inlay hint at `x1 + 5px`, and the wave
is clipped around it (`drawWithDescription`, lines 207-233). Can be dropped in the first version.

## 4. Gutter

Layout left -> right (`EditorGutterComponentImpl.java:221-247`), `GAP_BETWEEN_AREAS = 5px` (line 265):
```
gap | line numbers | gap | (extra line numbers) | gap | annotations | gap | line markers (icons, free painters) | folding
```
- Line numbers: color `LINE_NUMBERS_COLOR`, font = editor font + `editor.gutter.linenumber.font.size.delta`
  (lines 886-891). y baseline = `lineY + ascent`.
- The diff marker is painted over the whole gutter width `x = 0..gutterWidth` (except the annotations area),
  `DiffLineMarkerRenderer.kt:38-79`. So the line numbers sit on a colored background.
  AA is off in the gutter on non-HiDPI (lines 566-567).
- New UI: a 1px vertical `INDENT_GUIDE` line at `whitespaceSeparatorOffset` (lines 584-592), painted
  before the diff markers, so it is hidden in changes.
- Order in the gutter paint (lines 559-600): background -> editor backgrounds -> annotations ->
  separator line -> line markers (diff) -> folding -> line numbers.
- Chevrons (>> / <<): `DiffGutterOperation` puts a `GutterIconRenderer` (Alignment.LEFT) on
  the start line of the change (`util/DiffGutterOperation.java:18-29`, `SimpleDiffChangeUi.java:76-81, 173-187`).
  Only shown if the opposite side is editable. Read-only: no icons. Checkboxes (partial commit)
  come from other viewers, not relevant now.

## 5. Scrollbar / error stripe

`platform/platform-impl/.../EditorMarkupModelImpl.kt`:
- The diff makes one stripe highlighter per change with the key's `ERROR_STRIPE_COLOR`
  (`DiffDrawUtil.java:635-639`, `DiffStripeMarkerTextAttributes` lines 870-884).
  Empty sides get a stripe via `LineMarkerBuilder` on one offset (lines 744-749).
- Wide mark: `x = minMarkHeight + 2`, width `SCROLLBAR_WIDTH = 14px` (lines 407, 1218-1237).
- y mapping (lines 1471-1503): `y = scrollbarTop + editorY / editorPreferredHeight * trackHeight`
  (1:1 if the document is shorter than the track). The end y is the top of the last line in the range, and
  a height < `minMarkHeight` is expanded to `minMarkHeight` (max 4px, lines 1131, 1423-1425).
- Overlap is resolved per layer; the highest layer is shown (lines 1113-1192).
- The left editor has its scrollbar on the left side (mirrored).

In jetdiff this is the **hunk minimap**.

## 6. Color table

Scheme inheritance: New UI Light = "Light" (`themes/expUI/expUI_lightScheme.xml`, parent Default).
New UI Dark = "Dark" (`themes/expUI/expUI_darkScheme.xml`, parent Darcula). Dark and Islands Dark
override no `DIFF_*`, so they inherit the Darcula values. Sources: `platform-resources/src/DefaultColorSchemesManager.xml`
Default 489-523, Darcula 2255-2277. New UI Light MODIFIED: `expUI_lightScheme.xml:260-265`.

`bg` = `BACKGROUND` (line, divider fill, word). `stripe` = `ERROR_STRIPE_COLOR`.
`ignored` = `mix(bg, editorBg, 0.6)`, computed.

### Light

| Type | Classic Default bg | stripe | ignored (on ffffff) | New UI Light bg | stripe | ignored |
|---|---|---|---|---|---|---|
| INSERTED | `#BEE6BE` | `#AADEAA` | `#E5F5E5` | `#BEE6BE` | `#AADEAA` | `#E5F5E5` |
| DELETED  | `#D6D6D6` | `#C8C8C8` | `#EFEFEF` | `#D6D6D6` | `#C8C8C8` | `#EFEFEF` |
| MODIFIED | `#CAD9FA` | `#B8CBF5` | `#EAF0FD` | `#C2D8F2` | `#B6D2F2` | `#E7EFFA` |
| CONFLICT | `#FFD5CC` | `#FFC8BD` | `#FFEEEB` | `#FFD5CC` | `#FFC8BD` | `#FFEEEB` |

### Dark (the same diff colors; a different editor background gives a different ignored)

| Type | bg | stripe | ignored Darcula (2b2b2b) | ignored New UI Dark (1e1f22) | ignored Islands Dark (191a1c) |
|---|---|---|---|---|---|
| INSERTED | `#294436` | `#447152` | `#2A352F` | `#222E2A` | `#1F2B26` |
| DELETED  | `#484A4A` | `#656E76` | `#373737` | `#2F3032` | `#2C2D2E` |
| MODIFIED | `#385570` | `#43698D` | `#303C47` | `#283541` | `#25323E` |
| CONFLICT | `#45302B` | `#8F5247` | `#352D2B` | `#2E2626` | `#2B2322` |

### Surrounding colors

| Key | Classic Light | New UI Light | Darcula | New UI Dark |
|---|---|---|---|---|
| Editor bg (`TEXT`) | `#FFFFFF` | `#FFFFFF` | `#2B2B2B` | `#1E1F22` (Islands `#191A1C`) |
| Gutter / divider bg | `#F0F0F0` | = editor bg | `#313335` | = editor bg |
| `LINE_NUMBERS_COLOR` | `#999999` | `#AEB3C2` | `#606366` | `#4B5059` |
| `LINE_NUMBER_ON_CARET_ROW_COLOR` | - | `#767A8A` | `#A4A3A3` | `#A1A3AB` |
| Wave (`DIFF_SEPARATOR_WAVE`) | `#D0D0D0` | `#D0D0D0` (inherited) | `#555555` | `#555555` (inherited) |

Note: `DIFF_SEPARATORS_BACKGROUND` (e4e6eb / 2b2d30) is set in New UI, but `DIFF_SEPARATOR_WAVE`
takes priority (`DiffLineSeparatorRenderer.java:351-358`) and is inherited from the parent. Color-blind variants
(`deuteranopia`/`protanopia`) exist in the XML and are ignored here.

Where the color is used:
```
editor line bg       = inner ? ignored : bg
gutter line bg       = bg                            (DEFAULT gutterMode)
outermost 3px gutter = inner ? ignored : bg          (New UI)
word bg              = bg for the word's own type
empty side (2px line) = bg
divider fill         = bg
error stripe         = stripe
```

## 7. Polish and alignment

- The divider and the editor share the y axis via `getDividerGraphics`. A header component in the editor
  (breadcrumbs and the like) is added to the offset.
- The horizontal edges of the bands lie on integer y, so they are sharp even with AA. Only the curves get AA-soft.
- An extra 1px stroke along the middle curve keeps steep bands from disappearing (`DiffDrawUtil.java:161-165`).
- Empty side: a 2px band `[boundary-1, boundary+1)` in the editor, the gutter and the divider, so the line flows
  through without a notch. Line 0 is a special case with `+1`, so the band becomes `[0,2)`.
- The divider is filled first with the gutter bg, then the polygons, then the wave connectors on top.
- `STROKE_NORMALIZE` makes 1px strokes on integer y cover exactly one pixel row.
- Alignment mode (off by default) uses `withAlignedHeight` (`DiffDividerDrawUtil.java:510-538`)
  and draws no 2px lines. Not relevant until we support inlays for alignment.

## 8. Recipe for jetdiff (per frame)

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
    paint fold separators (the wave in the collapsed line)
    paint text, line numbers
    paint error stripe
divider:
    fill(dividerBg)
    for change: fill_aa(band_path(...), color); stroke_aa(mid_curve, 1px, color)
    for fold pair: stroke_aa(connector(...), waveColor, 1px)
```
