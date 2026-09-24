# The panics the compiler calls (section 10), and the one every runtime
# routine stops with.

import os

# Writes `message` and ends the program; it never returns.
pub fn panic(message: &string) -> void:
    say("panic: ")
    say(message)
    say("\r\n")
    unsafe:
        os.exit(255)

pub fn say(text: &string) -> void:
    unsafe:
        let data: *far char = &text
        os.write(data.cast[u8](), text.len)

export "cdecl16":
    # INT 0: division by zero, or a quotient too wide for its register.
    @link_name("M$EDIV")
    fn divide() -> void:
        panic("division by zero or overflow")

    @link_name("M$ECNV")
    fn convert() -> void:
        panic("float outside the integer type")

    @link_name("M$ESHF")
    fn shift() -> void:
        panic("shift count out of range")

    @link_name("M$EBND")
    fn bounds() -> void:
        panic("index out of bounds")

    @link_name("M$EKEY")
    fn key() -> void:
        panic("key not found")
