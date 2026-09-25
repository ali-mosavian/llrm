# Owned strings: literals are static until written, '+' and f-strings build
# heap strings, and every owner drops what it holds.

fn shout(text: string) -> string:
    let mut loud = text
    let mut at: u16 = 0
    while at < loud.len:
        let c = loud[at]
        if 'a' <= c <= 'z':
            loud[at] = char(u8(c) - 32)
        at += 1
    return loud

fn label(name: string, score: i16) -> string:
    return f"{name}: {score}"

fn main() -> i16:
    let greeting = "hello"
    let mut message: string = greeting + ", world"
    message.append("!")
    print(message)
    print(shout(message.copy()))
    print(greeting)
    let board = label("ada", 42)
    print(f"[{board}] has {board.len} chars")
    if board < label("bob", 1):
        print("ada sorts first")
    message = "reset"
    print(message)
    return 0
