fn main() -> i16:
    let values: [i16; 5] = [1, 2, 4, 8, 16]
    var total: i16 = 0
    for value in &values[1:4]:
        total += value
    return total
