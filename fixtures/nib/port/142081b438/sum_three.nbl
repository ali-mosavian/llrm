fn sum_three(first: &[i16], second: &[i16], third: &[i16]) -> i16:
    let mut total: i16 = 0
    for index in 0..first.len:
        total += first[index]
        total += second[index]
        total += third[index]
    return total

fn main() -> i16:
    let first: i16[4] = [1, 2, 3, 4]
    let second: i16[4] = [10, 20, 30, 40]
    let third: i16[4] = [100, 200, 300, 400]
    let result = sum_three(&first, &second, &third)
    if result == 1110:
        print("sum_three: ok")
    else:
        print("sum_three: bad")
    return result
