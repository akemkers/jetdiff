# 01 - IntelliJ sin diff-algoritme (reverse-engineered)

Kilde: sparse clone i `~/git/_ref/intellij-community`. Forkortelser for stier:

- `U/` = `platform/util/diff/src/com/intellij/diff/comparison/`
- `UD/` = `platform/util/diff/src/com/intellij/util/diff/`
- `F/` = `platform/util/diff/src/com/intellij/diff/fragments/`
- `I/` = `platform/diff-impl/src/com/intellij/diff/`

Alle linjenummer er fra filene slik de ligger i klonen.

Begreper brukt under:

- **DiffIterable**: liste av `Range(start1,end1,start2,end2)` for endringer over to sekvenser med lengde `length1/length2`. Det motsatte (unchanged) utledes.
- **Fair**: alle unchanged-ranges har lik lengde på begge sider (1:1-match). Alt i pipelinen er fair.
- **ChangeBuilder**: `markEqual(i1,i2[,end1,end2])` i stigende rekkefølge, hullene mellom blir changes (`U/iterables/DiffIterableUtil.kt:244-302`). `ExpandChangeBuilder` trimmer i tillegg like elementer i kantene av hver change (`:304-310`).
- **expand(a,b,range)**: skrell av like elementer forfra og bakfra i en range (`U/TrimUtil.kt:323-339`).

---

## 1. Pipeline: to tekster til `List<LineFragment>` med inner fragments

Inngang: `ComparisonManagerImpl.compareLinesInner` (`I/comparison/ComparisonManagerImpl.java:158-177`).

```
compareLinesInner(text1, text2, policy, fragmentsPolicy):
  if fragmentsPolicy == WORDS && Registry("diff.by.word.deprioritize.line.differences"):
      return compareLinesWordFirst(...)          # eksperimentell, se 4.6
  lineOffsets = split paa '\n' (LineOffsetsImpl.kt:48-53, siste linje kan vaere tom)
  lines = linjeinnhold UTEN '\n' (ComparisonManagerImpl.java:580-586)
  iterable  = ByLineRt.compare(lines1, lines2, policy)       # linjediff
  fragments = convertIntoLineFragments(iterable)             # :391-432, offsets INKL '\n'
  if fragmentsPolicy != NONE:
      fragments = createInnerFragments(fragments, ...)       # :217-312
  return fragments
# Etterpaa i viewer-laget: processBlocks(squash, trim)        (TwosideTextDiffProviderBase.java:74-81)
```

### 1.1 Linjediff: `ByLineRt.doCompare` (`U/ByLineRt.kt:61-82`)

Hver linje pakkes i `Line(content, policy)` med hash og equals etter policy og `nonSpaceChars` = antall tegn som ikke er `' '`, `'\t'`, `'\n'` (`:418-455`).

```
doCompare(lines1, lines2, policy):
  if policy == IGNORE_WHITESPACES:
      ch = compareSmart(lines1, lines2)                 # 2-stegs, se kap. 2
      ch = LineChunkOptimizer(lines1, lines2, ch)       # kap. 3
      return expandRanges(ch)                           # trim like linjer i kantene (:364-377)
  else:                                                 # DEFAULT og TRIM_WHITESPACES
      iw1, iw2 = samme linjer men med IGNORE_WHITESPACES-equals (:387-397)
      ch = compareSmart(iw1, iw2)                       # match IGNORERER whitespace
      ch = LineChunkOptimizer(lines1, lines2, ch)       # NB: bruker original-policy-equals
      return correctChangesSecondStep(lines1, lines2, ch)  # kap. 1.2
```

Poeng: selv i DEFAULT-modus matches linjer først whitespace-blindt. Det gjør at en re-indentert blokk fortsatt parres linje-for-linje, og ByWord kan etterpå vise bare whitespace-endringene.

### 1.2 `correctChangesSecondStep` (`U/ByLineRt.kt:135-266`)

Tar IW-matchingen og gjør den strikt (policy-equal). Kommentaren på `:140-166` er fasit:

