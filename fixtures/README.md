# Golden-fixtures

Fasit fra IntelliJ sin diff-algoritme, generert av `tools/golden`. Rust-testene i `smudiff-core` sammenligner mot disse.

Hver case er en mappe:

```
fixtures/<case>/a.txt          venstre
fixtures/<case>/b.txt          høyre
fixtures/<case>/expected.json  generert, ikke rediger for hånd
```

## expected.json

```json
{
  "source": "intellij-community@<commit>",
  "policies": {
    "DEFAULT":            { "lines": [...], "words": [...], "viewer": [...] },
    "TRIM_WHITESPACES":   { ... },
    "IGNORE_WHITESPACES": { ... }
  }
}
```

- `lines`: linjediff uten inner fragments (`compareLines`).
- `words`: med inner fragments på ordnivå (`compareLinesInner`, WORDS).
- `viewer`: det side-by-side-vieweren viser i standardmodus (BY_WORD), etter `processBlocks`.

Et fragment:

```json
{ "l1": [0, 2], "l2": [0, 3], "o1": [0, 40], "o2": [0, 55], "inner": null }
```

- `l1`/`l2`: linjer, halvåpne intervaller, 0-basert.
- `o1`/`o2`: offsets i hele teksten, UTF-8 bytes, halvåpne.
- `inner`: `null` betyr hel-linje-highlight. `[]` betyr endret, men ingenting synlig under policyen. Ellers en liste `{ "o1": [s, e], "o2": [s, e] }` med offsets relativt til fragmentets start, UTF-8 bytes.

Generer på nytt: `tools/golden/gen.sh`.
