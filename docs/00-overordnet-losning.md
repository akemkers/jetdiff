# 00 - Overordnet løsning for smudiff

GPU-akselerert diffviewer i Rust + GPUI, modellert etter JetBrains. Detaljene ligger i `research/`:

- `01-diff-algoritme.md` - pipeline, heuristikker, ord-diff
- `02-rendering.md` - trakt, farger, gutter, folding-bølger
- `03-scroll-folding-struktur.md` - sync scroll, folding, commit-view, tre

## Rammer

- Kun visning (read-only). Ingen chevroner, ingen redigering.
- macOS først, Linux senere. Ingen Windows.
- Ingen 3-veis merge.
- Blir på sikt en modul i et større verktøy (prosjektvelger, worktrees). Derfor er diff-viewen en egen crate uten app-antakelser.

## Crates

```
smudiff-core     ren logikk, ingen GUI. Testes med golden-tester.
  lcs            Myers + Reindexer + patience-fallback (port fra IntelliJ)
  by_line        whitespace-blind match, to-stegs smart diff, ChunkOptimizer
  by_word        tokenizer, inner fragments, LineFragmentSplitter

smudiff-syntax   tree-sitter med egne queries, tokens tilsvarer IntelliJ sine DEFAULT_*

smudiff-view     view-modell, fortsatt uten GUI
  model          FileDiff: blokker, linjer, syntaks per side
  state          cursor, kommandoer, scroll og folding samlet
  scroll         master scroll_y, slave beregnes i samme frame (1/3-anker)
  folding        kontekstnivåer, felles tilstand for begge sider
  geometry       gutter, tekst og scrollbar per side, én side alene for nye og slettede filer
  keys           tastesekvenser til kommandoer

smudiff-changes  FileChange { path, old_path, status } og tremodellen: mapper først,
                 slåtte enkeltbarn-mapper, ekspandert-tilstand

smudiff-git      git-CLI: endringer for en commit eller working tree, og filinnhold

smudiff-ui       GPUI
  side_editor    tekst, gutter, bakgrunner, ord-markering, error stripe
  divider        bezier-trakt og fold-koblinger
  file_diff      to editorer + divider
  changes_tree   filtreet med ikoner og status-farger
  changes_view   treet + én fil, som commit-dialogen i IntelliJ
  theme          fargetabell fra JetBrains (light og dark)

smudiff          binær: CLI
```

Moduler kjenner ikke hverandre. Delt tilstand ligger i små GPUI-entiteter som alle observerer: `Selection` (valgt fil), og etter hvert policy og kontekstnivå. Treet skriver til `Selection` ved klikk, og diff-viewen følger den. `J`/`K` i diffen sender et event som treet flytter valget for. Skallet lager entitetene og gir dem til modulene. Kjernen får aldri stier eller git.

Skillet mellom `view` og `ui` er det viktigste. All logikk for scrolling, folding og geometri skal kunne testes uten vindu. `ui` skal bare tegne det `view` sier.

## Viktige valg

| Valg | Beslutning | Grunn |
|---|---|---|
| Diff-kjerne | Porte IntelliJ sin Myers-kjerne (ca. 650 linjer) | Gir samme resultat som JetBrains og golden-tester mot IntelliJ. `imara-diff` kan være alternativ bak en trait |
| Offsets | UTF-8 byte | IntelliJ bruker UTF-16, vi konverterer i golden-testene |
| Side-by-side | Ikke-aligned med trakt | Trakten er poenget. 1/3-anker og heltallssteg i tomme blokker hindrer hakking |
| Commit-view | Tre + én fil | Som commit-dialogen i IntelliJ. Beholder trakten og sync scroll |
| Git | `git`-CLI | Samme resultat som `git diff`, med brukerens config og rename-deteksjon |
| Syntax | tree-sitter, begge filer i sin helhet | Slettede linjer får riktig syntaks |
| Tråder | Diff og highlight i bakgrunnen, vent maks ca. 200 ms før spinner | Ingen blinking ved raske diffs |
| Shaders | GPUI `runtime_shaders` | Metal Toolchain mangler lokalt. Kan byttes når den er installert |

## Faser

1. **Diff-kjerne.** Port LCS og by_line med golden-tester. Så by_word. Et lite verktøy som kjører IntelliJ sin algoritme på samme input og skriver JSON, gir fasit.
2. **Én fil, JetBrains-kvalitet.** Radmodell, sync scroll, trakt med riktig kurve (0.3/0.7), farger, ord-markering, gutter, error stripe. Dette er milepælen alt avhenger av.
3. **Folding og navigasjon.** Kollapsede områder med bølge, neste/forrige endring med animasjon.
4. **Syntax highlighting.**
5. **Commit-view.** Tre med mapper, ikoner og status-farger, og én fil om gangen. Commit mot forelder og working tree mot HEAD.
6. **Git difftool og mappe mot mappe.**

## Fra spiken (`spikes/gpui-trakt`, slettet, ligger i git-historikken)

Bekreftet: GPUI gir skarp tekst, anti-aliasede bezier-paths, klipping per side og smud scroll. Delt horisontal scroll fungerer.

Må gjøres ordentlig:
- Kurven bruker kontrollpunkter på 50 %, JetBrains bruker 30 % og 70 %.
- Anker i midten, JetBrains bruker 1/3 ned.
- Egne farger, ikke JetBrains sine.
- Linjer shapes på nytt hver frame. Trenger cache.
- `imara-diff` med `postprocess_lines`, ikke IntelliJ-heuristikken.
- Speilet gutter (begge gutter mot divideren) mangler.
