# jetdiff-core keeps IntelliJ's names

`jetdiff-core` is a port of IntelliJ's diff algorithm and is golden-tested against it, so it keeps IntelliJ's names (`LineFragment`, `DiffFragment`, `chunk`, `lines1`/`lines2`) to stay traceable to the Kotlin and Java source. Every other crate uses the language in `GLOSSARY.md`, and the translation happens where they use core. Other crates that port IntelliJ code point to the original in comments instead of keeping its names. The notes in `docs/research/` describe IntelliJ and keep its words.
