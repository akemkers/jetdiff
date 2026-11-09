//! The handle over the diff divider in Split, in the diff and in the walkthrough. Dragging it changes
//! the share of the width Left and Right get, and a double click gives 50/50 again.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{AnyElement, Context, Div, DragMoveEvent, ElementId, div, prelude::*, px};
use jetdiff_viewmodel::SplitShare;
use jetdiff_viewmodel::divider::DIVIDER_WIDTH;

use crate::drag_handle::drag_handle;

struct DiffDividerDrag {
    /// `on_drag_move` gets every drag of this type, and a walkthrough page has a handle per excerpt.
    id: ElementId,
    /// Where in the handle it was grabbed, so the divider doesn't jump to the mouse.
    grab_x: Rc<Cell<f32>>,
}

/// `code` fills the area, with the handle `offset` from its left edge. No handle when `offset` is
/// `None`, as in Unified or before the code is drawn.
pub fn with_diff_divider_handle<T: 'static>(
    id: ElementId,
    code: AnyElement,
    offset: Option<f32>,
    cx: &mut Context<T>,
    set_split_share: fn(&mut T, SplitShare, &mut Context<T>),
) -> Div {
    let handle = offset.map(|offset| {
        let drag = DiffDividerDrag {
            id: id.clone(),
            grab_x: Rc::default(),
        };
        drag_handle(
            id.clone(),
            drag,
            cx,
            |drag, grab| drag.grab_x.set(f32::from(grab.x)),
            move |this, cx| set_split_share(this, SplitShare::default(), cx),
        )
        .top_0()
        .bottom_0()
        .left(px(offset))
        .w(px(DIVIDER_WIDTH))
        .cursor_col_resize()
    });
    div()
        .relative()
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<DiffDividerDrag>, _, cx| {
                let drag = event.drag(cx);
                if drag.id != id {
                    return;
                }
                let x = f32::from(event.event.position.x - event.bounds.left());
                let middle = x - drag.grab_x.get() + DIVIDER_WIDTH / 2.;
                let width = f32::from(event.bounds.size.width);
                set_split_share(this, SplitShare::at(middle, width, DIVIDER_WIDTH), cx);
            }),
        )
        .child(code)
        .children(handle)
}
