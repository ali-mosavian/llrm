fn value() -> i16:
    let a: [i16; 2] = [5, 6]
    if true:
        let a: [i16; 3] = [a[0]; 3]
        return a[0] + a[2]
    return 0
