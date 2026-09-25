# Sequence patterns take a vec, array or view apart without copying it.

struct Point:
    x: i16
    y: i16

fn describe(values: &[i16]) -> i16:
    match values:
        []:
            print("empty")
        [only]:
            print(f"one {only}")
        [first, *middle, last]:
            print(f"{first}..{last} around {middle.len}")
    return 0

fn head(values: &[i16]) -> i16:
    let [first, *rest] = values else:
        return -1
    return first + i16(rest.len)

fn main() -> i16:
    let none: vec[i16] = []
    let one: i16[1] = [7]
    let many: vec[i16] = [1, 2, 3, 4]
    describe(&none)
    describe(&one)
    describe(&many)
    print(f"{head(&many)} {head(&none)}")
    let points: vec[Point] = [Point(x=1, y=2), Point(x=3, y=4)]
    match points:
        [Point(x, y), *_]:
            print(f"first at {x},{y}")
        _:
            print("no points")
    match many:
        [1, 2, *tail]:
            print(f"starts 1, 2 then {tail.len}")
        _:
            print("other")
    print(f"{none.empty()} {many.empty()}")
    return 0
