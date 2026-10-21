# 01 - IntelliJ's diff algorithm (reverse-engineered)

Source: sparse clone in `~/git/_ref/intellij-community`. Abbreviations for paths:

- `U/` = `platform/util/diff/src/com/intellij/diff/comparison/`
- `UD/` = `platform/util/diff/src/com/intellij/util/diff/`
- `F/` = `platform/util/diff/src/com/intellij/diff/fragments/`
- `I/` = `platform/diff-impl/src/com/intellij/diff/`

All line numbers are from the files as they are in the clone.

Terms used below:

- **DiffIterable**: a list of `Range(start1,end1,start2,end2)` for changes over two sequences of length `length1/length2`. The opposite (unchanged) is derived.
- **Fair**: all unchanged ranges have the same length on both sides (1:1 match). Everything in the pipeline is fair.
- **ChangeBuilder**: `markEqual(i1,i2[,end1,end2])` in ascending order; the holes between become changes (`U/iterables/DiffIterableUtil.kt:244-302`). `ExpandChangeBuilder` also trims equal elements at the edges of each change (`:304-310`).
- **expand(a,b,range)**: peel equal elements off the front and back of a range (`U/TrimUtil.kt:323-339`).

---

## 1. Pipeline: two texts to `List<LineFragment>` with inner fragments

Entry point: `ComparisonManagerImpl.compareLinesInner` (`I/comparison/ComparisonManagerImpl.java:158-177`).

```
compareLinesInner(text1, text2, policy, fragmentsPolicy):
  if fragmentsPolicy == WORDS && Registry("diff.by.word.deprioritize.line.differences"):
      return compareLinesWordFirst(...)          # experimental, see 4.6
  lineOffsets = split on '\n' (LineOffsetsImpl.kt:48-53, the last line can be empty)
  lines = line content WITHOUT '\n' (ComparisonManagerImpl.java:580-586)
  iterable  = ByLineRt.compare(lines1, lines2, policy)       # line diff
  fragments = convertIntoLineFragments(iterable)             # :391-432, offsets INCLUDING '\n'
  if fragmentsPolicy != NONE:
      fragments = createInnerFragments(fragments, ...)       # :217-312
  return fragments
# Afterwards, in the viewer layer: processBlocks(squash, trim)  (TwosideTextDiffProviderBase.java:74-81)
```

### 1.1 Line diff: `ByLineRt.doCompare` (`U/ByLineRt.kt:61-82`)

Each line is wrapped in `Line(content, policy)` with hash and equals according to the policy, and `nonSpaceChars` = the number of characters that are not `' '`, `'\t'`, `'\n'` (`:418-455`).

```
doCompare(lines1, lines2, policy):
  if policy == IGNORE_WHITESPACES:
      ch = compareSmart(lines1, lines2)                 # two-step, see ch. 2
      ch = LineChunkOptimizer(lines1, lines2, ch)       # ch. 3
      return expandRanges(ch)                           # trim equal lines at the edges (:364-377)
  else:                                                 # DEFAULT and TRIM_WHITESPACES
      iw1, iw2 = the same lines but with IGNORE_WHITESPACES equals (:387-397)
      ch = compareSmart(iw1, iw2)                       # the match IGNORES whitespace
      ch = LineChunkOptimizer(lines1, lines2, ch)       # NB: uses the original policy's equals
      return correctChangesSecondStep(lines1, lines2, ch)  # ch. 1.2
```

The point: even in DEFAULT mode, lines are first matched ignoring whitespace. That way a re-indented block is still paired line by line, and ByWord can afterwards show just the whitespace changes.

### 1.2 `correctChangesSecondStep` (`U/ByLineRt.kt:135-266`)

Takes the IW matching and makes it strict (policy-equal). The comment at `:140-166` is the reference:

