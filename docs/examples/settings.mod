# Reads "key=value" settings from one line through views: no field is copied
# until the program keeps it.

fn digits(text: &string) -> i16:
    let mut value: i16 = 0
    for c in text:
        value = value * 10 + i16(c) - i16('0')
    return value

fn show(key: &string, value: &string) -> void:
    print(f"{key:-8}= {value}")
    if key == "level":
        print(f"          twice that is {digits(value) * 2}")

fn keep(text: &string) -> string:
    return text.copy()

fn each_setting(line: &string) -> u16:
    let mut count: u16 = 0
    let mut start: u16 = 0
    let mut at: u16 = 0
    while at <= line.len:
        if at == line.len || line[at] == ';':
            let mut equals = start
            while equals < at && line[equals] != '=':
                equals += 1
            show(&line[start:equals], &line[equals + 1:at])
            count += 1
            start = at + 1
        at += 1
    return count

fn main() -> i16:
    let line = "name=Ada;role=pilot;level=7"
    let found = each_setting(&line)
    let kept = keep(&line[5:8])
    print(f"{found} settings, kept {kept}")
    return 0
