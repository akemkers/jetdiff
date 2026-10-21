# Kodestandard

Hvordan koden i jetdiff er organisert. Målet er at en fil kan leses uten å kjenne resten, og at man finner fram ved å lese navnene.

## Små filer med ett ansvar

En fil gjør én ting. Samler en fil flere typer eller ansvar, deles den i en mappe med én fil per ansvar. Rust-vanen er større filer, men her deler vi tidlig, fordi store filer er vanskelige å få oversikt over og gir for lite abstraksjon.

En modul med undermoduler er en fil og en mappe med samme navn. Ikke bruk `mod.rs`.

```
code_viewer.rs        setter sammen: velger split eller unified og tegner
code_viewer/
  painter.rs          byggeklossene: linje, linjenummer, fyll, markert rad, skillelinje
  split.rs            to sider med skillelinje
  unified.rs          én linje med to linjenumre
  scrollbar.rs        tommel og markører
  change.rs           endringen en rad hører til
```

Grupper etter domene. Lag ikke filer som `utils.rs` eller `types.rs`.

## Inngangsfila setter sammen

Fila som har navnet til modulen, sier hva som skjer, og overlater detaljene til undermodulene. Den skal kunne leses som en oppskrift:

```rust
pub fn paint(state: &UnifiedState, focused: bool, area: Rect, theme: &Theme, buf: &mut Buffer) {
    let mut painter = Painter::new(state, theme, focused, buf);
    painter.background(area);
    let scrollbar = match state.diff.row_layout() {
        RowLayout::Split => split::paint(&mut painter, area),
        RowLayout::Unified => unified::paint(&mut painter, area),
    };
    scrollbar::paint(&mut painter, scrollbar, area);
}
```

Blander en funksjon hva som skjer med hvordan det gjøres, får stegene egne funksjoner med navn.

## Lite grensesnitt

Bare det resten av craten trenger er `pub`. `code_viewer` viser fram `paint` og `row_at`, og alt annet er privat.

Typer som undermodulene deler, får `pub(super)`. Da ser modulen og undermodulene dem, men ikke resten av craten.

## Tilstand bak metoder

Felt er private. Andre filer spør gjennom metoder i stedet for å gå inn i strukturen:

```rust
// Ikke slik: split.rs vet hvordan tilstanden ser ut
let state = painter.state;
let marked = index == state.row;

// Slik: painter.rs er den eneste som vet det
for visible in painter.visible_rows(area) {
    if visible.marked { ... }
}
```

Da kan tilstanden endres ett sted. Prisen er noen metoder som stort sett sender videre, og det er verdt det.

## Navn

- Bruk domeneordene som allerede finnes: split og unified, ikke nye ord for det samme
- Ingen forkortelser. Bruk beskrivende navn: `viewport`, ikke `vp`, og `anchor_left`, ikke `al`. Det gjelder også parametre i closures: `|commits|`, ikke `|c|`. Unntakene er `e` for feil og `i` som løkketeller, som er vanlige i Rust
- Navnet sier hva noe er eller gjør. `Painter` tegner, mens `Painted` høres ut som noe som allerede er tegnet

## Ingen magiske tall

Et tall som betyr noe, får en konstant med navn. Si hvorfor i en kommentar når navnet ikke gjør det:

```rust
/// Den automatiske bredden tar aldri mer enn dette av vinduet, som i GPUI.
const MAX_AUTO_FRACTION: f32 = 0.4;

let max = (window_width as f32 * MAX_AUTO_FRACTION) as u16;
```

Konstanten står øverst i fila som bruker den. Brukes den av flere filer, står den i modulen over, som `WHEEL_ROWS` i `jetdiff-tui/src/lib.rs`. Unntaket er tall som forklarer seg selv der de står, som `0`, `+ 1` eller å dele på `2` for midten.

## Kommentarer

Skriv kode som forklarer seg selv. En kommentar forklarer hvorfor, ikke hva, og er kort. Ingen seksjons-skiller og ingen utkommentert kode.

Doc-kommentarer øverst i en fil sier hva fila har ansvar for.