```
builder = ExpandChangeBuilder(lines1, lines2)
sample = null            # a "group" of IW-equal lines that are not exactly equal
for (i1, i2) in all IW-matched pairs, in order:
    if !IW_equal(sample, lines1[i1]):
        flush(i1, i2)
        if lines1[i1] == lines2[i2]: builder.markEqual(i1, i2)
        else:                        sample = lines1[i1]
flush(len1, len2)

flush(end1, end2):
    if sample == null: return
    sub1 = [i in max(last1, builder.index1) .. end1 where IW_equal(sample, lines1[i])]
    sub2 = the same on side 2
    alignExactMatching(sub1, sub2); sample = null

alignExactMatching(sub1, sub2):
    if max(|sub1|,|sub2|) > 10 or |sub1| == |sub2|:
        zip in pairs, markEqual where lines1 == lines2          # :226-240
    else:
        brute force all C(n,k) combinations, pick the one with the most exactly equal pairs
        (getBestMatchingAlignment :268-319), markEqual the equal ones
```

Effect: among several IW-equal candidates, the exactly equal one is picked as the partner. Pairs that are not exact become a change (and get a ByWord highlight of the whitespace).

### 1.3 The LCS engine: `Diff.buildChanges` (`UD/Diff.kt:26-102`)

All levels (lines, words, characters) go through `DiffIterableUtil.diff` (`U/iterables/DiffIterableUtil.kt:26-69`) to `Diff.buildChanges`.

```
buildChanges(objs1, objs2):
  pre  = common prefix; post = common suffix                 # :27-28, :104-144
  if one of the trimmed sides is empty: return a single change  # doBuildChangesFast :55-66
  ints = Enumerator: object -> int id via hashCode/equals    # :34-36 (interning)
  doBuildChanges(ints1, ints2):
    d1, d2 = Reindexer.discardUnique(ints1, ints2)            # :72-73
    if d1 and d2 are empty: everything is one change          # :75-79
    if DiffConfig.USE_PATIENCE_ALG (=false):  Patience
    else:
      try:   MyersLCS(d1,d2).executeWithThreshold()           # :88-91
      catch FilesTooBig: PatienceIntLCS(d1,d2).execute(failOnSmallReduction=true)  # :93-97
    Reindexer.reindex(changes, builder)                       # map back to the original indices
```

**Reindexer** (`UD/Reindexer.kt:15-18, 37-52, 54-117`): removes elements that don't exist on the other side at all (they can never be matched). `discard1 = ints1 ∩ set(ints2)`, `discard2 = ints2 ∩ set(discard1)`. Myers runs on the compressed sequences, and `reindex` expands the BitSets back: discarded elements are marked as changed. The same idea as GNU diff's "discard confusing lines", but simpler (only completely unique ones).

**MyersLCS** (`UD/MyersLCS.kt`): classic linear-memory Myers with middle snake and recursion (`:90-193`). Properties:

- `executeWithThreshold`: max D = `max(20000 + 10*sqrt(N1+N2), DELTA_THRESHOLD_SIZE=20000)` (`:76-80`, `UD/DiffConfig.kt:10`). If D is exceeded, `FilesTooBigForDiffException` is thrown (`:188-191`).
- `executeLinear`: the same threshold, but doesn't throw. If it's exceeded, the whole sub-range stays marked as changed (the bits are set in `init`, `:41-42`).
- When `td` (the D found) is 0 or 1 it goes greedy: match a common run, otherwise jump ahead on the longer side (`:170-187`). That decides the tiebreak for single insertions.

**PatienceIntLCS** (`UD/PatienceIntLCS.kt:38-123`), fallback only:

```
execute(range, counter):          # counter starts at 2 when failOnSmallReduction
  trim common prefix/suffix
  if one side is empty: mark a change; return
  if counter == 0: checkReduction          # throw if the subproblem is still >= half of the whole
  counter = max(-1, counter-1)
  anchors = UniqueLCS(range)               # elements unique on BOTH sides, LIS on position
  if anchors == null:
      if counter >= 0: checkReduction
      MyersLCS(range).executeLinear()
  else:
      recurse on each gap between anchors (and before/after)
```

