use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `@f(x86_fp80 %x, x86_fp80 %y)` with `body` after `visitSelectInst` alone.
fn combined(body: &str) -> String {
    let text = format!("define i32 @f(x86_fp80 %x, x86_fp80 %y) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    printed(&module)
}

const NEGATE_IF_NEGATIVE: &str =
    "  %c = fcmp olt x86_fp80 %x, 0.0\n  %n = fneg x86_fp80 %x\n  %s = select i1 %c, x86_fp80 %n, x86_fp80 %x\n";

/// x_ldcmp's `(long)fabs_(v[i])`, -O2 at m32: `x < 0 ? -x : x` into `fptosi`
/// stayed a compare and a branch (a `fcom`, `fnstsw`, `sahf` and a jump a
/// trip); the pin makes it `fabs`, since `fptosi` reads neither the sign of a
/// zero nor of a NaN.
#[test]
fn a_negation_selected_by_a_sign_test_is_fabs_when_a_conversion_reads_it() {
    let out = combined(&format!("{NEGATE_IF_NEGATIVE}  %i = fptosi x86_fp80 %s to i32\n  ret i32 %i\n"));
    assert!(out.contains("call x86_fp80 @llvm.fabs.f80(x86_fp80 %x)") && !out.contains("select"), "{out}");
}

/// x_ld_abs adds the absolute value: an `fadd` reads the sign of a zero (`-0.0
/// + 0.0`), so the select stays (the pin's clang keeps `fcmp`, `fneg` and
/// `select` there too).
#[test]
fn a_negation_selected_by_a_sign_test_stays_when_an_addition_reads_the_zero() {
    let out = combined(&format!(
        "{NEGATE_IF_NEGATIVE}  %t = fadd x86_fp80 %s, %y\n  %i = fptosi x86_fp80 %t to i32\n  ret i32 %i\n"
    ));
    assert!(out.contains("select") && !out.contains("fabs"), "{out}");
}

/// `nsz` and `nnan` on the select say the same for any user.
#[test]
fn a_negation_selected_with_nsz_and_nnan_is_fabs_for_any_user() {
    let out = combined(
        "  %c = fcmp olt x86_fp80 %x, 0.0\n  %n = fneg x86_fp80 %x\n  %s = select nnan nsz i1 %c, x86_fp80 %n, x86_fp80 %x\n  %t = fadd x86_fp80 %s, %y\n  %i = fptosi x86_fp80 %t to i32\n  ret i32 %i\n",
    );
    assert!(out.contains("@llvm.fabs.f80") && !out.contains("select"), "{out}");
}

/// `x > 0 ? -x : x` is `-fabs(x)`.
#[test]
fn a_negation_selected_when_positive_is_the_negated_fabs() {
    let out = combined(
        "  %c = fcmp ogt x86_fp80 %x, 0.0\n  %n = fneg x86_fp80 %x\n  %s = select i1 %c, x86_fp80 %n, x86_fp80 %x\n  %i = fptosi x86_fp80 %s to i32\n  ret i32 %i\n",
    );
    assert!(out.contains("fneg x86_fp80 %") && out.contains("@llvm.fabs.f80") && !out.contains("select"), "{out}");
}

/// `x <= 0 ? 0.0 - x : x` needs only `nnan` evidence: the conversion gives it.
#[test]
fn a_subtraction_from_positive_zero_selected_by_a_sign_test_is_fabs() {
    let out = combined(
        "  %c = fcmp ole x86_fp80 %x, 0.0\n  %n = fsub x86_fp80 0.0, %x\n  %s = select i1 %c, x86_fp80 %n, x86_fp80 %x\n  %i = fptosi x86_fp80 %s to i32\n  ret i32 %i\n",
    );
    assert!(out.contains("@llvm.fabs.f80") && !out.contains("select"), "{out}");
}
