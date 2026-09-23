fn bump(values: &mut [u16]) -> void:
    values[1] += 3
fn main() -> i16:
    var values: [u16; 3] = [10, 20, 30]
    bump(&mut values)
    return 0