`checkReduction` (`:154-158`) throws -> `DiffTooBigException` -> the viewer shows "diff too big" (`I/tools/simple/SimpleDiffViewer.java:277-278`).

**UniqueLCS** (`UD/UniqueLCS.kt:23-95`): a hashmap for elements that occur exactly once on each side, then patience sorting (LIS with binary search and a predecessor array).

### 1.4 Size limits and fallbacks, all together

| Limit | Value | Where | Consequence |
|---|---|---|---|
| Myers D threshold | `max(20000+10*sqrt(N), 20000)` | `UD/MyersLCS.kt:77` | fallback to Patience |
| Patience reduction | subproblem >= 50 % on both sides after 2 levels | `UD/PatienceIntLCS.kt:67,74,154` | `DiffTooBigException`, the whole diff is aborted |
| Myers inside Patience | the same threshold, doesn't throw | `UD/MyersLCS.kt:56-64` | the range becomes one big change |
| Bad inner lines | `MAX_BAD_LINES = 3` | `UD/DiffConfig.kt:11`, `I/comparison/ComparisonManagerImpl.java:225-237` | after 3 failed ByWord attempts, inner is skipped for the rest |
| Brute-force align | n > 10 | `U/ByLineRt.kt:227` | a simple zip instead |
| Unimportant line | `nonSpaceChars <= 3` | `UD/DiffConfig.kt:12` | ch. 2 and 3 |

---

## 2. "Unimportant" lines: two-step comparison

`compareSmart` (`U/ByLineRt.kt:335-362`) + `SmartLineChangeCorrector` (`U/ChangeCorrector.kt:93-126`, base class `:14-57`).

```
compareSmart(lines1, lines2):
  T = 3                                           # getUnimportantLineCharCount
  big1 = [(i, l) for l in lines1 if l.nonSpaceChars > T]    # e.g. not "}", "", "  );"
  big2 = the same
  ch = diff(big1.lines, big2.lines)               # step 1: only the "important" lines
  # step 2: ChangeCorrector
  last1 = last2 = 0
  for (b1, b2) in matched pairs in ch:
      o1, o2 = big1.index[b1], big2.index[b2]     # original line index
      matchGap(last1, o1, last2, o2)
      builder.markEqual(o1, o2)
      last1, last2 = o1+1, o2+1
  matchGap(last1, len1, last2, len2)

matchGap(s1,e1,s2,e2):                             # ChangeCorrector.kt:101-115
  r = expand(lines, s1..e1, s2..e2)                # peel equal lines at the edges
  inner = diff(lines1[r.s1..r.e1], lines2[r.s2..r.e2])   # full diff on ALL lines in the gap
  markEqual the edges + everything unchanged in inner
```

Why: lines like `}`, empty lines and `*/` match everywhere and pull the LCS in the wrong direction. By anchoring on the important lines first, and then only filling the gaps, the small lines only get matched within the right area.

The same pattern exists at character level: `ByCharRt.compareTwoStep` diffs the non-whitespace code points first, and fills the gaps with a full character diff via `DefaultCharChangeCorrector` (`U/ByCharRt.kt:52-64`, `U/ChangeCorrector.kt:62-91`).

---

## 3. ChunkOptimizer: moving chunk boundaries (the "slider" heuristic)

The generic part (`U/ChunkOptimizer.kt:11-85`). It works on **unchanged** ranges: two neighboring ranges `r1, r2` where the change between them is a pure insert or delete (one side "touches", i.e. `r1.end == r2.start` on that side).