```
builder = ExpandChangeBuilder(lines1, lines2)
sample = null            # "gruppe" av IW-like linjer som ikke er eksakt like
for (i1, i2) in alle IW-matchede par, i rekkefoelge:
    if !IW_equal(sample, lines1[i1]):
        flush(i1, i2)
        if lines1[i1] == lines2[i2]: builder.markEqual(i1, i2)
        else:                        sample = lines1[i1]
flush(len1, len2)

flush(end1, end2):
    if sample == null: return
    sub1 = [i in max(last1, builder.index1) .. end1 der IW_equal(sample, lines1[i])]
    sub2 = tilsvarende paa side 2
    alignExactMatching(sub1, sub2); sample = null

alignExactMatching(sub1, sub2):
    if max(|sub1|,|sub2|) > 10 or |sub1| == |sub2|:
        zip parvis, markEqual der lines1 == lines2          # :226-240
    else:
        brute force alle C(n,k) kombinasjoner, velg den med flest eksakt like par
        (getBestMatchingAlignment :268-319), markEqual de like
```

Effekt: blant flere IW-like kandidater velges den eksakt like som partner. Ikke-eksakte par blir change (og får ByWord-highlight av whitespace).

### 1.3 LCS-motoren: `Diff.buildChanges` (`UD/Diff.kt:26-102`)

Alle nivåer (linjer, ord, tegn) går via `DiffIterableUtil.diff` (`U/iterables/DiffIterableUtil.kt:26-69`) til `Diff.buildChanges`.

```
buildChanges(objs1, objs2):
  pre  = felles prefix; post = felles suffix                  # :27-28, :104-144
  if en av de trimmede sidene er tom: return en enkelt change  # doBuildChangesFast :55-66
  ints = Enumerator: objekt -> int-id via hashCode/equals     # :34-36 (interning)
  doBuildChanges(ints1, ints2):
    d1, d2 = Reindexer.discardUnique(ints1, ints2)            # :72-73
    if d1 og d2 tomme: alt er en change                        # :75-79
    if DiffConfig.USE_PATIENCE_ALG (=false):  Patience
    else:
      try:   MyersLCS(d1,d2).executeWithThreshold()           # :88-91
      catch FilesTooBig: PatienceIntLCS(d1,d2).execute(failOnSmallReduction=true)  # :93-97
    Reindexer.reindex(changes, builder)                       # map tilbake til orig. indekser
```

**Reindexer** (`UD/Reindexer.kt:15-18, 37-52, 54-117`): fjerner elementer som ikke finnes på den andre siden i det hele tatt (de kan aldri matches). `discard1 = ints1 ∩ set(ints2)`, `discard2 = ints2 ∩ set(discard1)`. Myers kjøres på de komprimerte sekvensene, og `reindex` ekspanderer BitSet-ene tilbake: forkastede elementer blir markert som endret. Samme idé som GNU diff sin "discard confusing lines", men enklere (kun helt unike).

**MyersLCS** (`UD/MyersLCS.kt`): klassisk lineær-minne Myers med middle snake og rekursjon (`:90-193`). Egenskaper:

- `executeWithThreshold`: maks D = `max(20000 + 10*sqrt(N1+N2), DELTA_THRESHOLD_SIZE=20000)` (`:76-80`, `UD/DiffConfig.kt:10`). Overskrides D kastes `FilesTooBigForDiffException` (`:188-191`).
- `executeLinear`: samme terskel men kaster ikke. Overskrides den, forblir hele delområdet markert som endret (bitsene er satt i `init`, `:41-42`).
- Når `td` (funnet D) er 0 eller 1 går den grådig: match felles run, ellers hopp fram på den lengste siden (`:170-187`). Det bestemmer tiebreak for enkeltinnsettinger.

**PatienceIntLCS** (`UD/PatienceIntLCS.kt:38-123`), kun fallback:

```
execute(range, counter):          # counter starter paa 2 naar failOnSmallReduction
  trim felles prefix/suffix
  if en side tom: marker change; return
  if counter == 0: checkReduction          # kast hvis delproblemet fortsatt er >= halvparten av hele
  counter = max(-1, counter-1)
  anchors = UniqueLCS(range)               # elementer unike paa BEGGE sider, LIS paa posisjon
  if anchors == null:
      if counter >= 0: checkReduction
      MyersLCS(range).executeLinear()
  else:
      rekurser paa hvert gap mellom ankere (og foer/etter)
```

