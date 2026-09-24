# A sensor log: a smoothed stream of readings is a generator that escapes --
# it is stored, peeked at, and passed on to what reports it.

fn smoothed(samples: &[i16], width: u16) -> iter[i16]:
    let mut at: u16 = 0
    while at + width <= samples.len:
        let mut sum: i16 = 0
        for k in at..at + width:
            sum += samples[k]
        yield sum // i16(width)
        at += 1

fn peak(values: iter[i16]) -> i16:
    let mut best: i16 = -32768
    for v in values:
        if v > best:
            best = v
    return best

fn report(label: &string, values: iter[i16]) -> void:
    let mut line: string = f"{label:-8}"
    for v in values:
        line.append(f" {v:3}")
    print(line)

fn main() -> i16:
    let raw: i16[8] = [12, 15, 11, 30, 28, 27, 9, 10]
    report("raw", raw.iter())
    report("avg3", smoothed(raw, 3))
    let mut trend = smoothed(raw, 2)
    match trend.next():
        .some(first):
            print(f"first pair averages {first}")
        .none:
            print("too few readings")
    print(f"peak of the rest: {peak(trend)}")
    return 0
