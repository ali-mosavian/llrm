# Reads fields out of CSV rows as views: `field` and `trimmed` return views
# of the row they were lent, so no field is copied (sections 9 and 13).

fn trimmed(text: &string) -> &string:
    let mut start: u16 = 0
    let mut end = text.len
    while start < end && text[start] == ' ':
        start += 1
    while end > start && text[end - 1] == ' ':
        end -= 1
    return &text[start:end]

fn field(row: &string, wanted: u16) -> &string:
    let mut index: u16 = 0
    let mut start: u16 = 0
    for at in 0..row.len:
        if row[at] == ',':
            if index == wanted:
                return trimmed(&row[start:at])
            index += 1
            start = at + 1
    if index == wanted:
        return trimmed(&row[start:row.len])
    return &row[0:0]

fn main() -> i16:
    let rows = ["Ada, pilot, 7", "Grace,  admiral , 9", "Linus ,  , 3"]
    for row in rows:
        let name = field(row, 0)
        let role = field(row, 1)
        if role.len == 0:
            print(f"{name:-6}| (no role)")
        else:
            print(f"{name:-6}| {role} at level {field(row, 2)}")
    let best = field(rows[1], 0)
    print(f"{best == "Grace"} {best.copy()}")
    return 0
