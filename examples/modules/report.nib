# `report`: an alias shortens the qualifier.

import shapes.geometry as geo

pub fn describe(p: &geo.Point) -> string:
    match geo.side(p):
        geo.Side.left:
            return f"{p.x},{p.y} left"
        geo.Side.right:
            return f"{p.x},{p.y} right"
