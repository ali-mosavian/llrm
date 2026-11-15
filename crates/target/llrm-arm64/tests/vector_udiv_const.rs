//! An unsigned divide of a vector by a constant is the multiply-high
//! sequence (`BuildUDIV`), as the pin's `umull2; umull; uzp2; ushr` and a
//! multiply and subtract (`mls` is the machine combiner's, not yet here).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(body: &str) -> String {
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q) {{
  %x = load <4 x i32>, ptr %p, align 16
  {body}
  store <4 x i32> %r, ptr %q, align 16
  ret void
}}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// `urem <4 x i32> %x, 251` was `CannotSelect("URem")`: no vector loop with a
/// remainder by a constant compiled (`huge` -O2 330,025 instructions against
/// 173,637).
#[test]
fn a_v4i32_remainder_by_a_constant_is_a_multiply_high() {
    let asm = assembly("%r = urem <4 x i32> %x, <i32 251, i32 251, i32 251, i32 251>");
    for wanted in ["umull2", "umull ", "uzp2", "ushr"] {
        assert!(asm.contains(wanted), "{wanted}: {asm}");
    }
    assert!(!asm.contains("udiv"), "{asm}");
}

/// A divisor with the add fix-up (7) selects `usra`, which had no encoding:
/// "machine opcode USRAv4i32_shift has no arm64 instruction yet".
#[test]
fn a_v4i32_quotient_by_a_constant_with_a_fixup_is_a_multiply_high_and_usra() {
    let asm = assembly("%r = udiv <4 x i32> %x, <i32 7, i32 7, i32 7, i32 7>");
    assert!(asm.contains("umull2") && asm.contains("uzp2") && asm.contains("usra"), "{asm}");
}

#[test]
fn a_v4i32_quotient_by_a_power_of_two_is_a_shift() {
    let asm = assembly("%r = udiv <4 x i32> %x, <i32 8, i32 8, i32 8, i32 8>");
    assert!(asm.contains("ushr"), "{asm}");
    assert!(!asm.contains("umull"), "{asm}");
}

/// The loop's `<8 x i32>` remainder was `Unsupported { op: URem, ty: V8I32,
/// "split result" }`: the legalizer split every elementwise operation but a
/// divide (`huge` -O2 would not compile once its init loop was widened).
#[test]
fn a_v8i32_remainder_by_a_constant_splits_into_two_multiply_highs() {
    let ir = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q) {
  %x = load <8 x i32>, ptr %p, align 16
  %r = urem <8 x i32> %x, <i32 251, i32 251, i32 251, i32 251, i32 251, i32 251, i32 251, i32 251>
  store <8 x i32> %r, ptr %q, align 16
  ret void
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert_eq!(asm.matches("umull2").count(), 2, "{asm}");
    assert!(!asm.contains("udiv"), "{asm}");
}

/// `dst[i] = src[i] % divisor` over `n` elements.
fn remainder_loop(divisor: &str) -> String {
    let ir = format!(
        r#"target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @f(ptr noalias %d, ptr noalias %s, i64 %n, i32 %v) {{
entry:
  %c = icmp eq i64 %n, 0
  br i1 %c, label %exit, label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %sp = getelementptr inbounds i32, ptr %s, i64 %i
  %x = load i32, ptr %sp, align 4
  %r = urem i32 %x, {divisor}
  %dp = getelementptr inbounds i32, ptr %d, i64 %i
  store i32 %r, ptr %dp, align 4
  %i.next = add nuw i64 %i, 1
  %e = icmp eq i64 %i.next, %n
  br i1 %e, label %exit, label %loop
exit:
  ret void
}}
"#
    );
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

/// LoopVectorize refused `urem` ("A binary operation the vectorizer does not
/// widen yet"): `huge`'s `(r * 201 + c) % 251` init loop stayed scalar, 281,400
/// of its 330,025 executed instructions.
#[test]
fn a_loop_remainder_by_a_constant_is_vectorized() {
    let asm = remainder_loop("251");
    assert!(asm.contains("umull2") && asm.contains("uzp2"), "{asm}");
}

/// A divisor that is not a constant may be zero in a lane the scalar loop
/// never reaches: the loop stays scalar.
#[test]
fn a_loop_remainder_by_a_variable_stays_scalar() {
    let asm = remainder_loop("%v");
    assert!(!asm.contains("umull2"), "{asm}");
}