```
build():
  ranges = []
  for r in iterable.unchanged: ranges.push(r); processLast()
  return fair(createUnchanged(ranges))

processLast():                                           # :28-77
  if |ranges| < 2: return
  r1, r2 = the last two
  if r1.end1 != r2.start1 and r1.end2 != r2.start2: return   # a modified chunk, can't be moved
  count1 = len(r1); count2 = len(r2)
  eqF = expandForward(data, from r1.end,         max count2)   # how far the gap can slide down
  eqB = expandBackward(data, backward from r2.start, max count1)  # how far it can slide up
  if eqF == 0 and eqB == 0: return
  if eqF == count2:   # [A]B[B] -> [AB]B : slide all the way down, r2 is swallowed, merge with the next change
      replace r1,r2 with Range(r1.start, r1.end+count2); processLast(); return
  if eqB == count1:   # [A]A[B] -> A[AB] : slide all the way up, r1 is swallowed
      replace r1,r2 with Range(r2.start-count1, r2.end); processLast(); return
  touch = LEFT if r1.end1 == r2.start1 else RIGHT    # the side WITHOUT inserted lines
  shift = getShift(touch, eqF, eqB, r1, r2)          # >0 down, <0 up
  if shift != 0: r1.end += shift; r2.start += shift
```

Rule 1 (shared by lines and words): minimize the number of chunks. If an insert can slide so that it meets a neighboring change, that is done.

### 3.1 `LineChunkOptimizer.getShift` (`U/ChunkOptimizer.kt:174-260`)

```
getShift(touch, eqF, eqB, r1, r2):
  for (fn, T) in [(unchangedBoundary, 0), (changedBoundary, 0),
                  (unchangedBoundary, 3), (changedBoundary, 3)]:
      s = fn(touch, eqF, eqB, r1, r2, T)
      if s != null: return s
  return 0

unimportant(line, T) = line.nonSpaceChars <= T     # T=0: blank/whitespace-only, T=3: "}", "});" etc.

unchangedBoundary(...):              # :203-216  "insert right before/after an empty line"
  L = lines on the touch side; p = r2.start on the touch side (insertion point)
  fwd = first i in 0..eqF   where unimportant(L[p+i])     otherwise -1
  bwd = first i in 0..eqB   where unimportant(L[p-1-i])   otherwise -1
  return pick(fwd, bwd)

changedBoundary(...):                # :222-237  "insert starts/ends with an empty line"
  L = lines on the other side (the one with inserted lines)
  cs = r1.end, ce = r2.start (on that side) = the inserted block
  fwd = first i in 0..eqF   where unimportant(L[cs+i])    otherwise -1
  bwd = first i in 0..eqB   where unimportant(L[ce-1-i])  otherwise -1
  return pick(fwd, bwd)

pick(fwd, bwd):                      # :254-259
  if fwd == -1 and bwd == -1: return null      # try the next rule
  if fwd == 0 or bwd == 0:    return 0         # already on a good boundary, don't move
  return fwd if fwd != -1 else -bwd            # prefer moving DOWN
```

Priority, summed up: (1) a blank line in the unchanged context right by the insertion, (2) an inserted block that starts/ends with a blank line, (3)-(4) the same with "nearly empty" lines (<= 3 characters). Down is preferred. **Indentation is not used** (no git-style indent heuristic).

### 3.2 `WordChunkOptimizer.getShift` (`U/ChunkOptimizer.kt:100-163`)

Rule 2: minimize the number of "sentences" that change, where a sentence is words not separated by whitespace (e.g. `1.0.123 1.0.155` vs `1.0.123 1.0.134 1.0.155`).

```
getShift(touch, eqF, eqB, r1, r2):
  W = words on the touch side; T = text on the touch side; p = r2.start on the touch side
  if sepWS(T, W[p-1], W[p]): return 0                         # already on a whitespace boundary
  for i in 0..eqF-1: if sepWS(T, W[p+i], W[p+i+1]):   return i+1     # shift down
  for i in 0..eqB-1: if sepWS(T, W[p-1-i-1], W[p-1-i]): return -(i+1) # shift up
  return 0

sepWS(T, a, b) = a or b is a NewlineChunk
                 or there is a ' ', '\t', '\n' in T[a.end .. b.start]      # :151-161
```

---

## 4. Word level (ByWord)

