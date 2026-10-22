use super::lowered;
use crate::testing::{parsed, printed};

const DECLARES: &str = "declare i32 @llvm.expect.i32(i32, i32)\ndeclare i1 @llvm.expect.i1(i1, i1)\n";

fn run(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}define i32 @f(i32 %x, i1 %b) {{\nb0:\n{body}}}\n"));
    assert!(lowered(&mut module, "f").expect("runs"));
    printed(&module)
}

/// The weights `!prof` carries in `out`, as the node's operands read.
fn weights(out: &str) -> Vec<String> {
    out.lines().filter(|line| line.contains("branch_weights")).map(str::to_owned).collect()
}

/// `if (__builtin_expect(x, 1))`: the unoptimized shape clang emits. The call
/// went and the compare tests `x`; the branch has the likely weight first.
#[test]
fn a_compare_of_an_expect_call_gives_the_branch_its_weights_and_the_call_goes() {
    let out = run(
        "  %e = call i32 @llvm.expect.i32(i32 %x, i32 1)\n  %c = icmp ne i32 %e, 0\n  br i1 %c, label %t, label %f\nt:\n  ret i32 1\nf:\n  ret i32 2\n",
    );
    assert!(!out.contains("@llvm.expect.i32(i32 %x"), "{out}");
    assert!(out.contains("icmp ne i32 %x, 0"), "{out}");
    assert!(out.contains("!prof"), "{out}");
    assert_eq!(weights(&out).len(), 1, "{out}");
    assert!(weights(&out)[0].contains("2000") && weights(&out)[0].contains("i32 1"), "{out}");
}

/// Expecting 0 of a `!= 0` test makes the false edge the likely one.
#[test]
fn an_expected_zero_makes_the_false_edge_likely() {
    let out = run(
        "  %e = call i32 @llvm.expect.i32(i32 %x, i32 0)\n  %c = icmp ne i32 %e, 0\n  br i1 %c, label %t, label %f\nt:\n  ret i32 1\nf:\n  ret i32 2\n",
    );
    let node = &weights(&out)[0];
    assert!(node.find("i32 1").unwrap() < node.find("2000").unwrap(), "{out}");
}

/// A bool expected: the branch tests the call's result itself.
#[test]
fn an_expect_of_a_bool_is_the_branch_condition() {
    let out = run(
        "  %e = call i1 @llvm.expect.i1(i1 %b, i1 true)\n  br i1 %e, label %t, label %f\nt:\n  ret i32 1\nf:\n  ret i32 2\n",
    );
    assert!(out.contains("br i1 %b,"), "{out}");
    assert!(weights(&out)[0].contains("2000"), "{out}");
}

/// A switch on an expect: the expected case is the likely one, the default
/// and the other cases unlikely.
#[test]
fn a_switch_on_an_expect_call_weighs_the_expected_case() {
    let out = run(
        "  %e = call i32 @llvm.expect.i32(i32 %x, i32 2)\n  switch i32 %e, label %d [\n    i32 1, label %a\n    i32 2, label %c\n  ]\na:\n  ret i32 1\nc:\n  ret i32 2\nd:\n  ret i32 3\n",
    );
    assert!(out.contains("switch i32 %x,"), "{out}");
    let node = &weights(&out)[0];
    // default 1, case 1: 1, case 2: 2000
    let positions: Vec<usize> = node.match_indices("i32 ").map(|(at, _)| at).collect();
    assert_eq!(positions.len(), 3, "{node}");
    assert!(node.ends_with("i32 2000}") || node.contains("i32 1, i32 1, i32 2000"), "{node}");
}

/// A call whose result nothing branches on is replaced by its argument.
#[test]
fn an_unused_expect_is_replaced_by_its_argument() {
    let out = run("  %e = call i32 @llvm.expect.i32(i32 %x, i32 7)\n  %r = add i32 %e, 1\n  ret i32 %r\n");
    assert!(out.contains("add i32 %x, 1"), "{out}");
    assert!(weights(&out).is_empty(), "{out}");
}

/// The phi that defines the expected value: the edge that brings a constant
/// the call does not expect is the unlikely one.
#[test]
fn a_phi_that_defines_the_expected_value_marks_the_edge_of_the_contradicting_constant() {
    let out = run(
        "  %c = icmp slt i32 %x, 0\n  br i1 %c, label %neg, label %join\nneg:\n  br label %join\njoin:\n  %p = phi i32 [ 0, %b0 ], [ 1, %neg ]\n  %e = call i32 @llvm.expect.i32(i32 %p, i32 1)\n  ret i32 %e\n",
    );
    // The phi's constant 0 contradicts the expected 1 and comes by the edge
    // b0 -> join, the false successor: the true edge is the likely one.
    assert_eq!(weights(&out).len(), 1, "{out}");
    let node = &weights(&out)[0];
    assert!(node.find("2000").unwrap() < node.find("i32 1}").unwrap(), "{out}");
}
