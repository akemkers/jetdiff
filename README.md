# smudiff

GPU-akselerert diff-viewer med JetBrains-inspirert visning. Skrevet i Rust med GPUI. macOS først, Linux etter hvert.

Status: diff-kjernen porteres fra IntelliJ og testes mot IntelliJ sin egen kode. GUI-en viser to filer, en commit eller lokale endringer, med et filtre som i commit-dialogen til IntelliJ.

## Krav

- Rust (edition 2024, stable)
- For golden-generatoren: JDK 25 og Gradle 9.x

## Kjøre

```
cargo run --release -p smudiff -- [<commit> | <venstre> <høyre>] [--light]
```

- Uten argumenter: lokale endringer mot HEAD, også nye filer git ikke kjenner
- `<commit>`: commiten mot forelderen, f.eks. `HEAD~2`
- `<venstre> <høyre>`: to filer

Commit og lokale endringer åpnes med et tre over endrede filer til venstre. Nye og slettede filer vises som én side. Binærfiler vises ikke.

Uendrede linjer er foldet, med 4 linjer kontekst. Klikk på en fold for å vise mer.

Syntaksfarger fra IntelliJ for Kotlin, Java, TypeScript/TSX/JavaScript, Rust, Go, JSON, YAML, TOML, XML og Markdown. Språket velges fra filendelsen, høyre side først.

Eksempler per språk ligger i `examples/`:

```
cargo run --release -p smudiff -- examples/kotlin/a.kt examples/kotlin/b.kt
```

### Taster

Et lite, vim-inspirert keymap for lesing. Det er ingen cursor, bare en markert linje. Tastelinja nederst viser de viktigste.

| Tast | Gjør |
| --- | --- |
| `J` `K`, Tab Shift+Tab | Neste og forrige fil i treet |
| `j` `k`, pil opp og ned | Flytt markert linje. Hopper over lukkede folds |
| `h` `l`, pil venstre og høyre | Scroll sideveis 4 tegn |
| `0` `$` | Scroll til linjestart og til slutten av markert linje |
| `gg` `G` | Første og siste linje |
| `d` `u`, Ctrl+d Ctrl+u | Halv side ned og opp |
| `n` `p`, Ctrl+n Ctrl+p, F7 Shift+F7 | Neste og forrige endring |
| `H` `L` | Til venstre og høyre side |
| `o` | Åpne fold ved markert linje ett nivå |
| `f` | Åpne alle folds, eller lukk alle hvis ingen er lukket |
| Cmd+Pluss Cmd+Minus Cmd+0 | Zoom inn, ut og tilbake til 16pt. Treet følger med |
| Cmd+Alt+C | Bytt kontekst: 1, 2, 4, 8 linjer eller ingen folding |
| `q`, Cmd+Q, Cmd+W | Avslutt |

Fonten er JetBrains Mono, bygd inn i appen (`crates/smudiff-ui/fonts/`, SIL Open Font License).

Ikonene i treet (`crates/smudiff-ui/icons/`) er fra intellij-community (Apache 2.0), og Rust-ikonet er fra intellij-rust (MIT).

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
- `crates/smudiff-syntax`: syntax highlighting med tree-sitter og egne queries
- `crates/smudiff-view`: view-modell uten GUI (scroll, folding, markert linje, geometri)
- `crates/smudiff-changes`: endrede filer og tremodellen
- `crates/smudiff-git`: endringer og filinnhold via `git`
- `crates/smudiff-ui`: GPUI-visningen
- `crates/smudiff`: binæren
- `examples/`: filpar å prøve viewen på, ett per språk
- `fixtures/`: testcaser med fasit
- `tools/golden/`: Kotlin-generator som lager fasiten
- `docs/`: overordnet løsning og research
