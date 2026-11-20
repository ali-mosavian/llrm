//! `#pragma STDC FENV_ACCESS ON` makes clang emit
//! `llvm.experimental.constrained.*` calls with `metadata !"round.dynamic"` /
//! `!"fpexcept.strict"` arguments. The parser took only `metadata !N`, so
//! QuickrBasic's `pfpu.c` stopped at "expected a constant, found Exclaim" and
//! `llrm-llc` refused the file. Each form `pfpu.c` uses must parse, verify and
//! print back as it was read.

use llrm_mir::{parse, print, verify};

const MODULE: &str = r#"
define x86_fp80 @arith(x86_fp80 %a, x86_fp80 %b, i32 %i, double %d) strictfp {
entry:
  %s = call x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80 %a, x86_fp80 %b, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %t = call x86_fp80 @llvm.experimental.constrained.fsub.f80(x86_fp80 %s, x86_fp80 %b, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %m = call x86_fp80 @llvm.experimental.constrained.fmul.f80(x86_fp80 %t, x86_fp80 %a, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %q = call x86_fp80 @llvm.experimental.constrained.fdiv.f80(x86_fp80 %m, x86_fp80 %b, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %e = call x86_fp80 @llvm.experimental.constrained.fpext.f80.f64(double %d, metadata !"fpexcept.strict") strictfp
  %f = call x86_fp80 @llvm.experimental.constrained.sitofp.f80.i32(i32 %i, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %r = call x86_fp80 @llvm.experimental.constrained.rint.f80(x86_fp80 %q, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %g = call double @llvm.experimental.constrained.fptrunc.f64.f80(x86_fp80 %r, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %n = call i32 @llvm.experimental.constrained.fptosi.i32.f80(x86_fp80 %e, metadata !"fpexcept.strict") strictfp
  %c = call i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80 %f, x86_fp80 %e, metadata !"olt", metadata !"fpexcept.strict") strictfp
  %k = call i1 @llvm.experimental.constrained.fcmp.f80(x86_fp80 %f, x86_fp80 %e, metadata !"une", metadata !"fpexcept.strict") strictfp
  ret x86_fp80 %r
}

declare x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fsub.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fmul.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fdiv.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fpext.f80.f64(double, metadata)
declare x86_fp80 @llvm.experimental.constrained.sitofp.f80.i32(i32, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.rint.f80(x86_fp80, metadata, metadata)
declare double @llvm.experimental.constrained.fptrunc.f64.f80(x86_fp80, metadata, metadata)
declare i32 @llvm.experimental.constrained.fptosi.i32.f80(x86_fp80, metadata)
declare i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80, x86_fp80, metadata, metadata)
declare i1 @llvm.experimental.constrained.fcmp.f80(x86_fp80, x86_fp80, metadata, metadata)
"#;

#[test]
fn test_every_constrained_form_pfpu_uses_parses_verifies_and_prints_back() {
    let module = parse::module(MODULE).expect("constrained calls parse");
    assert_eq!(verify::verify(&module), Vec::<String>::new());
    let printed = print::module(&module);
    for wanted in
        [r#"metadata !"round.dynamic""#, r#"metadata !"fpexcept.strict""#, r#"metadata !"olt""#, r#"metadata !"une""#]
    {
        assert!(printed.contains(wanted), "{wanted} missing:\n{printed}");
    }
    let again = print::module(&parse::module(&printed).expect("the printed text parses"));
    assert_eq!(printed, again);
}

#[test]
fn test_a_constrained_call_with_a_wrong_signature_or_an_unlisted_operation_is_refused() {
    let short = r#"
declare x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80, x86_fp80, metadata)
"#;
    let module = parse::module(short).expect("parses");
    assert!(verify::verify(&module).iter().any(|one| one.contains("incorrect argument type")));
    let unlisted = r#"
declare double @llvm.experimental.constrained.sin.f64(double, metadata, metadata)
"#;
    let module = parse::module(unlisted).expect("parses");
    assert!(verify::verify(&module).iter().any(|one| one.contains("an intrinsic MIR does not have")));
}
