//! `select (fcmp ugt <8 x float> v, 0), t, 0` on bytes is one mask `bic` of
//! the truncated values (pin `combineVSelectWithAllOnesOrZeros`); with no
//! `VSELECT` combine the zero arm stayed: a `movi`, `mvn` x2, `and`, `bic`,
//! `orr` (17 instructions against the pin's 11), +46,080 executed
//! instructions on jpeg_single -O2. Still 13: the pin hoists the `not` above
//! the truncates and concat (DEVIATE VSEL2) and makes one `bic`.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const IR: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @ugt(ptr %p, ptr %o) {
  %v = load <8 x float>, ptr %p, align 4
  %c = fcmp ugt <8 x float> %v, zeroinitializer
  %i = fptosi <8 x float> %v to <8 x i32>
  %t = trunc <8 x i32> %i to <8 x i8>
  %s = select <8 x i1> %c, <8 x i8> %t, <8 x i8> zeroinitializer
  store <8 x i8> %s, ptr %o, align 1
  ret void
}
"#;

fn instructions(asm: &str) -> usize {
    asm.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('.') && !line.ends_with(':')).count()
}

#[test]
fn a_vector_select_with_a_zero_arm_is_one_mask_and() {
    let asm = driver::assembly(IR, Platform::Linux, Level::O2).expect("lowers");
    assert!(!asm.contains("movi"), "{asm}");
    assert!(!asm.contains("orr"), "{asm}");
    assert!(instructions(&asm) <= 13, "{asm}");
}

const FREEZE_IR: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %q, ptr %o) {
  %w = load <4 x float>, ptr %q, align 4
  %i = fptosi <4 x float> %w to <4 x i32>
  %x = freeze <4 x i32> %i
  store <4 x i32> %x, ptr %o, align 4
  ret void
}
define void @g(ptr %p, ptr %q, ptr %o) {
  %v = load <8 x i8>, ptr %p, align 1
  %w = load <8 x float>, ptr %q, align 4
  %i = fptosi <8 x float> %w to <8 x i32>
  %t = trunc <8 x i32> %i to <8 x i8>
  %x = freeze <8 x i8> %t
  %s = and <8 x i8> %v, %x
  store <8 x i8> %s, ptr %o, align 1
  ret void
}
"#;

/// A vector `freeze` of a value that may be poison stopped selection: a
/// `COPY` of a vector took the general class ("STRQui: a store of something
/// that is no register") and a `<8 x i32>` freeze had no split.
#[test]
fn a_vector_freeze_of_a_possibly_poison_value_lowers() {
    let asm = driver::assembly(FREEZE_IR, Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("fcvtzs"), "{asm}");
}

/// `select i1 c, <4 x i32> a, <4 x i32> b` (the mid-end makes one of a
/// vector select on a splat condition) stopped lowering: "Unsupported { op:
/// Select, ty: V4I32, what: convert node to libcall" (the dag_debug loop).
#[test]
fn a_vector_select_on_a_scalar_condition_lowers() {
    let ir = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(i1 %c, ptr %p, ptr %q, ptr %o) {
  %a = load <4 x i32>, ptr %p, align 16
  %b = load <4 x i32>, ptr %q, align 16
  %s = select i1 %c, <4 x i32> %a, <4 x i32> %b
  store <4 x i32> %s, ptr %o, align 16
  ret void
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, Level::O2).expect("lowers");
    assert!(asm.contains("and") || asm.contains("bsl") || asm.contains("bit") || asm.contains("bif"), "{asm}");
}
