use super::{Executed, run};
use crate::frontends::nib;
use crate::hir::codec;
use crate::hir::model::Number;

fn execute(source: &str, entry: &str, arguments: &[Number]) -> Executed {
    let json = nib::compile(source, "t").expect("compiles");
    let program = codec::decode(&json).expect("decodes");
    run(&program, entry, arguments).expect("runs")
}

fn value(source: &str, entry: &str, arguments: &[i64]) -> Option<Number> {
    let arguments: Vec<Number> = arguments.iter().map(|one| Number::Int(*one)).collect();
    execute(source, entry, &arguments).value
}

#[test]
fn arithmetic_wraps_at_the_declared_width() {
    let source = "fn f(a: i16, b: i16) -> i16:\n    return a * b + 7\n";
    assert_eq!(value(source, "f", &[300, 300]), Some(Number::Int(24_471)));
    assert_eq!(value(source, "f", &[-3, 4]), Some(Number::Int(-5)));
}

#[test]
fn floor_division_rounds_down_and_remainder_keeps_the_dividends_sign() {
    let source = "fn f(a: i16, b: i16) -> i16:\n    return (a // b) * 100 + a % b\n";
    assert_eq!(value(source, "f", &[-7, 2]), Some(Number::Int(-401)));
    assert_eq!(value(source, "f", &[7, -2]), Some(Number::Int(-399)));
    assert_eq!(value(source, "f", &[7, 2]), Some(Number::Int(301)));
    assert_eq!(value(source, "f", &[-8, 2]), Some(Number::Int(-400)));
}

#[test]
fn loops_and_internal_calls_run() {
    let source = include_str!("../../fixtures/nib/control.nib");
    assert_eq!(value(source, "count", &[12]), Some(Number::Int(11)));
}

#[test]
fn arrays_fill_index_and_iterate() {
    let source = "fn filled(seed: i16) -> i16:\n    let mut cells: i16[9] = [seed * 3] * 9\n    cells[4] = 1\n    \
                  let mut total: i16 = 0\n    for cell in cells:\n        total += cell\n    return total\n";
    assert_eq!(value(source, "filled", &[5]), Some(Number::Int(121)));
}

#[test]
fn struct_copies_have_value_semantics() {
    let source = "struct point:\n    mut x: i16\n    y: i16\nfn calculate() -> i16:\n    \
                  let mut current: point = point(x=1, y=2)\n    let snapshot = current\n    \
                  current = point(x=current.y, y=current.x)\n    current.x += snapshot.x\n    \
                  return current.x * 10 + current.y\n";
    assert_eq!(value(source, "calculate", &[]), Some(Number::Int(31)));
}

#[test]
fn print_captures_text_integers_floats_and_fixed() {
    let source = "fn main() -> i16:\n    let mut x: f64 = 0.1\n    print(\"hi\")\n    print(f\"{-5} {x * 3.0}\")\n    return 0\n";
    assert_eq!(
        execute(source, "main", &[]).output,
        "hi\n-5 0.30000000000000004\n"
    );
    let fixed = include_str!("../../fixtures/nib/fixed.nib");
    let executed = execute(fixed, "fixed_literals", &[]);
    assert_eq!(executed.output, "2.25\nfixed=2.25\n");
    assert_eq!(executed.value, Some(Number::Int(147_456)));
}

#[test]
fn nbody_matches_the_python_reference() {
    let executed = execute(
        include_str!("../../fixtures/nib/nbody.nib"),
        "nbody",
        &[Number::Int(1)],
    );
    assert!(
        executed
            .output
            .starts_with("PX=-14.896484375\nPY=-11.92578125\nVX=0.103515625\n")
    );
    assert!(
        executed
            .output
            .ends_with("VX=-0.103515625\nVY=-0.07421875\nDONE\n")
    );
    assert_eq!(executed.value, Some(Number::Int(-10_177)));
}
