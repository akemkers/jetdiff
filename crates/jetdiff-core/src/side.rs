//! Port of `Side.kt`, only what is needed.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn from_left(is_left: bool) -> Self {
        if is_left { Side::Left } else { Side::Right }
    }

    pub fn other(self) -> Self {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    pub fn select<T>(self, left: T, right: T) -> T {
        match self {
            Side::Left => left,
            Side::Right => right,
        }
    }
}
