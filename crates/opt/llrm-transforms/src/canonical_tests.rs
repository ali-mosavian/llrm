//! Adapted from llrm-core's `optimize/canonical_tests.rs`, the port of
//! `tests/test_canonical.py`, each body now MIR text run by llrm-mir's
//! interpreter.
//!
//! Skipped: test_a_folded_term_keeps_the_source_bytes_it_owned -- MIR
//! instructions own no raised source bytes.

use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;
use llrm_mir::opcode::{IntPredicate, Opcode};

use super::identities;

/// `t = x op constant; if t test 0 return 1 else return 2`, x an argument.
fn _body(
    op: &str,
    constant: i64,
    test: &str,
) -> Module {
    let text = format!(
        "define i16 @f(i16 %x) {{
b0:
  %t = {op} i16 %x, {constant}
  %c = icmp {test} i16 %t, 0
  br i1 %c, label %b1, label %b2

b1:
  ret i16 1

b2:
  ret i16 2
}}
"
    );
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn returned(
    module: &Module,
    x: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: x, width: 16 }], 1_000).expect("runs")
}

/// Rotation's `bound - 0 + 0` and `bound <=u 0` used to be folded inside the rewrite that wrote them.
#[test]
fn test_a_neutral_term_is_its_operand_and_a_test_below_zero_is_equality() {
    for (op, constant) in [("add", 0), ("sub", 0), ("mul", 1)] {
        for (test, equality) in [("ule", IntPredicate::Eq), ("ugt", IntPredicate::Ne)] {
            let body = _body(op, constant, test);
            let mut folded = body.clone();
            let (context, function) = folded.function_mut("f").expect("@f");
            assert!(identities(context, function));
            let entry = function.block(function.entry().unwrap()).instructions();
            let opcodes = entry.iter().map(|&one| function.instruction(one).opcode.clone()).collect::<Vec<_>>();
            assert_eq!(opcodes, [Opcode::ICmp(equality), Opcode::Br]);
            assert_eq!(llrm_mir::verify::verify(&folded), Vec::<String>::new());
            for x in [0, 1, 0xFFFF] {
                assert_eq!(returned(&folded, x), returned(&body, x), "{op} {test} {x}");
            }
        }
    }
}

#[test]
fn test_a_term_that_is_not_neutral_stays() {
    let mut body = _body("sub", 1, "ult");
    let before = llrm_mir::print::module(&body);
    let (context, function) = body.function_mut("f").expect("@f");
    assert!(!identities(context, function));
    assert_eq!(llrm_mir::print::module(&body), before);
}

/// `text` through `Canonical` under the pass manager's verifier and
/// preserved-analyses check.
fn canonical(text: &str) -> (Module, Module) {
    let before = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let mut module = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(super::Canonical);
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    (before, module)
}

fn printed(module: &Module) -> String {
    llrm_mir::print::module(module)
}

#[test]
fn a_constant_on_the_left_of_any_compare_moves_right_with_the_same_answer() {
    for predicate in ["eq", "ne", "ult", "ule", "ugt", "uge", "slt", "sle", "sgt", "sge"] {
        let text = format!(
            "define i16 @f(i16 %x) {{
b0:
  %c = icmp {predicate} i16 5, %x
  %r = zext i1 %c to i16
  ret i16 %r
}}
"
        );
        let (before, after) = canonical(&text);
        let compare = printed(&after).lines().find(|line| line.contains("icmp")).unwrap().to_owned();
        assert!(compare.ends_with("i16 %x, 5"), "{predicate}: {compare}");
        for x in [0, 4, 5, 6, 0x8000, 0xFFFF] {
            assert_eq!(returned(&after, x), returned(&before, x), "{predicate} {x}");
        }
    }
}

#[test]
fn a_compare_of_two_constants_or_two_values_is_left_alone() {
    let text = "define i1 @f(i16 %x) {
b0:
  %k = icmp slt i16 3, 4
  %v = icmp slt i16 %x, %x
  %r = and i1 %k, %v
  ret i1 %r
}
";
    let (before, after) = canonical(text);
    assert_eq!(printed(&after), printed(&before));
}

