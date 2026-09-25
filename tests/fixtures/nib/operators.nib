fn mix(a: u16, b: u16, n: u8) -> u16:
    let mut x: u16 = a
    x <<= n
    x |= b
    x ^= 21845
    x &= 65280
    x >>= n
    return x | ~b & a ^ b
fn shifts(a: i32, n: u16) -> i32:
    return (a >> n) + (a << n)
fn logic(a: i16, b: i16) -> bool:
    return a < b && !a == 0 || b == 7
fn widen(a: i8, b: u8, c: bool) -> i32:
    return i32(a) + i32(b) + i32(c) + i32(i16(u8(a)))
fn narrow(a: i32) -> i16:
    return i16(i8(a)) + i16(u8(a))
fn filled(seed: i16) -> i16:
    let mut cells: i16[9] = [seed * 3] * 9
    cells[4] = 1
    let mut total: i16 = 0
    for cell in cells:
        total += cell
    return total
fn main() -> i16:
    print(f"{mix(4660, 43981, 3)} {shifts(-100000, 3)} {shifts(100000, 5)}")
    print(f"{i16(logic(1, 2))} {i16(logic(0, 2))} {i16(logic(3, 7))} {i16(logic(3, 2))}")
    print(f"{widen(-5, 200, true)} {narrow(-129)} {narrow(70000)} {filled(5)}")
    return 0
