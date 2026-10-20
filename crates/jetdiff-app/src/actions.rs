//! Det `r` og `y` i diffen gjør, uten UI. UI-et spør om bekreftelse og skriver til utklippstavla.

use std::ops::Range;

use jetdiff_view::{DiffMode, Side};

/// Brukeren vil rulle tilbake en endring. Sendes til den som kan skrive fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRequest {
    /// Høyre side slik den vises, for å oppdage om fila er endret siden.
    pub before: String,
    /// Linjene i endringen, per side.
    pub lines: [Range<usize>; 2],
}

/// Endringen på markert linje, eller hvorfor den ikke kan rulles tilbake.
pub fn rollback_request(
    mode: &DiffMode,
    can_rollback: bool,
) -> Result<RollbackRequest, &'static str> {
    let diff = mode.diff();
    if !can_rollback {
        return Err("Bare lokale endringer kan rulles tilbake");
    }
    if diff.single_side().is_some() {
        return Err("Hele fila er ny eller slettet");
    }
    let block = mode.cursor_block().ok_or("Ingen endring på linja")?;
    Ok(RollbackRequest {
        before: diff.text(Side::Right).to_owned(),
        lines: diff.blocks()[block].lines.clone(),
    })
}

/// `fil:linje` for markert linje. `None` når det ikke finnes noen fil.
pub fn location(mode: &DiffMode, paths: &[String; 2]) -> Option<String> {
    let c = mode.location();
    let path = &paths[c.side.select(0, 1)];
    (!path.is_empty()).then(|| format!("{path}:{}", c.line + 1))
}