`checkReduction` (`:154-158`) kaster -> `DiffTooBigException` -> viewer viser "diff too big" (`I/tools/simple/SimpleDiffViewer.java:277-278`).

**UniqueLCS** (`UD/UniqueLCS.kt:23-95`): hashmap for elementer som forekommer nøyaktig en gang på hver side, så patience sorting (LIS med binærsøk og predecessor-array).

### 1.4 Størrelsesgrenser og fallbacks, samlet

| Grense | Verdi | Sted | Konsekvens |
|---|---|---|---|
| Myers D-terskel | `max(20000+10*sqrt(N), 20000)` | `UD/MyersLCS.kt:77` | fallback til Patience |
| Patience reduksjon | delproblem >= 50 % begge sider etter 2 nivåer | `UD/PatienceIntLCS.kt:67,74,154` | `DiffTooBigException`, hele diffen avbrytes |
| Myers inne i Patience | samme terskel, kaster ikke | `UD/MyersLCS.kt:56-64` | område blir en stor change |
| Bad inner lines | `MAX_BAD_LINES = 3` | `UD/DiffConfig.kt:11`, `I/comparison/ComparisonManagerImpl.java:225-237` | etter 3 feilede ByWord-forsøk hoppes inner over for resten |
| Brute-force align | n > 10 | `U/ByLineRt.kt:227` | enkel zip i stedet |
| Unimportant line | `nonSpaceChars <= 3` | `UD/DiffConfig.kt:12` | kap. 2 og 3 |

---

## 2. "Unimportant" linjer: to-stegs sammenligning

`compareSmart` (`U/ByLineRt.kt:335-362`) + `SmartLineChangeCorrector` (`U/ChangeCorrector.kt:93-126`, basisklasse `:14-57`).

```
compareSmart(lines1, lines2):
  T = 3                                           # getUnimportantLineCharCount
  big1 = [(i, l) for l in lines1 if l.nonSpaceChars > T]    # f.eks. ikke "}", "", "  );"
  big2 = tilsvarende
  ch = diff(big1.lines, big2.lines)               # steg 1: kun "viktige" linjer
  # steg 2: ChangeCorrector
  last1 = last2 = 0
  for (b1, b2) in matchede par i ch:
      o1, o2 = big1.index[b1], big2.index[b2]     # original linjeindeks
      matchGap(last1, o1, last2, o2)
      builder.markEqual(o1, o2)
      last1, last2 = o1+1, o2+1
  matchGap(last1, len1, last2, len2)

matchGap(s1,e1,s2,e2):                             # ChangeCorrector.kt:101-115
  r = expand(lines, s1..e1, s2..e2)                # skrell like linjer i kantene
  inner = diff(lines1[r.s1..r.e1], lines2[r.s2..r.e2])   # full diff paa ALLE linjer i gapet
  markEqual kanter + alle unchanged i inner
```

Hvorfor: linjer som `}`, tomme linjer og `*/` matcher overalt og drar LCS-en i feil retning. Ved å ankre på de viktige linjene først, og så bare fylle gapene, havner små linjer bare i match innenfor riktig område.

Samme mønster finnes på tegn-nivå: `ByCharRt.compareTwoStep` differ ikke-whitespace kodepunkter først, og fyller gapene med full tegndiff via `DefaultCharChangeCorrector` (`U/ByCharRt.kt:52-64`, `U/ChangeCorrector.kt:62-91`).

---

## 3. ChunkOptimizer: flytting av chunk-grenser ("slider"-heuristikk)

Generisk del (`U/ChunkOptimizer.kt:11-85`). Den jobber på **unchanged**-ranges. To naborange `r1, r2` der endringen mellom dem er en ren insert eller delete (en side "berører", dvs. `r1.end == r2.start` på den siden).