### 4.1 Tokenizing: `getInlineChunks` (`U/ByWordRt.kt:637-683`)

Iterate over code points:

```
isPunctuation(c) = ASCII 33-47, 58-64, 91-96, 123-126, EXCEPT '_'   # U/TrimUtil.kt:16-22
isWhiteSpace(c)  = c in {' ', '\t', '\n'}                           # :29-31
isAlpha(c)       = !isWhiteSpace(c) && !isPunctuation(c)            # :24-27  (NB: '\r' is "alpha")
isContinuousScript(c):                                              # :33-46
    c < 128 -> false; decimal digit -> false; non-BMP -> true
    ideographic -> true; not alphabetic -> true   # e.g. unicode symbols and non-ASCII punctuation
    otherwise Hiragana/Katakana/Thai/Javanese -> true

for each code point c:
  if isAlpha(c) && !isContinuousScript(c): add to the current word (hash = hash*31 + c)
  else:
    end the current word -> WordChunk
    if isAlpha(c):   a WordChunk for c alone        # CJK and the like: each character is a word
    elif c == '\n':  NewlineChunk
    # whitespace and ASCII punctuation do NOT become tokens; they live in the gaps
```

`WordChunk.equals` = exact text (DEFAULT) regardless of policy (`:1098-1107`). All `NewlineChunk`s are equal (`:1118-1127`).

### 4.2 `compareAndSplit` (`U/ByWordRt.kt:89-134`), used for inner fragments

```
compareAndSplit(t1, t2, policy):
  w1, w2 = getInlineChunks(t1), getInlineChunks(t2)
  ch = diff(w1, w2)                                  # the same LCS engine
  ch = WordChunkOptimizer(w1, w2, t1, t2, ch)        # 3.2
  blocks = LineFragmentSplitter(t1, t2, w1, w2, ch).run()   # 4.4
  for block in blocks:
      sub = ch limited to the block
      it  = AdjustmentPunctuationMatcher(sub)        # 4.3, goes from word indices to character offsets
      it  = matchAdjustmentWhitespaces(it, policy)   # 4.3
      fragments = it.changes as DiffFragment (relative to the block start)
      emit LineBlock(fragments, offsets, the number of '\n' on each side)
```

### 4.3 Punctuation and whitespace in the gaps

**AdjustmentPunctuationMatcher** (`U/ByWordRt.kt:695-874`): matched words are marked equal on character offsets. The gaps between words (whitespace + punctuation) are compared with `ByCharRt.comparePunctuation` (`U/ByCharRt.kt:95-134`), which only runs LCS on ASCII punctuation characters and leaves everything else unmatched.

```
for each matched word pair (i1, i2):
    matchBackward(the gap BEFORE i1/i2); markEqual(the word); matchForward(remember the gap AFTER i1/i2)
matchBackward(the gap before the next match) against the remembered gap after the previous match:
  the same gap on both sides ("A B" vs "A B")         -> comparePunctuation(gap1, gap2)
  unmatched words on both sides ("A X B"/"A Y B")     -> compare the after-gap and the before-gap separately
  unmatched words on one side only ("A B"/"A Y B")    -> one gap against the concatenation of two gaps
                                                         (comparePunctuation2Side, :550-583)
```

An example in the code: `"[ X { A ! B } Y ]"` vs `"( X ... Y )"` gives the groups `[`/`(`, `{`+`}`/`...`, `]`/`)` (`:689-694`).

**Whitespace per policy** (`U/ByWordRt.kt:488-507`). After punctuation matching, all the whitespace in the gaps is still "changed":

```
DEFAULT  (DefaultCorrector :880-906):
  for each change: peel EQUAL whitespace characters from the end, then from the start
  (expandWhitespacesBackward/Forward, TrimUtil.kt:126-148); drop empty ones
  => "a b" vs "a b": no fragment.  "a  b" vs "a b": a delete of one space.

TRIM_WHITESPACES (DefaultCorrector, then TrimSpacesCorrector :989-1028):
  trim edge whitespace ONLY if it is leading/trailing on the line (isLeadingTrailingSpace :590-623)
  drop if the rest is exactly equal

IGNORE_WHITESPACES (IgnoreSpacesCorrector :939-962):
  expandWhitespaces (match equal whitespace), trim ALL whitespace at the edges,
  drop if IW-equal
```

