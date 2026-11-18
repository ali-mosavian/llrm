//! `muls.w` reads the low word of both operands, so the `ext.l` the pin's
//! pattern puts in front of its destination is dead weight:
//! gcc's `loop_versioning` -O3 `total` loop ran an `ext.l` an iteration (4
//! clocks of 54, 500 trips) beside gcc's none, 119,520 clocks against 113,096
//! at the 68000.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DOT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@cells = global [256 x i16] zeroinitializer, align 2

define i32 @total(i16 %n) nounwind {
entry:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %loop, label %exit
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %idx = sext i16 %i to i32
  %p = getelementptr inbounds [256 x i16], ptr @cells, i32 0, i32 %idx
  %v = load i16, ptr %p, align 2
  %ve = sext i16 %v to i32
  %i1 = add nuw nsw i16 %i, 1
  %m = sext i16 %i1 to i32
  %mul = mul nsw i32 %ve, %m
  %s1 = add nsw i32 %sum, %mul
  %d = icmp eq i16 %i1, %n
  br i1 %d, label %exit, label %loop
exit:
  %r = phi i32 [ 0, %entry ], [ %s1, %loop ]
  ret i32 %r
}
"#;

const DOT_LOADS: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

define i32 @total(ptr %a, ptr %b, i32 %n) nounwind {
entry:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %loop, label %exit
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %pa = getelementptr inbounds i16, ptr %a, i32 %i
  %pb = getelementptr inbounds i16, ptr %b, i32 %i
  %va = load i16, ptr %pa, align 2
  %vb = load i16, ptr %pb, align 2
  %xa = sext i16 %va to i32
  %xb = sext i16 %vb to i32
  %m = mul nsw i32 %xa, %xb
  %s1 = add nsw i32 %sum, %m
  %i1 = add nuw nsw i32 %i, 1
  %d = icmp eq i32 %i1, %n
  br i1 %d, label %exit, label %loop
exit:
  %r = phi i32 [ 0, %entry ], [ %s1, %loop ]
  ret i32 %r
}
"#;

fn loop_body(asm: &str) -> String {
    let body = asm.split("total:").nth(1).expect("the function is emitted");
    let body = body.split("\n.size").next().unwrap_or(body);
    body.lines().skip_while(|line| !line.trim_start().starts_with(".LBB")).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_half_multiply_does_not_extend_its_destination() {
    for level in [Level::O2, Level::O3] {
        let asm = assembly(DOT, cpu_of("68000").unwrap(), level).expect("compiles");
        let body = loop_body(&asm);
        assert!(body.contains("muls"), "{level:?}: the loop is not a half multiply:\n{asm}");
        assert!(!body.contains("ext.l"), "{level:?}: `ext.l` before `muls.w`, which reads the low word only:\n{asm}");
    }
}

/// Stripping the extension off a loaded operand too turned `move.w (%a0)+, %d3;
/// ext.l %d3` into `moveq #0, %d3; move.w (%a0), %d3` and a separate `addq.l
/// #2, %a0`: `dot` -Os 114 -> 116 B, `mul_words_to_long` 114 -> 122 B, each at
/// gcc's bar.
#[test]
fn a_loaded_operand_keeps_its_post_increment() {
    for level in [Level::O2, Level::Os] {
        let asm = assembly(DOT_LOADS, cpu_of("68000").unwrap(), level).expect("compiles");
        let body = loop_body(&asm);
        let walked = body.lines().filter(|line| line.contains("(%a") && line.contains(")+")).count();
        assert_eq!(walked, 2, "{level:?}: a load lost its `(An)+`:\n{asm}");
    }
}
