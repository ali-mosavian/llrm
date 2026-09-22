fn value() -> i32:
    var a: [i32; 5] = [7; 5]
    a[2] = 1
    var total: i32 = 0
    for item in a:
        total += item
    return total
