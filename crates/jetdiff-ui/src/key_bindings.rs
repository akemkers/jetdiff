//! The keys outside the diff as GPUI key bindings, under jetdiff's key context, so they only apply
//! when the focus is inside `RepoView` or `FilePairView`.

use gpui::{KeyBinding, actions};
use jetdiff_app::{AppCommand, BINDINGS};

use crate::file_diff::{ResetZoom, ToggleDiffMode, ZoomIn, ZoomOut};
use crate::help::{HideHelp, ToggleHelp};
use crate::repo_view::{
    FocusDown, FocusLeft, FocusRight, FocusUp, NextCommit, NextFile, PrevCommit, PrevFile,
    SourceReload, ToggleSidePanel, ToggleWalkthrough,
};

pub const KEY_CONTEXT: &str = "jetdiff";

// The views don't handle Quit. Whoever shows them decides what it does, like quitting the app.
actions!(jetdiff, [Quit]);

/// The rest of the keys go through the vim parser in `FileDiffView`.
pub fn key_bindings() -> Vec<KeyBinding> {
    BINDINGS
        .iter()
        .map(|&(keys, command)| key_binding(keys, command))
        .collect()
}

fn key_binding(keys: &str, command: AppCommand) -> KeyBinding {
    let context = Some(KEY_CONTEXT);
    match command {
        AppCommand::ZoomIn => KeyBinding::new(keys, ZoomIn, context),
        AppCommand::ZoomOut => KeyBinding::new(keys, ZoomOut, context),
        AppCommand::ResetZoom => KeyBinding::new(keys, ResetZoom, context),
        AppCommand::FocusLeft => KeyBinding::new(keys, FocusLeft, context),
        AppCommand::FocusDown => KeyBinding::new(keys, FocusDown, context),
        AppCommand::FocusUp => KeyBinding::new(keys, FocusUp, context),
        AppCommand::FocusRight => KeyBinding::new(keys, FocusRight, context),
        AppCommand::ToggleSidePanel => KeyBinding::new(keys, ToggleSidePanel, context),
        AppCommand::ToggleDiffMode => KeyBinding::new(keys, ToggleDiffMode, context),
        AppCommand::NextFile => KeyBinding::new(keys, NextFile, context),
        AppCommand::PrevFile => KeyBinding::new(keys, PrevFile, context),
        AppCommand::NextCommit => KeyBinding::new(keys, NextCommit, context),
        AppCommand::ToggleWalkthrough => KeyBinding::new(keys, ToggleWalkthrough, context),
        AppCommand::PrevCommit => KeyBinding::new(keys, PrevCommit, context),
        AppCommand::SourceReload => KeyBinding::new(keys, SourceReload, context),
        AppCommand::ToggleHelp => KeyBinding::new(keys, ToggleHelp, context),
        AppCommand::HideHelp => KeyBinding::new(keys, HideHelp, context),
        AppCommand::Quit => KeyBinding::new(keys, Quit, context),
    }
}