### 4.4 LineFragmentSplitter (`U/LineFragmentSplitter.kt`)

Splits a multi-line line fragment into "logically different" blocks based on the word matching. Indices: `-1` = start, `words.size` = end.

```
run():                                                    # :29-58
  hasEqual = false; last = (-1,-1); pending = null
  for (i1, i2) in matched word pairs:
      if w1[i1] and w2[i2] are both NewlineChunk:
          addChunk(i1, i2, hasEqual); hasEqual = false          # split AFTER a matched '\n'
      else:
          if both are the first word on their line:
              addChunk(i1-1, i2-1, hasEqual); hasEqual = false  # split BEFORE a matched line start
          hasEqual = true
  addChunk(len1, len2, hasEqual); flush pending

addChunk(e1, e2, hasEqual):                              # :60-80
  chunk = the block from last to e (offsets = right after '\n'), with the flags:
          hasEqualWords, hasWordsInside (anything other than '\n'), isEqualIW
  if pending and shouldMerge(pending, chunk): pending = merge
  else: emit pending; pending = chunk

shouldMerge(a, b):                                        # :130-138
  !a.hasEqual && !b.hasEqual        -> true   # lines that only matched on '\n' are merged
  a.isEqualIW && b.isEqualIW        -> true   # pure whitespace changes are merged
  !a.hasWords || !b.hasWords        -> true   # a block without words (empty lines) is squashed
  otherwise false
```

### 4.5 From blocks to LineFragments, and the fallback to whole lines (`I/comparison/ComparisonManagerImpl.java:217-312`)

```
createInnerFragments(fragments):
  bad = 0
  for f in fragments:
    try:
      if f is a pure insert/delete (startLine==endLine on one side):        # :257-265
          if isEqualTexts(sub1, sub2, policy): f.inner = []   # an "ignored" change
          else:                                f.inner = null  # whole line
      elif bad >= 3: f unchanged (inner = null)                             # :231, :267
      else:
          blocks = ByWord.compareAndSplit(sub1, sub2, policy)   # sub = text INCLUDING '\n'
          for block i: emit LineFragment(lines from the newline counts,
                                         the last block gets f.endLine, :299-301,
                                         inner = block.fragments)
    catch DiffTooBig: keep f; bad++
```

A whole-line highlight happens when:

- `innerFragments == null`: an insert/delete, too many failures, DiffTooBig, or
- `LineFragmentImpl` gets exactly one inner fragment that covers all of both sides, and sets it to `null` (`F/LineFragmentImpl.kt:18-19, 72-83`).

`innerFragments == []` (an empty list) means: a changed line, but no visible differences under the policy (e.g. only whitespace in IGNORE mode).

### 4.6 `compareWordsFirst` (experimental, `U/ByWordRt.kt:176-447`)

Controlled by the registry key `diff.by.word.deprioritize.line.differences` (the default is not in the clone; assumed off). Diffs only `WordChunk`s (without newlines) over the whole text, and lifts the result to line blocks via `LineUplifter`. Useful for Java->Kotlin, or text with different line breaking. Can wait.

---

## 5. ComparisonPolicy and UI options

`ComparisonPolicy { DEFAULT, TRIM_WHITESPACES, IGNORE_WHITESPACES }` (`U/ComparisonPolicy.kt:4-8`). Equals/hash (`U/ComparisonUtil.kt:14-37`):

- DEFAULT: exact.
- TRIM: ignore leading/trailing `' ' '\t' '\n'`. For multi-line text, each line is trimmed (`equalsTrimWhitespaces :58-86`).
- IGNORE: ignore all `' ' '\t' '\n'` everywhere.

