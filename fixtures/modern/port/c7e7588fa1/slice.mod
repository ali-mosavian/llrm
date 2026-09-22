fn sum(values: &[i16]) -> i16:
    var total: i16 = 0
    for value in &values:
        total += value
    return total
fn main() -> i16:
    let values: [i16; 4] = [1, 2, 3, 4]
    return sum(&values[1:3])
