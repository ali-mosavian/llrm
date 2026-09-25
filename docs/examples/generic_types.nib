# Generic structs and enums: an instance per type argument list, chosen by
# the brackets, the field values, or the type expected.

enum Slot[T]:
    empty
    held(T)

struct Pair[A, B]:
    first: A
    second: B

struct Stack[T]:
    mut items: vec[T]

fn Stack.top(self: &Stack[T]) -> Slot[T]:
    if self.items.len == 0:
        return .empty
    return .held(self.items[self.items.len - 1])

fn larger[T](a: T, b: T) -> T:
    return a > b ? a : b

fn first[T](values: &[T]) -> Option[&T]:
    match values:
        []:
            return .none
        [head, *_]:
            return .some(head)

enum Fault:
    full

fn store(stack: &mut Stack[i16], value: i16) -> Result[void, Fault]:
    if stack.items.len == 3:
        return .err(.full)
    stack.items.push(value)
    return Result.ok()

fn main() -> i16:
    let score = Pair(first="ada", second=42)
    let flag: Pair[u8, bool] = Pair[u8, bool](first=7, second=true)
    print(f"{score.first} {score.second} {flag.first} {flag.second}")
    let mut stack = Stack(items=[5, 9])
    match stack.top():
        .held(value):
            print(f"top {value}")
        .empty:
            print("empty")
    for value in [11, 12, 13]:
        match store(stack, value):
            .ok(_):
                print(f"stored {value}")
            .err(_):
                print(f"full at {value}")
    let low: u8 = 3
    let high: u8 = 200
    print(f"{larger(low, high)} {larger(-1, -5)}")
    let rolls: i16[3] = [4, 6, 1]
    match first(rolls):
        .some(roll):
            print(f"first roll {roll}")
        .none:
            print("no rolls")
    let nothing: Slot[i16] = Slot.empty
    match nothing:
        .held(_):
            print("held")
        .empty:
            print("nothing held")
    return 0