#[test]
fn a_constant_zero_on_the_left_of_uge_becomes_an_equality_test() {
    let (before, after) = canonical(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp uge i16 0, %x
  %r = zext i1 %c to i16
  ret i16 %r
}
",
    );
    assert!(printed(&after).contains("icmp eq i16 %x, 0"), "{}", printed(&after));
    for x in [0, 1, 0xFFFF] {
        assert_eq!(returned(&after, x), returned(&before, x), "{x}");
    }
}

#[test]
fn neutral_terms_fold_on_either_side_but_zero_minus_x_stays() {
    let (before, after) = canonical(
        "define i16 @f(i16 %x) {
b0:
  %a = add i16 0, %x
  %m = mul i16 1, %a
  %n = sub i16 0, %m
  ret i16 %n
}
",
    );
    assert_eq!(
        printed(&after),
        "define i16 @f(i16 %x) {
b0:
  %n = sub i16 0, %x
  ret i16 %n
}
"
    );
    for x in [0, 1, 7, 0xFFFF] {
        assert_eq!(returned(&after, x), returned(&before, x), "{x}");
    }
}

#[test]
fn signed_and_other_tests_against_zero_are_left_alone() {
    for test in ["slt", "sle", "sgt", "sge", "ult", "uge", "eq", "ne"] {
        let body = _body("add", 1, test);
        let mut folded = body.clone();
        let (context, function) = folded.function_mut("f").expect("@f");
        assert!(!identities(context, function), "{test}");
        assert_eq!(printed(&folded), printed(&body), "{test}");
    }
}

/// A zero test of an extension tests what was extended, at its width; of an
/// `i1`, it is the `i1` or its complement. None was rewritten, so a loop
/// exiting on a frontend's sign-extended truth was never counted.
#[test]
fn a_zero_test_of_an_extension_tests_what_was_extended() {
    for (from, ext, predicate, left) in [
        ("i1", "sext", "ne", "%c"),
        ("i1", "zext", "ne", "%c"),
        ("i1", "sext", "eq", "xor i1 %c, true"),
        ("i1", "zext", "eq", "xor i1 %c, true"),
        ("i8", "sext", "eq", "icmp eq i8 %c, 0"),
        ("i8", "zext", "ne", "icmp ne i8 %c, 0"),
    ] {
        let text = format!(
            "define i16 @f(i16 %x) {{
b0:
  %c = trunc i16 %x to {from}
  %e = {ext} {from} %c to i16
  %t = icmp {predicate} i16 %e, 0
  %r = zext i1 %t to i16
  ret i16 %r
}}
"
        );
        let (before, after) = canonical(&text);
        let shown = printed(&after);
        assert!(!shown.contains("icmp ne i16") && !shown.contains("icmp eq i16"), "{ext} {from} {predicate}:\n{shown}");
        assert!(
            shown.contains(left) || shown.contains(&format!("zext i1 {left} to i16")),
            "{ext} {from} {predicate}:\n{shown}"
        );
        for x in [0, 1, 2, 0x80, 0xFF, 0x100, 0xFFFF] {
            assert_eq!(returned(&after, x), returned(&before, x), "{ext} {from} {predicate} {x}");
        }
    }
}

/// The HIR's loop test, `icmp ne (sext i1 %go), 0`: once canonical, the
/// counter is induction's and Rotate enters the loop at its body.
#[test]
fn a_loop_on_a_sign_extended_truth_is_rotated_once_canonical() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b2 ]
  %go = icmp slt i16 %i, 4
  %truth = sext i1 %go to i16
  %test = icmp ne i16 %truth, 0
  br i1 %test, label %b2, label %b3

b2:
  %sum = add i16 %acc, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    let before = llrm_mir::parse::module(text).unwrap();
    let mut module = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.verify_each = true;
    passes.add(super::Canonical);
    passes.add(crate::rotate::Rotate { proven: true, copy: false });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    let shown = printed(&module);
    let entry = shown.split("b0:\n").nth(1).unwrap().lines().next().unwrap();
    assert_eq!(entry.trim(), "br label %b2", "{shown}");
    for x in [0, 7, 0xFFFF] {
        assert_eq!(returned(&module, x), returned(&before, x), "{x}");
    }
}
