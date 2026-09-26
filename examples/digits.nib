# Option, Result, and '?': parse a decimal number from a string's chars.

enum ParseError:
    empty
    not_digit(at: u16)
    too_big

fn digit(c: char) -> Result[u8, ParseError]:
    if c < '0' || c > '9':
        return .err(.not_digit(0))
    return .ok(u8(c) - u8('0'))

fn parse(text: string) -> Result[u16, ParseError]:
    if text.len == 0:
        return .err(.empty)
    let mut value: u32 = 0
    for c in text:
        value = value * 10 + u32(digit(c)?)
        if value > 65535:
            return .err(.too_big)
    return .ok(u16(value))

fn first_even(values: &[i16]) -> Option[i16]:
    for value in values:
        if value % 2 == 0:
            return .some(value)
    return .none

fn report(text: string) -> void:
    match parse(text):
        .ok(value):
            print(f"{value}")
        .err(.empty):
            print("empty")
        .err(.not_digit(_)):
            print("not a digit")
        .err(.too_big):
            print("too big")

fn main() -> i16:
    report("1234")
    report("")
    report("12x4")
    report("99999")
    let values: i16[4] = [3, 7, 8, 9]
    match first_even(values):
        .some(value):
            print(f"first even {value}")
        .none:
            print("no even value")
    return 0
