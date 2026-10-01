# jetdiff

Binæren. Leser argumentene og config-fila, laster innholdet og åpner vinduet.

## Hvorfor

Alt annet er biblioteker som ikke antar noe om appen rundt. Det er fordi diff-viewen skal kunne bli en modul i et større verktøy. Denne craten er det eneste stedet som bestemmer hvordan jetdiff startes: argumenter, config-fil og tastebindinger.

## Bruk

```
cargo run --release -p jetdiff -- [<mappe> | <commit> | <pr-lenke> | <venstre> <høyre>]
```

Se hoved-[README](../../README.md) for argumentene, innstillingene og tastene.

```mermaid
graph LR
    args[argumenter] --> source{Source}
    source -- to filer --> fd[FileDiff] --> fdv[FileDiffView]
    source -- mappe --> wt[Repo::work_tree] --> cv[ChangesView]
    source -- commit --> c[Repo::commit] --> cv
```

Innholdet lastes før vinduet åpnes, så feil som ukjent commit skrives til terminalen med exit-kode 1. Feil i argumentene eller config-fila gir exit-kode 2.

Tastebindinger som ikke går gjennom vim-parseren i `jetdiff-view` ligger i `bind_keys` i `main.rs`: zoom, panelbytte, filbytte, `b`, `?` og avslutt.
