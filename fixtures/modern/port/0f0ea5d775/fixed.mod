type fixed16 = fixed i32, fraction=16

fn scaled(left: fixed16, right: fixed16) -> fixed16:
    return left * right / right

fn main() -> i16:
    scaled(1.5, 2.25)
    return 0
