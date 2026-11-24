//! The keys in the keymap bar and the help.

/// Each key is drawn as a key label of its own, followed by what the keys do.
pub type Keys = (&'static [&'static str], &'static str);

pub const DIFF_KEYMAP_BAR: [Keys; 6] = [
    (&["Tab", "⇧Tab"], "file"),
    (&["j", "k"], "line"),
    (&["n", "p"], "hunk"),
    WALKTHROUGH,
    (&["?"], "all keys"),
    (&["q"], "quit"),
];

/// No `?`, since the help is not shown in the walkthrough.
pub const WALKTHROUGH_KEYMAP_BAR: [Keys; 7] = [
    (&["Tab", "⇧Tab"], "step"),
    (&["j", "k"], "scroll"),
    (&["n", "p"], "hunk"),
    (&["Enter"], "open in diff"),
    (&["v"], "split/unified"),
    (&["w"], "diff"),
    (&["q"], "quit"),
];

/// The walkthrough is only in the window so far, so the terminal leaves these out.
pub const WALKTHROUGH: Keys = (&["w"], "walkthrough");
pub const WALKTHROUGH_HELP: Keys = (&["w"], "walkthrough, in steps");

/// Zoom is on the secondary modifier, which is Cmd only on macOS.
pub const ZOOM: Keys = if cfg!(target_os = "macos") {
    (&["⌘+", "⌘-", "⌘0"], "zoom")
} else {
    (&["⌃+", "⌃-", "⌃0"], "zoom")
};

/// The Ctrl variants from vim are not included.
pub const ALL: [(&str, &[Keys]); 4] = [
    (
        "Move",
        &[
            (&["j", "k"], "up/down"),
            (&["h", "l"], "sideways"),
            (&["0", "$"], "line start/end"),
            (&["d", "u"], "half page"),
            (&["gg", "G"], "top and bottom"),
            (&["n", "p"], "next/previous hunk"),
            (&["H", "L"], "Left/Right side"),
        ],
    ),
    (
        "Files",
        &[
            (&["Tab", "⇧Tab"], "next/previous file"),
            WALKTHROUGH_HELP,
            (&["J", "K"], "next/previous file"),
            (&["N", "P"], "next/previous commit"),
            (&["⌃hjkl"], "move focus"),
            (&["b"], "toggle side panel"),
        ],
    ),
    ("Folds", &[(&["o"], "open fold"), (&["f"], "toggle folds")]),
    (
        "Other",
        &[
            (&["v"], "split/unified"),
            (&["y"], "copy file:line"),
            (&["r"], "reload"),
            (&["z"], "roll back hunk"),
            ZOOM,
            (&["q"], "quit"),
        ],
    ),
];
