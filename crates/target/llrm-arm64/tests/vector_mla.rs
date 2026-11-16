//! A lane-wise product added to or subtracted from a register is `mla` or
//! `mls` (`MULADDv_OP1/OP2`, `MULSUBv_OP2` of the machine combiner). A
//! `mul; sub` pair was left as two instructions (`recsum` -O2, 4 a trip).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(
    lanes: &str,
    body: &str,
) -> String {
    let (n, bytes) = if lanes == "4" { (4, 16) } else { (2, 8) };
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q, ptr %r, ptr %o) {{
  %x = load <{n} x i32>, ptr %p, align {bytes}
  %y = load <{n} x i32>, ptr %q, align {bytes}
  %z = load <{n} x i32>, ptr %r, align {bytes}
  %m = mul <{n} x i32> %x, %y
  {body}
  store <{n} x i32> %t, ptr %o, align {bytes}
  ret void
}}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

#[test]
fn a_product_added_to_a_register_is_mla() {
    for lanes in ["4", "2"] {
        let asm = assembly(lanes, &format!("%t = add <{lanes} x i32> %z, %m"));
        assert!(asm.contains("mla") && !asm.contains("mul "), "{asm}");
    }
}

#[test]
fn a_product_subtracted_from_a_register_is_mls() {
    for lanes in ["4", "2"] {
        let asm = assembly(lanes, &format!("%t = sub <{lanes} x i32> %z, %m"));
        assert!(asm.contains("mls") && !asm.contains("mul "), "{asm}");
    }
}

/// `sub t, c` is `neg` of the register and `mla` (`MULSUBv_OP1`).
#[test]
fn a_register_subtracted_from_a_product_is_neg_and_mla() {
    let asm = assembly("4", "%t = sub <4 x i32> %m, %z");
    assert!(asm.contains("neg ") && asm.contains("mla") && !asm.contains("mul "), "{asm}");
}

/// A product read twice cannot be folded away.
#[test]
fn a_product_read_twice_is_not_fused() {
    let asm = assembly("4", "%a = add <4 x i32> %z, %m\n  %t = add <4 x i32> %a, %m");
    assert!(asm.contains("mul ") && !asm.contains("mla"), "{asm}");
}
