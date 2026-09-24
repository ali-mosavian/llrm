fn value() -> u16:
    let mut x: u16 = 1
    x <<= 4
    x |= 3
    x ^= 1
    x &= 255
    x >>= 1
    return x
