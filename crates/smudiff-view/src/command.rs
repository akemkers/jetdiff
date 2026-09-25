//! Alt brukeren kan be diffen om. Tastene oversettes hit i `keys`, og `DiffState` utfører.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    LineStart,
    LineEnd,
    GoToTop,
    GoToBottom,
    HalfPageDown,
    HalfPageUp,
    NextChange,
    PrevChange,
    /// Neste fil i treet. Gjør ingenting når bare én fil vises.
    NextFile,
    PrevFile,
    SwitchSide,
    /// Ett nivå, som `ExpandRegionAction`.
    FoldOpen,
    /// Åpner alle hvis noen er lukket, ellers lukkes alle.
    ToggleFolds,
    NextContextRange,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    Quit,
}
