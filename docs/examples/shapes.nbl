# Enums with and without payloads, and exhaustive match.

struct Point:
    x: i16
    y: i16

enum Shape:
    point(Point)
    circle(center: Point, radius: u16)
    rectangle(min: Point, max: Point)

enum Mode: u8
    text
    cga
    vga = 19

fn area(shape: &Shape) -> i32:
    match shape:
        .point(_):
            return 0
        .circle(_, radius):
            let r = i32(radius)
            return 3 * r * r
        .rectangle(Point(x0, y0), Point(x1, y1)):
            return i32(x1 - x0) * i32(y1 - y0)

fn columns(mode: Mode) -> u16:
    match mode:
        .text:
            return 80
        .cga:
            return 40
        .vga:
            return 320

fn main() -> i16:
    let shapes: Shape[3] = [
        Shape.point(Point(1, 2)),
        Shape.circle(center=Point(0, 0), radius=5),
        .rectangle(min=Point(1, 1), max=Point(4, 5)),
    ]
    let mut total: i32 = 0
    for shape in &shapes:
        total += area(shape)
    print(f"area {total}")
    let mode = Mode.vga
    print(f"mode {mode} has {columns(mode)} columns")
    print(f"text is {columns(.text)}")
    return 0
