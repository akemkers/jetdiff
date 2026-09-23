# smudiff

GPU-akselerert diff-viewer med JetBrains-inspirert visning. Skrevet i Rust med GPUI. macOS først, Linux etter hvert.

Status: diff-kjernen porteres fra IntelliJ og testes mot IntelliJ sin egen kode. GUI finnes foreløpig bare som spike.

## Krav

- Rust (edition 2024, stable)
- For golden-generatoren: JDK 25 og Gradle 9.x

## Kjøre spiken

Viser to filer side om side med trakt mellom:

```
cd spikes/gpui-trakt
cargo run --release -- testdata/a.java testdata/b.java
```

Spiken bruker `imara-diff` og en enklere visning. Den erstattes av `smudiff-core` og en ordentlig UI senere.

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
- `spikes/gpui-trakt/`: GPUI-prototype av side-by-side med trakt
- `docs/`: overordnet løsning og research