```
build():
  ranges = []
  for r in iterable.unchanged: ranges.push(r); processLast()
  return fair(createUnchanged(ranges))

processLast():                                           # :28-77
  if |ranges| < 2: return
  r1, r2 = to siste
  if r1.end1 != r2.start1 and r1.end2 != r2.start2: return   # modified-chunk, ikke flyttbar
  count1 = len(r1); count2 = len(r2)
  eqF = expandForward(data, fra r1.end,           max count2)   # hvor langt kan gapet gli ned
  eqB = expandBackward(data, bakover fra r2.start, max count1)  # hvor langt kan det gli opp
  if eqF == 0 and eqB == 0: return
  if eqF == count2:   # [A]B[B] -> [AB]B : gli helt ned, r2 slukes, merge med neste change
      erstatt r1,r2 med Range(r1.start, r1.end+count2); processLast(); return
  if eqB == count1:   # [A]A[B] -> A[AB] : gli helt opp, r1 slukes
      erstatt r1,r2 med Range(r2.start-count1, r2.end); processLast(); return
  touch = LEFT if r1.end1 == r2.start1 else RIGHT    # siden UTEN innsatte linjer
  shift = getShift(touch, eqF, eqB, r1, r2)          # >0 ned, <0 opp
  if shift != 0: r1.end += shift; r2.start += shift
```

Regel 1 (felles for linjer og ord): minimer antall chunks. Kan en insert gli slik at den treffer en nabochange, gjøres det.

### 3.1 `LineChunkOptimizer.getShift` (`U/ChunkOptimizer.kt:174-260`)

```
getShift(touch, eqF, eqB, r1, r2):
  for (fn, T) in [(unchangedBoundary, 0), (changedBoundary, 0),
                  (unchangedBoundary, 3), (changedBoundary, 3)]:
      s = fn(touch, eqF, eqB, r1, r2, T)
      if s != null: return s
  return 0

unimportant(line, T) = line.nonSpaceChars <= T     # T=0: blank/whitespace-only, T=3: "}", "});" osv.

unchangedBoundary(...):              # :203-216  "insert rett foer/etter en tom linje"
  L = linjer paa touch-siden; p = r2.start paa touch-siden (innsettingspunkt)
  fwd = foerste i in 0..eqF   der unimportant(L[p+i])     ellers -1
  bwd = foerste i in 0..eqB   der unimportant(L[p-1-i])   ellers -1
  return pick(fwd, bwd)

changedBoundary(...):                # :222-237  "insert starter/slutter med en tom linje"
  L = linjer paa den andre siden (den med innsatte linjer)
  cs = r1.end, ce = r2.start (paa den siden) = innsatt blokk
  fwd = foerste i in 0..eqF   der unimportant(L[cs+i])    ellers -1
  bwd = foerste i in 0..eqB   der unimportant(L[ce-1-i])  ellers -1
  return pick(fwd, bwd)

pick(fwd, bwd):                      # :254-259
  if fwd == -1 and bwd == -1: return null      # proev neste regel
  if fwd == 0 or bwd == 0:    return 0         # allerede paa god grense, ikke flytt
  return fwd if fwd != -1 else -bwd            # foretrekk aa flytte NED
```

Oppsummert prioritet: (1) blank linje i unchanged-kontekst rett ved innsettingen, (2) innsatt blokk som starter/slutter med blank linje, (3)-(4) samme med "nesten tomme" linjer (<= 3 tegn). Nedover foretrekkes. **Indentasjon brukes ikke** (ingen git-style indent heuristic).

### 3.2 `WordChunkOptimizer.getShift` (`U/ChunkOptimizer.kt:100-163`)

Regel 2: minimer antall "setninger" som endres, der en setning er ord som ikke er skilt av whitespace (eks. `1.0.123 1.0.155` vs `1.0.123 1.0.134 1.0.155`).

```
getShift(touch, eqF, eqB, r1, r2):
  W = ord paa touch-siden; T = tekst paa touch-siden; p = r2.start paa touch-siden
  if sepWS(T, W[p-1], W[p]): return 0                         # allerede paa whitespace-grense
  for i in 0..eqF-1: if sepWS(T, W[p+i], W[p+i+1]):   return i+1     # skift ned
  for i in 0..eqB-1: if sepWS(T, W[p-1-i-1], W[p-1-i]): return -(i+1) # skift opp
  return 0

sepWS(T, a, b) = a eller b er NewlineChunk
                 or finnes ' ', '\t', '\n' i T[a.end .. b.start]      # :151-161
```

---

## 4. Ordnivå (ByWord)

### 4.1 Tokenisering: `getInlineChunks` (`U/ByWordRt.kt:637-683`)

Iterer på kodepunkter:

```
isPunctuation(c) = ASCII 33-47, 58-64, 91-96, 123-126, UNNTATT '_'   # U/TrimUtil.kt:16-22
isWhiteSpace(c)  = c in {' ', '\t', '\n'}                           # :29-31
isAlpha(c)       = !isWhiteSpace(c) && !isPunctuation(c)            # :24-27  (NB: '\r' er "alpha")
isContinuousScript(c):                                              # :33-46
    c < 128 -> false; desimalsiffer -> false; ikke-BMP -> true
    ideografisk -> true; ikke alfabetisk -> true   # f.eks. unicode-symboler og ikke-ASCII tegnsetting
    ellers Hiragana/Katakana/Thai/Javanesisk -> true

for hvert kodepunkt c:
  if isAlpha(c) && !isContinuousScript(c): legg til i paagaaende ord (hash = hash*31 + c)
  else:
    avslutt paagaaende ord -> WordChunk
    if isAlpha(c):   WordChunk for c alene            # CJK o.l.: hvert tegn er et ord
    elif c == '\n':  NewlineChunk
    # whitespace og ASCII-tegnsetting blir IKKE tokens, de lever i gapene
```

`WordChunk.equals` = eksakt tekst (DEFAULT) uansett policy (`:1098-1107`). Alle `NewlineChunk` er like (`:1118-1127`).

### 4.2 `compareAndSplit` (`U/ByWordRt.kt:89-134`), brukt for inner fragments

```
compareAndSplit(t1, t2, policy):
  w1, w2 = getInlineChunks(t1), getInlineChunks(t2)
  ch = diff(w1, w2)                                  # samme LCS-motor
  ch = WordChunkOptimizer(w1, w2, t1, t2, ch)        # 3.2
  blocks = LineFragmentSplitter(t1, t2, w1, w2, ch).run()   # 4.4
  for block in blocks:
      sub = ch begrenset til blokken
      it  = AdjustmentPunctuationMatcher(sub)        # 4.3, gaar fra ord-indekser til tegn-offsets
      it  = matchAdjustmentWhitespaces(it, policy)   # 4.3
      fragments = it.changes som DiffFragment (relativ til blokkstart)
      emit LineBlock(fragments, offsets, antall '\n' paa hver side)
```

### 4.3 Tegnsetting og whitespace i gapene

**AdjustmentPunctuationMatcher** (`U/ByWordRt.kt:695-874`): matchede ord markeres equal på tegn-offsets. Gapene mellom ord (whitespace + tegnsetting) sammenlignes med `ByCharRt.comparePunctuation` (`U/ByCharRt.kt:95-134`), som kun LCS-er ASCII-tegnsettingstegn og lar alt annet være umatchet.

```
for hvert matchet ordpar (i1, i2):
    matchBackward(gap FOER i1/i2); markEqual(ordet); matchForward(husk gap ETTER i1/i2)
matchBackward(gap foer neste match) mot husket gap etter forrige match:
  samme gap paa begge sider ("A B" vs "A B")      -> comparePunctuation(gap1, gap2)
  umatchede ord paa begge sider ("A X B"/"A Y B") -> sammenlign etter-gap og foer-gap hver for seg
  umatchede ord bare paa en side ("A B"/"A Y B")  -> ett gap mot konkatenering av to gap
                                                     (comparePunctuation2Side, :550-583)
```

Eksempel i koden: `"[ X { A ! B } Y ]"` vs `"( X ... Y )"` gir gruppene `[`/`(`, `{`+`}`/`...`, `]`/`)` (`:689-694`).

**Whitespace per policy** (`U/ByWordRt.kt:488-507`). Etter punktum-matching står all whitespace i gapene fortsatt som "changed":

```
DEFAULT  (DefaultCorrector :880-906):
  for change: skrell LIKE whitespace-tegn fra slutten, saa fra starten
  (expandWhitespacesBackward/Forward, TrimUtil.kt:126-148); dropp tomme
  => "a b" vs "a b": ingen fragment.  "a  b" vs "a b": delete av ett mellomrom.

TRIM_WHITESPACES (DefaultCorrector, saa TrimSpacesCorrector :989-1028):
  trim kant-whitespace KUN hvis den er leading/trailing paa linja (isLeadingTrailingSpace :590-623)
  dropp hvis resten er eksakt lik

IGNORE_WHITESPACES (IgnoreSpacesCorrector :939-962):
  expandWhitespaces (match like whitespace), trim ALL whitespace i kantene,
  dropp hvis IW-lik
```

