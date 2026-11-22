//! -Os: a local function called once, from a cold arm, is inlined (gcc's
//! `-finline-functions-called-once`). The pin drops the `LastCallToStatic`
//! bonus at a cold site, so `big` stayed a function and its call and frame
//! cost bytes (QuickrBasic m68k Os: -1404 B on 68000, -1456 B on 68020).

use std::process::Command;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare i32 @a(i32)
declare i32 @b(i32)
declare i32 @c(i32)
declare i32 @d(i32)
define internal i32 @big(i32 %x) nounwind optsize {
  %r0 = call i32 @a(i32 %x)
  %b0 = call i32 @b(i32 %r0)
  %r1 = add i32 %r0, %b0
  %c0 = call i32 @c(i32 %r1)
  %c3 = mul i32 %c0, 3
  %r2 = add i32 %r1, %c3
  %big = icmp sgt i32 %r2, 100
  br i1 %big, label %sub, label %tail
sub:
  %d0 = call i32 @d(i32 %r2)
  %r3 = sub i32 %r2, %d0
  br label %tail
tail:
  %r4 = phi i32 [ %r2, %0 ], [ %r3, %sub ]
  %a1 = call i32 @a(i32 %r4)
  %b1 = call i32 @b(i32 %r4)
  %c1 = call i32 @c(i32 %x)
  %s1 = add i32 %r4, %a1
  %s2 = add i32 %s1, %b1
  %s3 = add i32 %s2, %c1
  ret i32 %s3
}
define i32 @run(i32 %k, i32 %v) nounwind optsize {
  %cold = icmp eq i32 %k, 3
  br i1 %cold, label %cold_arm, label %hot_arm, !prof !0
cold_arm:
  %x = call i32 @big(i32 %v)
  ret i32 %x
hot_arm:
  %y = call i32 @a(i32 %v)
  ret i32 %y
}
!0 = !{!\"branch_weights\", i32 1, i32 4000}
";

fn assembly(level: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, IR).unwrap();
    let said =
        Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc")).args(["-S", level, "-o", "-"]).arg(&input).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(said.stdout).unwrap()
}

#[test]
fn a_function_called_once_from_a_cold_arm_is_inlined_at_size() {
    let text = assembly("-Os");
    assert!(!text.contains("big:"), "{text}");
}

/// The knob is for a caller optimized for size: -O2 keeps the pin's choice.
#[test]
fn the_pins_cold_site_rule_stands_at_speed() {
    let text = assembly("-O2");
    assert!(text.contains("big:"), "{text}");
}
