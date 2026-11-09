//! A call of an `llvm.*` intrinsic nothing lowers was emitted as a call to a
//! symbol of that name: `llvm.frameaddress.p0` became `jmp
//! llvm.frameaddress.p0` and failed only at link. The 6502 names the intrinsic
//! and refuses.

use llrm_6502::driver::{Level, Request, assembly};

const FRAMEADDRESS: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"

declare ptr @llvm.frameaddress.p0(i32)

define ptr @f() {
entry:
  %p = call ptr @llvm.frameaddress.p0(i32 0)
  ret ptr %p
}
";

#[test]
fn an_intrinsic_nothing_lowers_is_refused_by_name_at_every_level() {
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let error = assembly(FRAMEADDRESS, &Request::new(level)).unwrap_err();
        assert!(error.contains("llvm.frameaddress"), "{level:?}: {error}");
    }
}

#[test]
fn a_lowered_intrinsic_still_compiles() {
    let text = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"

declare i8 @llvm.ctpop.i8(i8)

define i8 @f(i8 %x) {
entry:
  %n = call i8 @llvm.ctpop.i8(i8 %x)
  ret i8 %n
}
";
    assert!(assembly(text, &Request::new(Level::O2)).is_ok());
}
