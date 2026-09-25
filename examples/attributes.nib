# bits structs: fields packed into one integer from bit 0 upward, read and
# written with shifts and masks, converted to and from the backing integer.

# The VGA text attribute byte.
bits struct Attr: u8
    mut fg: u4
    mut bg: u3
    blink: bool

# The 16550 UART's line status register.
bits struct LineStatus: u8
    data_ready: bool
    overrun: bool
    parity: bool
    framing: bool
    break_: bool
    holding_empty: bool
    idle: bool
    fifo_error: bool

enum Parity: u2
    none
    odd
    even
    mark

bits struct LineControl: u8
    word: u2
    mut stop: u1
    parity: Parity
    mut offset: i3

fn inverted(a: Attr) -> Attr:
    let mut flipped = a
    flipped.fg = a.bg
    flipped.bg = u8(a.fg & 7)
    return flipped

fn main() -> i16:
    let normal = Attr(fg=7, bg=0, blink=false)
    let warning = Attr(fg=14, bg=4, blink=true)
    print(f"normal {u8(normal)}, warning {u8(warning)}, inverted {u8(inverted(normal))}")

    let status = LineStatus(0x61)
    if status.data_ready && status.holding_empty:
        print("byte waiting, ready to send")
    print(f"errors: {status.overrun || status.parity || status.framing}")

    let mut control = LineControl(word=3, stop=0, parity=Parity.even, offset=-2)
    control.stop = 1
    control.offset += 3
    print(f"control {u8(control)}: parity {u8(control.parity)}, offset {control.offset}")
    return 0
