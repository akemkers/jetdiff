# jetdiff-core keeps IntelliJ's names

`jetdiff-core` is a port of IntelliJ's diff algorithm and is golden-tested against it, so it keeps IntelliJ's names (`LineFragment`, `DiffFragment`, `chunk`, `lines1`/`lines2`) to stay traceable to the Kotlin and Java source. The language in `GLOSSARY.md` applies from `jetdiff-viewmodel` and up, and the translation happens where those crates use core. The notes in `docs/research/` describe IntelliJ and keep its words for the same reason.
