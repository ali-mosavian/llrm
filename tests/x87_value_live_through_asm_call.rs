//! An x87 value live through a block holding an x87 asm call was left on the
//! FP stack: its spill came after the call's stack inputs, which leave the
//! stack holding nothing else, and fx_pow of QB's pfpu.c at O1 failed with
//! "floating stack input is unavailable". The spill precedes the inputs.

use std::process::Command;

const IR: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

; Function Attrs: strictfp
define void @fx_pow(ptr %0, i1 %1, i1 %2) #0 {
  %4 = tail call i1 @llvm.experimental.constrained.fcmp.f80(x86_fp80 +qnan, x86_fp80 0.000000e+00, metadata !"oeq", metadata !"fpexcept.strict") #0
  %5 = tail call x86_fp80 @llvm.experimental.constrained.sitofp.f80.i32(i32 0, metadata !"round.dynamic", metadata !"fpexcept.strict") #0
  %6 = tail call i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80 0.000000e+00, x86_fp80 0.000000e+00, metadata !"ogt", metadata !"fpexcept.strict") #0
  br i1 %1, label %7, label %10

7:                                                ; preds = %3
  %8 = select i1 %2, x86_fp80 0.000000e+00, x86_fp80 +qnan
  %9 = tail call x86_fp80 @llrm.ia64.asm.d9f1.st0_st1.st0.st1.m(x86_fp80 %8, x86_fp80 0.000000e+00)
  br label %10

10:                                               ; preds = %7, %3
  store x86_fp80 %5, ptr %0, align 16
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind strictfp willreturn memory(inaccessiblemem: readwrite)
declare i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80, x86_fp80, metadata, metadata) #1

; Function Attrs: nocallback nofree nosync nounwind strictfp willreturn memory(inaccessiblemem: readwrite)
declare x86_fp80 @llvm.experimental.constrained.sitofp.f80.i32(i32, metadata, metadata) #1

; Function Attrs: nocallback nofree nosync nounwind strictfp willreturn memory(inaccessiblemem: readwrite)
declare i1 @llvm.experimental.constrained.fcmp.f80(x86_fp80, x86_fp80, metadata, metadata) #1

declare x86_fp80 @llrm.ia64.asm.d9f1.st0_st1.st0.st1.m(x86_fp80, x86_fp80)

attributes #0 = { strictfp }
attributes #1 = { nocallback nofree nosync nounwind strictfp willreturn memory(inaccessiblemem: readwrite) }
"#;

#[test]
fn a_value_live_through_an_x87_call_is_spilled_before_the_call() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("f.ll");
    std::fs::write(&input, IR).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O1", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = String::from_utf8_lossy(&done.stdout);
    let at = |needle: &str| asm.find(needle).unwrap_or_else(|| panic!("no {needle}: {asm}"));
    assert!(at("fstp tbyte ptr [rsp]") < at("db 0d9h,0f1h"), "{asm}");
}
