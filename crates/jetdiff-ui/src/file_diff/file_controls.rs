//! The file controls at the right of the diff header: the diff mode and Fold.

use gpui::{App, ClickEvent, Context, Div, Window, div, prelude::*, px};
use jetdiff_viewmodel::{Command, DiffMode};

use super::{FileDiffView, ToggleDiffMode};
use crate::button_group::{Button, group, toggle};
use crate::theme::UiPalette;

const GAP: f32 = 10.;

impl FileDiffView {
    pub(super) fn file_controls(
        &self,
        palette: &UiPalette,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GAP * scale))
            .child(self.diff_mode(palette, scale, cx))
            .child(self.fold(palette, scale, cx))
    }

    /// A click on the side not shown does what `v` does.
    fn diff_mode(&self, palette: &UiPalette, scale: f32, cx: &mut Context<Self>) -> Div {
        let split = matches!(self.mode, DiffMode::Split(_));
        let buttons = vec![
            Button::new("Split", toggle_diff_mode(cx))
                .active(split)
                .enabled(self.mode.diff().can_split()),
            Button::new("Unified", toggle_diff_mode(cx)).active(!split),
        ];
        group("diff-mode", buttons, palette, scale)
    }

    /// Does what `f` does, which only works in Split until folds come to Unified.
    fn fold(&self, palette: &UiPalette, scale: f32, cx: &mut Context<Self>) -> Div {
        let split = matches!(self.mode, DiffMode::Split(_));
        let fold = Button::new(
            "Fold",
            cx.listener(|this, _, window, cx| this.execute(Command::ToggleFolds, window, cx)),
        )
        .active(split && self.fold_unchanged)
        .enabled(split);
        toggle("fold", fold, palette, scale)
    }
}

fn toggle_diff_mode(
    cx: &mut Context<FileDiffView>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(|this, _, window, cx| this.toggle_diff_mode(&ToggleDiffMode, window, cx))
}
