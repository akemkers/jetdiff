//! The text width horizontal scroll uses in Split, where the sides can have different widths.

/// The narrower side, so horizontal scroll reaches the end of the lines on both. A side with no room
/// for text doesn't count.
pub fn split_text_width<Width: Copy + PartialOrd + Default>(text_widths: [Width; 2]) -> Width {
    let no_room = Width::default();
    match text_widths.map(|width| (width > no_room).then_some(width)) {
        [Some(left_side), Some(right_side)] if right_side < left_side => right_side,
        [Some(width), _] | [None, Some(width)] => width,
        [None, None] => no_room,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_narrower_side_with_room_for_text() {
        assert_eq!(split_text_width([300., 500.]), 300.);
        assert_eq!(split_text_width([500., 300.]), 300.);
        assert_eq!(split_text_width([0., 500.]), 500.);
        assert_eq!(split_text_width([37_u16, 0]), 37);
        assert_eq!(split_text_width([0_u16, 0]), 0);
    }
}