(The `equalsIgnoreWhitespaces` implementation itself is not in the sparse clone. `TrimUtil.kt:583` says the whitespace definition is "from Strings.isWhiteSpace", i.e. `' ' '\t' '\n'`.)

| Step | DEFAULT | TRIM | IGNORE |
|---|---|---|---|
| Line LCS | IW lines (`ByLineRt.kt:75-78`) | IW lines | IW lines |
| Line post-processing | `correctChangesSecondStep` | `correctChangesSecondStep` (with TRIM equals) | `expandRanges` |
| Word LCS | exact words | exact words | exact words |
| Whitespace in word gaps | `DefaultCorrector` | Default + `TrimSpacesCorrector` | `IgnoreSpacesCorrector` |
| Char mode | `compareTwoStep` | `compareTrimWhitespaces` | `compareIgnoreWhitespaces` (`ComparisonManagerImpl.java:323-339`) |

The UI-level `IgnorePolicy` (`I/tools/util/base/IgnorePolicy.java:11-43`):

| IgnorePolicy | ComparisonPolicy | squash | trimChunks |
|---|---|---|---|
| DEFAULT | DEFAULT | yes | no |
| TRIM_WHITESPACES | TRIM | yes | no |
| IGNORE_WHITESPACES | IGNORE | yes | no |
| IGNORE_WHITESPACES_CHUNKS ("and empty lines") | IGNORE | yes | **yes** |
| FORMATTING | DEFAULT + ignored ranges from the language plugin (`compareLinesWithIgnoredRanges`, `ComparisonManagerImpl.java:623-650`) | yes | no |

`HighlightPolicy` (`I/tools/util/base/HighlightPolicy.java`): BY_LINE (inner NONE), BY_WORD (WORDS, squash), BY_WORD_SPLIT (WORDS, **no** squash), BY_CHAR (CHARS), DO_NOT_HIGHLIGHT.

Squash = `highlight.isShouldSquash && ignore.isShouldSquash` (`I/tools/util/text/TwosideTextDiffProviderBase.java:75-76`).

### 5.1 `processBlocks` squash/trim (`I/comparison/ComparisonManagerImpl.java:448-544`)

```
processBlocks(fragments, squash, trim):
  for each group of adjacent fragments (endLine == the next startLine and endOffset == the next startOffset, :546-555):
    if trim && policy == IGNORE:
        drop from the start/end of the group the fragments that are IW-equal or have inner == []
    if squash: merge the group into one LineFragment:
        inner = the concatenation of all inner (inner == null -> one fragment over everything), shifted
                to the new start; adjacent inner fragments are merged (:513-544)
```

Important: LineFragmentSplitter splits a change into several fragments, but in the default mode (BY_WORD) squash glues them back together. The splitting still affects the result. Each block gets its own punctuation/whitespace matching, and a block without inner (whole line) becomes one big inner fragment after squash. BY_WORD_SPLIT shows the blocks as separate changes.

---

## 6. Data model

```
DiffFragment {                         # F/DiffFragment.kt:10-17
  startOffset1, endOffset1, startOffset2, endOffset2   # character offsets (UTF-16 in Java)
}                                      # not empty, start <= end (F/DiffFragmentImpl.kt:12-14)

LineFragment : DiffFragment {          # F/LineFragment.kt:12-27
  startLine1, endLine1, startLine2, endLine2   # half-open line intervals
  offsets cover whole lines INCLUDING '\n' (can be missing on the last line)
  innerFragments: List<DiffFragment>?  # relative to the fragment's startOffset
                                       # null = whole line, [] = no visible difference
}
```

Offsets for an empty side (`ComparisonManagerImpl.java:416-432`): `start == end` gives `lineStart(start)`, or the end of the text if `start == lineCount`.

Type (`I/util/DiffUtil.java:1549-1581`):

