//! A handle dragged with the mouse to change a size, for the side panel and the diff divider.

use gpui::{
    ClickEvent, Context, Div, ElementId, EmptyView, Pixels, Point, Stateful, div, prelude::*,
};

/// An absolute handle that drags `drag`. `grabbed` gets where in the handle the drag started, and a
/// double click calls `reset`. Give each size its own `drag` type, since `on_drag_move` listens to
/// all drags of the same type.
pub fn drag_handle<T: 'static, D: 'static>(
    id: impl Into<ElementId>,
    drag: D,
    cx: &mut Context<T>,
    grabbed: impl Fn(&D, Point<Pixels>) + 'static,
    reset: impl Fn(&mut T, &mut Context<T>) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .on_drag(drag, move |drag, grab, _, cx| {
            grabbed(drag, grab);
            cx.new(|_| EmptyView)
        })
        .on_click(cx.listener(move |this, click: &ClickEvent, _, cx| {
            if click.click_count() == 2 {
                reset(this, cx);
                cx.notify();
            }
        }))
}
