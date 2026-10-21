# golden

The oracle for `fixtures/`. Runs IntelliJ's real diff code on `a.txt`/`b.txt` and writes `expected.json` (format: `fixtures/README.md`).

```
tools/golden/gen.sh
```

Builds with Gradle (`installDist`) and runs the generator over every `fixtures/*/` that has both `a.txt` and `b.txt`. Requires Gradle 9.x and JDK 25. The output is deterministic.

## Source

- `intellij-src/util-diff/`: an unchanged copy of `platform/util/diff/src` from `intellij-community@7af191d46d84acf6e30e077df05af9a4c9fe9005`. Apache-2.0, see `NOTICE`.
- Dependencies from `https://www.jetbrains.com/intellij-repository/releases`: `com.jetbrains.intellij.platform:util-base-multiplatform` and `util-multiplatform` `262.10968.117` (pulls in `kotlin-stdlib` 2.4.0 and `org.jetbrains:annotations` 26.0.2). Compiles against the vendored code without API differences.
- `intellij-src/diff-impl/`: `InnerFragmentsPolicy` (unchanged) and a trimmed `ComparisonManagerImpl` (see below).

## Stubs in ComparisonManagerImpl

The logic is unchanged. Only IDE dependencies are replaced:

- `ProgressIndicator` / `IndicatorCancellationChecker` -> `CancellationChecker.EMPTY`. The `ByLine`/`ByWord` wrappers in diff-impl only call `ByLineRt`/`ByWordRt`, so those are called directly.
- `Registry.is("diff.by.word.deprioritize.line.differences")` -> `false`, which is the default in `platform/diff-impl/resources/intellij.platform.diff.impl.xml`. `compareLinesWordFirst` is therefore not included.
- `LineOffsetsUtil.create(text)` -> `LineOffsetsImpl.create(text)` (that's all it does).
- `ContainerUtil.getLastItem` and `com.intellij.util.Consumer` -> plain Java.
- Only the two-sided methods for lines, words, characters and `processBlocks` are kept.

## What `lines`, `words` and `viewer` are

For each `ComparisonPolicy` in `DEFAULT`, `TRIM_WHITESPACES`, `IGNORE_WHITESPACES`:

```
lo1, lo2 = LineOffsetsImpl.create(text1), LineOffsetsImpl.create(text2)
lines  = compareLines(text1, text2, lo1, lo2, policy)
words  = compareLinesInner(text1, text2, lo1, lo2, policy, InnerFragmentsPolicy.WORDS)
viewer = processBlocks(words, text1, text2, policy, squash = true, trim = false)
```

`viewer` is what `SimpleDiffViewer` (side by side) shows with the default settings:

- `SimpleDiffViewer.computeDifferences` -> `SmartTextDiffProvider` -> `SimpleTextDiffProvider.compareRange` -> `compareLinesInner(text1, text2, lo1, lo2, policy, highlightPolicy.getFragmentsPolicy())`, then `TwosideTextDiffProviderBase.doCompare` -> `processBlocks(fragments, text1, text2, policy, squash, trim)`.
- The default `HighlightPolicy` is `BY_WORD` and the default `IgnorePolicy` is `DEFAULT` (`TextDiffSettingsHolder.PlaceSettings`).
- `BY_WORD`: `fragmentsPolicy = WORDS`, `isShouldSquash = true`.
- `squash = highlight.isShouldSquash && ignore.isShouldSquash` = `true` for `IgnorePolicy` `DEFAULT`, `TRIM_WHITESPACES` and `IGNORE_WHITESPACES`.
- `trim = ignore.isShouldTrimChunks` = `false` for all three. Only `IGNORE_WHITESPACES_CHUNKS` ("ignore whitespaces and empty lines") sets `trim = true`. It is not in the fixtures.

The policy keys in the JSON correspond to `IgnorePolicy` `DEFAULT`, `TRIM_WHITESPACES` and `IGNORE_WHITESPACES`, which map 1:1 to `ComparisonPolicy`.

## Offsets

Java uses UTF-16 indices. The generator converts them to UTF-8 byte offsets: `o1`/`o2` are absolute in the text, and inner offsets are relative to the fragment's start (`utf8(start + inner) - utf8(start)`).

## Line breaks

IntelliJ diffs `Document` content, which always has `\n` (CRLF is normalized on load). `LineOffsetsImpl` splits only on `\n`. The generator therefore fails if a fixture contains `\r`, and there are no CRLF cases.
