//! Tastene i tastelinja og hjelpen.

pub type Keys = (&'static str, &'static str);

pub const BAR: [Keys; 5] = [
    ("Tab Shift-Tab", "fil"),
    ("j k", "opp/ned"),
    ("n p", "endring"),
    ("?", "alle taster"),
    ("q", "avslutt"),
];

/// Ctrl-variantene fra vim er ikke med.
pub const ALL: [(&str, &[Keys]); 4] = [
    (
        "Flytt",
        &[
            ("j k", "opp/ned"),
            ("h l", "sideveis"),
            ("0 $", "linjestart/-slutt"),
            ("d u", "halv side"),
            ("gg G", "topp og bunn"),
            ("n p", "neste/forrige endring"),
            ("H L", "venstre/høyre side"),
        ],
    ),
    (
        "Filer",
        &[
            ("Tab Shift-Tab", "neste/forrige fil"),
            ("J K", "neste/forrige fil"),
            ("C-hjkl", "bytt panel"),
            ("b", "skjul panel"),
        ],
    ),
    ("Folds", &[("o", "åpne fold"), ("f", "toggle folds")]),
    (
        "Annet",
        &[
            ("v", "split/unified"),
            ("y", "kopier fil:linje"),
            ("r", "rull tilbake endring"),
            ("Cmd +/-/0", "zoom"),
            ("q", "avslutt"),
        ],
    ),
];
