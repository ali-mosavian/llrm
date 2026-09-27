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
fn _body(op: &str, constant: i64, test: &str) -> Module {
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

fn returned(module: &Module, x: u128) -> Val {
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
