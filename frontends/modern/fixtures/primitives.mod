fn boolValue() -> bool:
    return true

fn charValue() -> char:
    return 'A'

fn i8Value() -> i8:
    return -128

fn u8Value() -> u8:
    return 255

fn i16Value() -> i16:
    return -32768

fn u16Value() -> u16:
    return 65535

fn i32Value() -> i32:
    return -2147483648

fn u32Value() -> u32:
    return 4294967295

fn f32Value() -> f32:
    return 1.5

fn f64Value() -> f64:
    return -2.25

fn unsignedDivide(left: u32, right: u32) -> u32:
    return left / right

fn unsignedRemainder(left: u16, right: u16) -> u16:
    return left % right

fn unsignedLess(left: u8, right: u8) -> bool:
    return left < right

fn floatProduct(left: f32, right: f32) -> f32:
    return left * right

fn nothing() -> void:
    return