```
getLineDiffType(f) = type(f.startLine1 != f.endLine1, f.startLine2 != f.endLine2)
getDiffType(inner) = type(endOffset1 != startOffset1, endOffset2 != startOffset2)
type(del, ins) = MODIFIED if del && ins; DELETED if del; INSERTED if ins
```

The same rule is used for the line and inner levels. An inner change inside a MODIFIED line can therefore be INSERTED or DELETED and is colored accordingly.

---

## 7. `imara-diff` as a base?

Local copy: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/imara-diff-0.2.0/`.

What it gives:

- `Algorithm::{Histogram, Myers, MyersMinimal}` (`src/lib.rs:170-218`), prefix/suffix stripping (`:268-271`), Myers with git/gnu heuristics, and discarding tokens that don't exist on the other side (corresponds to Reindexer).
- `InternedInput<T: Hash+Eq>` / `TokenSource` (`src/intern.rs:32-64`): we can intern our own `Line` type with a policy-dependent `Hash`/`Eq`, and words/code points.
- The result as `removed/added: Vec<bool>` plus `hunks()`, i.e. the same shape as IntelliJ's BitSets.
- Slider post-processing with git's indent heuristic (`postprocess_lines`, the `SliderHeuristic` trait).

Assessment:

- **Sensible as an LCS primitive**, but not bit-for-bit equal to IntelliJ. The Myers tiebreak, the threshold (`MAX_COST_MIN=256`, `sqrt`-based, `src/myers.rs:32,61`) and the fact that it never fails give different choices than IntelliJ's "exact Myers up to D=20000, otherwise Patience". Histogram gives even more divergent results and should not be used on characters.
- IntelliJ's `MyersLCS` + `Reindexer` + `PatienceIntLCS` + `UniqueLCS` add up to about 650 lines of Kotlin. Porting them directly is cheap and makes golden tests against IntelliJ possible. Recommendation: define an `Lcs` trait, port the IntelliJ engine as the reference, and keep imara-diff's `Myers` as an alternative backend for large files or benchmarking.
- **Don't** use imara's `postprocess_lines`. The IntelliJ sliders come from `ChunkOptimizer` and must run on the raw LCS output.

Must be written ourselves (whatever the backend):

1. `Line` with policy hash/eq and `nonSpaceChars`; line splitting on `'\n'`.
2. `compareSmart` + `SmartLineChangeCorrector` (ch. 2).
3. The IW-first strategy and `correctChangesSecondStep` with brute-force alignment (ch. 1.2).
4. The generic `ChunkOptimizer` + `LineChunkOptimizer` + `WordChunkOptimizer` (ch. 3).
5. `getInlineChunks` with unicode classification (`isContinuousScript` needs general category + script tables, e.g. the `unicode-general-category`/`unicode-script` crates).
6. `LineFragmentSplitter`, `AdjustmentPunctuationMatcher`, `comparePunctuation(2Side)` and the three whitespace correctors.
7. `ByCharRt` (code point diff, `compareTwoStep`, the IW variant) for BY_CHAR and as a helper.
8. `createInnerFragments` with `MAX_BAD_LINES`, `dropWholeChangedFragments`, `processBlocks` (squash/trim) and type derivation.
9. Offsets: IntelliJ uses UTF-16 indices. In Rust we should use byte offsets in UTF-8 and iterate over `char`. Watch out that `countChars`/`charOffsetAfter` in Kotlin count UTF-16 units.
10. Cancellation (`CancellationChecker`) and the "too big" error as its own `Result` variant.

Proposed Rust pipeline:

```
fn compare_lines_inner(t1, t2, policy, inner: InnerPolicy) -> Result<Vec<LineFragment>, TooBig>:
    lines = split('\n')
    ch = if policy == Ignore { smart(lines_iw) |> line_opt |> expand_ranges }
         else { smart(lines_iw) |> line_opt(lines_policy) |> second_step(lines_policy) }
    frags = to_line_fragments(ch)
    if inner != None { frags = inner_fragments(frags, policy) }   # by_word::compare_and_split
    process_blocks(frags, squash, trim)
```
