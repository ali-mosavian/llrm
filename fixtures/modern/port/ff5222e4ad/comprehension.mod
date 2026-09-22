fn main() -> i16:
    let values: [i16; 4] = [1, 2, 3, 4]
    let doubled = [value * 2 for value in values]
    var total: i16 = 0
    for value in (item + 1 for item in doubled):
        total += value
    return total
