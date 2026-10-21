//! Rollback og `fil:linje` for markert linje.

use std::ops::Range;

use jetdiff_viewmodel::{Cursor, FileDiff, Side};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRequest {
    /// Høyre side slik den vises, for å oppdage om fila er endret siden.
    pub before: String,
    pub lines: [Range<usize>; 2],
}

/// `block` er blokken cursoren står i.
pub fn rollback_request(
    diff: &FileDiff,
    block: Option<usize>,
    can_rollback: bool,
) -> Result<RollbackRequest, &'static str> {
    if !can_rollback {
        return Err("Bare lokale endringer kan rulles tilbake");
    }
    if diff.single_side().is_some() {
        return Err("Hele fila er ny eller slettet");
    }
    let block = block.ok_or("Ingen endring på linja")?;
    Ok(RollbackRequest {
        before: diff.text(Side::Right).to_owned(),
        lines: diff.blocks()[block].lines.clone(),
    })
}

/// `cursor` er markert linje i nyeste versjon.
pub fn location(cursor: Cursor, paths: &[String; 2]) -> Option<String> {
    let path = &paths[cursor.side.select(0, 1)];
    (!path.is_empty()).then(|| format!("{path}:{}", cursor.line + 1))
}
