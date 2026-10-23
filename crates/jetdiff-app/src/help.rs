//! The keys in the keymap bar and the help.

pub type Keys = (&'static str, &'static str);

pub const KEYMAP_BAR: [Keys; 5] = [
    ("Tab Shift-Tab", "file"),
    ("j k", "up/down"),
    ("n p", "hunk"),
    ("?", "all keys"),
    ("q", "quit"),
];

/// The Ctrl variants from vim are not included.
pub const ALL: [(&str, &[Keys]); 4] = [
    (
        "Move",
        &[
            ("j k", "up/down"),
            ("h l", "sideways"),
            ("0 $", "line start/end"),
            ("d u", "half page"),
            ("gg G", "top and bottom"),
            ("n p", "next/previous hunk"),
            ("H L", "Left/Right side"),
        ],
    ),
    (
        "Files",
        &[
            ("Tab Shift-Tab", "next/previous file"),
            ("J K", "next/previous file"),
            ("C-hjkl", "move focus"),
            ("b", "toggle side panel"),
        ],
    ),
    ("Folds", &[("o", "open fold"), ("f", "toggle folds")]),
    (
        "Other",
        &[
            ("v", "split/unified"),
            ("y", "copy file:line"),
            ("r", "roll back hunk"),
            ("Cmd +/-/0", "zoom"),
            ("q", "quit"),
        ],
    ),
];
