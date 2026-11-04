//! A byte loaded and extended through a walking pointer was `moveq #0, %d2;
//! move.b (%a0), %d2` and an `addq.l #1, %a0` (bench/library/crc16 spent 2 B
//! more than gcc's `move.b (%a0)+, %d2`): the writeback rows covered plain
//! loads only.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn walk(extension: &str) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %p, i32 %n) {{
entry:
  br label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %q1, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %b = load i8, ptr %q
  %z = {extension} i8 %b to i32
  %s1 = add i32 %s, %z
  %q1 = getelementptr i8, ptr %q, i32 1
  %i1 = sub i32 %i, 1
  %c = icmp ne i32 %i1, 0
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s1
}}
"
    )
}

#[test]
fn a_zero_extended_byte_walk_loads_through_post_increment() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&walk("zext"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(out.contains("moveq\t#0, %d2\n\tmove.b\t(%a0)+, %d2"), "{cpu}: no cleared (An)+ load:\n{out}");
        assert!(!out.contains("addq.l\t#1, %a0"), "{cpu}: a separate step:\n{out}");
    }
}

#[test]
fn a_sign_extended_byte_walk_loads_through_post_increment() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&walk("sext"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(out.contains("move.b\t(%a0)+, %d2\n\t"), "{cpu}: no (An)+ load:\n{out}");
        assert!(!out.contains("addq.l\t#1, %a0"), "{cpu}: a separate step:\n{out}");
    }
}
