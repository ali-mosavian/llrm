fn bool_value() -> bool:
    return true

fn char_value() -> char:
    return 'A'

fn i8_value() -> i8:
    return -128

fn u8_value() -> u8:
    return 255

fn i16_value() -> i16:
    return -32768

fn u16_value() -> u16:
    return 65535

fn i32_value() -> i32:
    return -2147483648

fn u32_value() -> u32:
    return 4294967295

fn f32_value() -> f32:
    return 1.5

fn f64_value() -> f64:
    return -2.25

fn unsigned_divide(left: u32, right: u32) -> u32:
    return left / right

fn unsigned_remainder(left: u16, right: u16) -> u16:
    return left % right

fn unsigned_less(left: u8, right: u8) -> bool:
    return left < right

fn float_product(left: f32, right: f32) -> f32:
    return left * right

fn nothing() -> void:
    return
