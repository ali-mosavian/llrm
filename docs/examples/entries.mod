# Section 18's complete example: `name=value` lines from VALUES.DAT, read
# through std.io, parsed, and summed up.

import std.io as io

enum LoadError:
    not_found
    unreadable
    invalid(line: u16)

struct Entry:
    name: string
    value: i16

# `line` as `name=value`, the value a decimal integer.
fn parse(line: &string, number: u16) -> Result[Entry, LoadError]:
    let mut at: u16 = 0
    while at < line.len && line[at] != '=':
        at += 1
    let negative = at + 1 < line.len && line[at + 1] == '-'
    let mut digit = negative ? at + 2 : at + 1
    if at == 0 || digit >= line.len:
        return .err(.invalid(number))
    let mut value: i16 = 0
    while digit < line.len:
        let letter = line[digit]
        if letter < '0' || letter > '9':
            return .err(.invalid(number))
        value = value * 10 + i16(u8(letter) - u8('0'))
        digit += 1
    let name = &line[0:at]
    return .ok(Entry(name=name.copy(), value=negative ? -value : value))

fn load_entries(path: &string) -> Result[vec[Entry], LoadError]:
    match io.File.open(path):
        .ok(opened):
            let mut file = opened
            let mut entries: vec[Entry] = []
            let mut number: u16 = 0
            for line in file.lines():
                number += 1
                if !line.empty():
                    let entry = parse(&line, number)?
                    entries.push(entry)
            return .ok(entries)
        .err(.not_found):
            return .err(.not_found)
        .err(_):
            return .err(.unreadable)

fn main() -> Result[void, LoadError]:
    let entries = load_entries("VALUES.DAT")?

    let positive = {
        entry.name.copy(): entry.value
        for entry in entries
        if entry.value > 0
    }

    match entries:
        []:
            print("no entries")
        [first, *rest]:
            print(f"first={first.name}, remaining={rest.len}")
    print(f"{positive.len} positive, width={positive["width"]}")
    return .ok()
