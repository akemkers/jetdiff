# golden

Orakel for `fixtures/`. Kjører IntelliJ sin ekte diff-kode på `a.txt`/`b.txt` og skriver `expected.json` (format: `fixtures/README.md`).

```
tools/golden/gen.sh
```

Bygger med Gradle (`installDist`) og kjører generatoren over alle `fixtures/*/` som har både `a.txt` og `b.txt`. Krever Gradle 9.x og JDK 25. Output er deterministisk.

## Kilde

- `intellij-src/util-diff/`: uendret kopi av `platform/util/diff/src` fra `intellij-community@7af191d46d84acf6e30e077df05af9a4c9fe9005`. Apache-2.0, se `NOTICE`.
- Avhengigheter fra `https://www.jetbrains.com/intellij-repository/releases`: `com.jetbrains.intellij.platform:util-base-multiplatform` og `util-multiplatform` `262.10968.117` (drar inn `kotlin-stdlib` 2.4.0 og `org.jetbrains:annotations` 26.0.2). Kompilerer mot vendret kode uten API-avvik.
- `intellij-src/diff-impl/`: `InnerFragmentsPolicy` (uendret) og en trimmet `ComparisonManagerImpl` (se under).

## Stubs i ComparisonManagerImpl

Logikken er uendret. Kun IDE-avhengigheter er byttet:

- `ProgressIndicator` / `IndicatorCancellationChecker` -> `CancellationChecker.EMPTY`. `ByLine`/`ByWord`-wrapperne i diff-impl kaller bare `ByLineRt`/`ByWordRt`, så de kalles direkte.
- `Registry.is("diff.by.word.deprioritize.line.differences")` -> `false`, som er default i `platform/diff-impl/resources/intellij.platform.diff.impl.xml`. `compareLinesWordFirst` er derfor ikke med.
- `LineOffsetsUtil.create(text)` -> `LineOffsetsImpl.create(text)` (det er alt den gjør).
- `ContainerUtil.getLastItem` og `com.intellij.util.Consumer` -> vanlig Java.
- Kun to-sidige metoder for linjer, ord, tegn og `processBlocks` er beholdt.

## Hva `lines`, `words` og `viewer` er

For hver `ComparisonPolicy` i `DEFAULT`, `TRIM_WHITESPACES`, `IGNORE_WHITESPACES`:

```
lo1, lo2 = LineOffsetsImpl.create(text1), LineOffsetsImpl.create(text2)
lines  = compareLines(text1, text2, lo1, lo2, policy)
words  = compareLinesInner(text1, text2, lo1, lo2, policy, InnerFragmentsPolicy.WORDS)
viewer = processBlocks(words, text1, text2, policy, squash = true, trim = false)
```

`viewer` er det `SimpleDiffViewer` (side-by-side) viser med standardinnstillinger:

- `SimpleDiffViewer.computeDifferences` -> `SmartTextDiffProvider` -> `SimpleTextDiffProvider.compareRange` -> `compareLinesInner(text1, text2, lo1, lo2, policy, highlightPolicy.getFragmentsPolicy())`, deretter `TwosideTextDiffProviderBase.doCompare` -> `processBlocks(fragments, text1, text2, policy, squash, trim)`.
- Default `HighlightPolicy` er `BY_WORD` og default `IgnorePolicy` er `DEFAULT` (`TextDiffSettingsHolder.PlaceSettings`).
- `BY_WORD`: `fragmentsPolicy = WORDS`, `isShouldSquash = true`.
- `squash = highlight.isShouldSquash && ignore.isShouldSquash` = `true` for `IgnorePolicy` `DEFAULT`, `TRIM_WHITESPACES` og `IGNORE_WHITESPACES`.
- `trim = ignore.isShouldTrimChunks` = `false` for alle tre. Kun `IGNORE_WHITESPACES_CHUNKS` ("ignore whitespaces and empty lines") setter `trim = true`. Den er ikke med i fixturene.

Policy-nøklene i JSON tilsvarer `IgnorePolicy` `DEFAULT`, `TRIM_WHITESPACES` og `IGNORE_WHITESPACES`, som mapper 1:1 til `ComparisonPolicy`.

## Offsets

Java bruker UTF-16-indekser. Generatoren konverterer til UTF-8-byteoffsets: `o1`/`o2` er absolutte i teksten, inner-offsets er relative til fragmentets start (`utf8(start + inner) - utf8(start)`).

## Linjeskift

IntelliJ differ `Document`-innhold, som alltid har `\n` (CRLF normaliseres ved lasting). `LineOffsetsImpl` splitter kun på `\n`. Generatoren feiler derfor hvis en fixture inneholder `\r`, og det finnes ingen CRLF-caser.