### 4.4 LineFragmentSplitter (`U/LineFragmentSplitter.kt`)

Deler en flerlinjes line-fragment i "logisk ulike" blokker basert på ord-matchingen. Indekser: `-1` = start, `words.size` = slutt.

```
run():                                                    # :29-58
  hasEqual = false; last = (-1,-1); pending = null
  for (i1, i2) in matchede ordpar:
      if w1[i1] og w2[i2] er begge NewlineChunk:
          addChunk(i1, i2, hasEqual); hasEqual = false          # splitt ETTER matchet '\n'
      else:
          if begge er foerste ord paa sin linje:
              addChunk(i1-1, i2-1, hasEqual); hasEqual = false  # splitt FOER matchet linjestart
          hasEqual = true
  addChunk(len1, len2, hasEqual); flush pending

addChunk(e1, e2, hasEqual):                              # :60-80
  chunk = blokk fra last til e (offsets = rett etter '\n'), med flagg:
          hasEqualWords, hasWordsInside (noe annet enn '\n'), isEqualIW
  if pending og shouldMerge(pending, chunk): pending = merge
  else: emit pending; pending = chunk

shouldMerge(a, b):                                        # :130-138
  !a.hasEqual && !b.hasEqual        -> true   # linjer som kun matchet paa '\n' slaas sammen
  a.isEqualIW && b.isEqualIW        -> true   # rene whitespace-endringer slaas sammen
  !a.hasWords || !b.hasWords        -> true   # blokk uten ord (tomme linjer) squashes
  ellers false
```

### 4.5 Fra blokker til LineFragments, og fallback til hel-linje (`I/comparison/ComparisonManagerImpl.java:217-312`)

```
createInnerFragments(fragments):
  bad = 0
  for f in fragments:
    try:
      if f er ren insert/delete (startLine==endLine paa en side):          # :257-265
          if isEqualTexts(sub1, sub2, policy): f.inner = []   # "ignorert" endring
          else:                                f.inner = null  # hel-linje
      elif bad >= 3: f uendret (inner = null)                               # :231, :267
      else:
          blocks = ByWord.compareAndSplit(sub1, sub2, policy)   # sub = tekst INKL '\n'
          for block i: emit LineFragment(linjer fra newline-tellinger,
                                         siste blokk faar f.endLine, :299-301,
                                         inner = block.fragments)
    catch DiffTooBig: behold f; bad++
```

Hel-linje-highlight skjer når:

- `innerFragments == null`: insert/delete, for mange feil, DiffTooBig, eller
- `LineFragmentImpl` får nøyaktig ett inner fragment som dekker hele begge sider, og setter det til `null` (`F/LineFragmentImpl.kt:18-19, 72-83`).

`innerFragments == []` (tom liste) betyr: endret linje, men ingen synlige forskjeller under policy (f.eks. bare whitespace i IGNORE-modus).

### 4.6 `compareWordsFirst` (eksperimentell, `U/ByWordRt.kt:176-447`)

Styrt av registry-nøkkel `diff.by.word.deprioritize.line.differences` (default ikke i klonen, antatt av). Differ kun `WordChunk` (uten newlines) på hele teksten, og løfter resultatet til linjeblokker via `LineUplifter`. Nyttig for Java->Kotlin eller tekst med ulik linjebryting. Kan vente.

---

## 5. ComparisonPolicy og UI-opsjoner

`ComparisonPolicy { DEFAULT, TRIM_WHITESPACES, IGNORE_WHITESPACES }` (`U/ComparisonPolicy.kt:4-8`). Equals/hash (`U/ComparisonUtil.kt:14-37`):

- DEFAULT: eksakt.
- TRIM: ignorer leading/trailing `' ' '\t' '\n'`. For flerlinjetekst trimmes hver linje (`equalsTrimWhitespaces :58-86`).
- IGNORE: ignorer all `' ' '\t' '\n'` overalt.

