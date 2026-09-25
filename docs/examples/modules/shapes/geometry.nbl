# `shapes.geometry`: only its `pub` declarations can be named elsewhere.

pub struct Point:
    x: i16
    y: i16

pub enum Side:
    left
    right

pub fn Point.shifted(self: &Point, dx: i16) -> Point:
    return Point(x=self.x + dx, y=self.y)

pub fn side(p: &Point) -> Side:
    return p.x < 0 ? Side.left : Side.right

fn secret() -> i16:
    return 7
