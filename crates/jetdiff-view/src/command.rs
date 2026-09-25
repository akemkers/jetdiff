//! Alt brukeren kan be diffen om. Tastene oversettes hit i `keys`, og `DiffState` utfører.

use jetdiff_core::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    MoveDown,
    MoveUp,
    ScrollLeft,
    ScrollRight,
    /// Helt til venstre.
    ScrollToStart,
    /// Slutten av den markerte linja til høyre kant.
    ScrollToEnd,
    GoToTop,
    GoToBottom,
    HalfPageDown,
    HalfPageUp,
    NextChange,
    PrevChange,
    /// Neste fil i treet. Gjør ingenting når bare én fil vises.
    NextFile,
    PrevFile,
    /// Gjør ingenting når bare én side vises, eller siden alt er aktiv.
    FocusSide(Side),
    /// Ett nivå, som `ExpandRegionAction`.
    FoldOpen,
    /// Åpner alle hvis noen er lukket, ellers lukkes alle.
    ToggleFolds,
    NextContextRange,
    /// `fil:linje` for markert linje til utklippstavla.
    CopyLocation,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    Quit,
}
