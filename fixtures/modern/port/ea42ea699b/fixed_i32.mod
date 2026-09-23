type scalar = fixed i32, fraction=9
fn product(left: scalar, right: scalar) -> scalar:
    return left * right
fn quotient(left: scalar, right: scalar) -> scalar:
    return left / right
fn main() -> i16:
    print(quotient(1, 0.001953125))
    print(quotient(-1, 0.001953125))
    print(quotient(4194303.998046875, 0.001953125))
    print(quotient(-4194304, -1))
    print(product(100000, 0.5))
    return 0
