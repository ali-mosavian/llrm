use llrm_mir::interpret::{Val, run};
use llrm_mir::types::FloatKind;

use super::expanded;

const TEXT: &str = "declare float @llvm.fmuladd.f32(float, float, float)\n\
    define float @f(float %a, float %b, float %c) {\nb0:\n  \
    %r = call float @llvm.fmuladd.f32(float %a, float %b, float %c)\n  ret float %r\n}\n";

/// `llvm.fmuladd` reached the old selector as a call it could not select
/// ("llvm-llc: @f: @llvm.fmuladd.f32", tests/check/c/remat_x87_argument.c at
/// `--isel=old`). It is now an `fmul` and an `fadd`, with the intrinsic's
/// answer.
#[test]
fn an_expanded_fmuladd_is_a_multiply_and_an_add() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    let mut after = before.clone();
    expanded(&mut after).expect("expands");
    let text = llrm_mir::print::module(&after);
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{text}");
    assert!(!text.contains("call float"), "fmuladd kept its call:\n{text}");
    assert!(text.contains("fmul") && text.contains("fadd"), "{text}");
    let float = |x: f32| Val::Float(FloatKind::Float, u64::from(x.to_bits()));
    let args = || vec![float(1.5), float(-2.0), float(0.25)];
    assert_eq!(run(&after, "f", args(), 1000), run(&before, "f", args(), 1000));
    assert_eq!(run(&after, "f", args(), 1000), Ok(float(-2.75)));
}