(Selve `equalsIgnoreWhitespaces`-implementasjonen ligger ikke i sparse-klonen. `TrimUtil.kt:583` sier at whitespace-definisjonen er "from Strings.isWhiteSpace", altså `' ' '\t' '\n'`.)

| Steg | DEFAULT | TRIM | IGNORE |
|---|---|---|---|
| Linje-LCS | IW-linjer (`ByLineRt.kt:75-78`) | IW-linjer | IW-linjer |
| Etterbehandling linjer | `correctChangesSecondStep` | `correctChangesSecondStep` (med TRIM-equals) | `expandRanges` |
| Ord-LCS | eksakte ord | eksakte ord | eksakte ord |
| Whitespace i ord-gap | `DefaultCorrector` | Default + `TrimSpacesCorrector` | `IgnoreSpacesCorrector` |
| Char-modus | `compareTwoStep` | `compareTrimWhitespaces` | `compareIgnoreWhitespaces` (`ComparisonManagerImpl.java:323-339`) |

UI-nivå `IgnorePolicy` (`I/tools/util/base/IgnorePolicy.java:11-43`):

| IgnorePolicy | ComparisonPolicy | squash | trimChunks |
|---|---|---|---|
| DEFAULT | DEFAULT | ja | nei |
| TRIM_WHITESPACES | TRIM | ja | nei |
| IGNORE_WHITESPACES | IGNORE | ja | nei |
| IGNORE_WHITESPACES_CHUNKS ("and empty lines") | IGNORE | ja | **ja** |
| FORMATTING | DEFAULT + ignored ranges fra språk-plugin (`compareLinesWithIgnoredRanges`, `ComparisonManagerImpl.java:623-650`) | ja | nei |

`HighlightPolicy` (`I/tools/util/base/HighlightPolicy.java`): BY_LINE (inner NONE), BY_WORD (WORDS, squash), BY_WORD_SPLIT (WORDS, **ikke** squash), BY_CHAR (CHARS), DO_NOT_HIGHLIGHT.

Squash = `highlight.isShouldSquash && ignore.isShouldSquash` (`I/tools/util/text/TwosideTextDiffProviderBase.java:75-76`).

### 5.1 `processBlocks` squash/trim (`I/comparison/ComparisonManagerImpl.java:448-544`)

```
processBlocks(fragments, squash, trim):
  for gruppe av tilstoetende fragments (endLine == neste startLine og endOffset == neste startOffset, :546-555):
    if trim && policy == IGNORE:
        dropp fra start/slutt av gruppa fragments som er IW-like eller har inner == []
    if squash: slaa gruppa sammen til ett LineFragment:
        inner = konkatenering av alle inner (inner == null -> ett fragment over hele), forskjoevet
                til ny start; tilstoetende inner-fragments merges (:513-544)
```

Viktig: LineFragmentSplitter deler en endring i flere fragments, men i standardmodus (BY_WORD) limes de sammen igjen av squash. Splittingen påvirker likevel resultatet. Hver blokk får sin egen punktum/whitespace-matching, og en blokk uten inner (hel-linje) blir ett stort inner fragment etter squash. BY_WORD_SPLIT viser blokkene som separate changes.

---

## 6. Datamodell

```
DiffFragment {                         # F/DiffFragment.kt:10-17
  startOffset1, endOffset1, startOffset2, endOffset2   # tegn-offsets (UTF-16 i Java)
}                                      # ikke tom, start <= end (F/DiffFragmentImpl.kt:12-14)

LineFragment : DiffFragment {          # F/LineFragment.kt:12-27
  startLine1, endLine1, startLine2, endLine2   # halvaapne linjeintervaller
  offsets dekker hele linjer INKL '\n' (kan mangle paa siste linje)
  innerFragments: List<DiffFragment>?  # relativ til fragmentets startOffset
                                       # null = hel-linje, [] = ingen synlig forskjell
}
```

Offsets for tom side (`ComparisonManagerImpl.java:416-432`): `start == end` gir `lineStart(start)`, eller slutten av teksten hvis `start == lineCount`.

Type (`I/util/DiffUtil.java:1549-1581`):

```
getLineDiffType(f) = type(f.startLine1 != f.endLine1, f.startLine2 != f.endLine2)
getDiffType(inner) = type(endOffset1 != startOffset1, endOffset2 != startOffset2)
type(del, ins) = MODIFIED if del && ins; DELETED if del; INSERTED if ins
```

