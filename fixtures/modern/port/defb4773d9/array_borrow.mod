fn bump(values: &mut [u16]) -> void:
    values[1] += values.len()
fn calculate() -> u16:
    var values: [u16; 3] = [10, 20, 30]
    bump(&mut values)
    return values[0] + values[1] + values[2]
