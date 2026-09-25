# vec[T]: an owned, growable sequence. push and pop grow and shrink it,
# a list literal or comprehension builds one, and dropping it drops
# whatever its elements own.

struct Item:
    name: string
    count: i16

fn stocked(items: &vec[Item]) -> i16:
    let mut total: i16 = 0
    for item in items:
        total += item.count
    return total

fn restock(items: &mut vec[Item], name: string, count: i16) -> void:
    items.push(Item(name=name, count=count))

fn main() -> i16:
    let mut items: vec[Item] = []
    restock(items, "rope", 3)
    restock(items, "lamp" + "s", 2)
    restock(items, f"torch x{4}", 1)
    print(f"{items.len} kinds, {stocked(items)} in stock")
    for item in items:
        print(f"  {item.name}: {item.count}")

    let counts = [item.count for item in items]
    let doubled = [c * 2 for c in counts]
    let mut stack: vec[i16] = [7] * 2
    stack.push(doubled[0])
    print(f"popped {stack.pop()}, {stack.len} left")

    let names: vec[string] = ["north", "east"]
    let mut route = names.copy()
    route.push("south")
    route[0] = "up"
    print(f"{names[0]} {route[0]} {route[2]}")
    return 0
