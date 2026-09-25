fn main() -> i16:
    let values: i16[4] = [1, 2, 1, 3]
    let doubled = [value * 2 for value in values]
    let mut generated_total: i16 = 0
    for value in (item + 1 for item in doubled):
        generated_total += value

    let table = {item: item * 10 for item in values}
    let lookup_total = table.get(1, 0) + table.get(3, 0) + table.get(9, 5)
    if doubled.len == 4:
        if generated_total == 18:
            if table.len == 3:
                if table.capacity == 4:
                    if lookup_total == 45:
                        print("collections: ok")
                        return 0
    print("collections: bad")
    return 1