Samme regel brukes for linje- og inner-nivå. En inner-endring inni en MODIFIED-linje kan altså være INSERTED eller DELETED og farges deretter.

---

## 7. `imara-diff` som base?

Lokal kopi: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/imara-diff-0.2.0/`.

Hva den gir:

- `Algorithm::{Histogram, Myers, MyersMinimal}` (`src/lib.rs:170-218`), prefix/suffix-stripping (`:268-271`), Myers med git/gnu-heuristikker og forkasting av tokens som ikke finnes på andre siden (tilsvarer Reindexer).
- `InternedInput<T: Hash+Eq>` / `TokenSource` (`src/intern.rs:32-64`): vi kan interne vår egen `Line`-type med policy-avhengig `Hash`/`Eq`, og ord/kodepunkter.
- Resultat som `removed/added: Vec<bool>` pluss `hunks()`, altså samme form som IntelliJ sine BitSets.
- Slider-postprocessing med git indent-heuristikk (`postprocess_lines`, `SliderHeuristic`-trait).

Vurdering:

- **Fornuftig som LCS-primitiv**, men ikke bit-for-bit lik IntelliJ. Myers-tiebreak, terskel (`MAX_COST_MIN=256`, `sqrt`-basert, `src/myers.rs:32,61`) og at den aldri feiler gir andre valg enn IntelliJ sin "eksakt Myers opp til D=20000, ellers Patience". Histogram gir enda mer avvikende resultater og skal ikke brukes på tegn.
- IntelliJ sin `MyersLCS` + `Reindexer` + `PatienceIntLCS` + `UniqueLCS` er til sammen ca. 650 linjer Kotlin. Å porte dem direkte er billig og gir mulighet for golden-tester mot IntelliJ. Anbefaling: definer et `Lcs`-trait, port IntelliJ-motoren som referanse, og hold imara-diff `Myers` som alternativ backend for store filer eller benchmarking.
- **Ikke** bruk imara sin `postprocess_lines`. IntelliJ-sliderne kommer fra `ChunkOptimizer` og må kjøres på rå LCS-output.

Må skrives selv (uansett backend):

1. `Line` med policy-hash/eq og `nonSpaceChars`; linjesplitting på `'\n'`.
2. `compareSmart` + `SmartLineChangeCorrector` (kap. 2).
3. IW-først-strategien og `correctChangesSecondStep` med brute-force-alignment (kap. 1.2).
4. Generisk `ChunkOptimizer` + `LineChunkOptimizer` + `WordChunkOptimizer` (kap. 3).
5. `getInlineChunks` med unicode-klassifisering (`isContinuousScript` trenger general category + script-tabeller, f.eks. `unicode-general-category`/`unicode-script`-crates).
6. `LineFragmentSplitter`, `AdjustmentPunctuationMatcher`, `comparePunctuation(2Side)` og de tre whitespace-korrektorene.
7. `ByCharRt` (kodepunktdiff, `compareTwoStep`, IW-varianten) for BY_CHAR og som hjelper.
8. `createInnerFragments` med `MAX_BAD_LINES`, `dropWholeChangedFragments`, `processBlocks` (squash/trim) og typeutledning.
9. Offsets: IntelliJ bruker UTF-16-indekser. I Rust bør vi bruke byte-offsets i UTF-8 og iterere på `char`. Se opp for at `countChars`/`charOffsetAfter` i Kotlin teller UTF-16-enheter.
10. Avbrudd (`CancellationChecker`) og "too big"-feil som egen `Result`-variant.

Forslag til Rust-pipeline:

```
fn compare_lines_inner(t1, t2, policy, inner: InnerPolicy) -> Result<Vec<LineFragment>, TooBig>:
    lines = split('\n')
    ch = if policy == Ignore { smart(lines_iw) |> line_opt |> expand_ranges }
         else { smart(lines_iw) |> line_opt(lines_policy) |> second_step(lines_policy) }
    frags = to_line_fragments(ch)
    if inner != None { frags = inner_fragments(frags, policy) }   # by_word::compare_and_split
    process_blocks(frags, squash, trim)
```
