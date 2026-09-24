# smudiff

GPU-akselerert diff-viewer med JetBrains-inspirert visning. Skrevet i Rust med GPUI. macOS først, Linux etter hvert.

Status: diff-kjernen porteres fra IntelliJ og testes mot IntelliJ sin egen kode. GUI-en viser én fil om gangen.

## Krav

- Rust (edition 2024, stable)
- For golden-generatoren: JDK 25 og Gradle 9.x

## Kjøre

```
cargo run --release -p smudiff -- <venstre> <høyre> [--light]
```

Uendrede linjer er foldet, med 4 linjer kontekst. Klikk på en fold for å vise mer.

### Taster

Et lite, vim-inspirert keymap for lesing. Tastelinja nederst viser de viktigste.

| Tast | Gjør |
| --- | --- |
| `h` `j` `k` `l`, piltaster | Flytt cursoren. `j`/`k` hopper over lukkede folds |
| `0` `$` | Linjestart og linjeslutt |
| `gg` `G` | Første og siste linje |
| `d` `u`, Ctrl+d Ctrl+u | Halv side ned og opp |
| `n` `p`, Ctrl+n Ctrl+p, F7 Shift+F7 | Neste og forrige endring |
| Tab | Bytt side |
| `o` | Åpne fold ved cursoren ett nivå |
| `f` | Åpne alle folds, eller lukk alle hvis ingen er lukket |
| Cmd+Pluss Cmd+Minus Cmd+0 | Zoom inn, ut og tilbake til 13pt |
| Cmd+Alt+C | Bytt kontekst: 1, 2, 4, 8 linjer eller ingen folding |
| `q`, Cmd+Q, Cmd+W | Avslutt |

JetBrains Mono brukes hvis den er installert eller finnes i en JetBrains-IDE under `/Applications`, ellers Menlo.

## Tester

```
cargo test
```

Kjører enhetstester og golden-testen i `crates/smudiff-core/tests/golden.rs`. Den sammenligner resultatet med `fixtures/*/expected.json`.

Mot et annet sett med fixturer:

```
SMUDIFF_FIXTURES=/sti/til/fixturer cargo test --test golden
```

## Fixturer

Hver case er en mappe `fixtures/<navn>/` med `a.txt` og `b.txt`. Fasiten `expected.json` lages av IntelliJ sin diff-kode:

```
tools/golden/gen.sh
```

Ny case: lag mappa med `a.txt` og `b.txt`, kjør `gen.sh`, og kjør `cargo test`. Format i `fixtures/README.md`, generatoren er beskrevet i `tools/golden/README.md`.

## Struktur

- `crates/smudiff-core`: diff-algoritmen, portert fra IntelliJ
- `fixtures/`: testcaser med fasit
- `tools/golden/`: Kotlin-generator som lager fasiten
- `docs/`: overordnet løsning og research
