fn sum(values: &[i16]) -> i16:
    var total: i16 = 0
    for value in &values:
        total += value
    return total

fn first(text: string) -> char:
    for byte in text:
        return byte
    return '\0'

fn main() -> i16:
    let values: [i16; 5] = [1, 2, 4, 8, 16]
    let text: string = "metal"
    let middle = sum(&values[1:4])
    if values.len() == 5:
        if values.capacity() == 5:
            if middle == 14:
                if text.len() == 5:
                    if text.capacity() == 5:
                        if first(text) == 'm':
                            print("descriptors: ok")
                            return 0
    print("descriptors: bad")
    return 1
