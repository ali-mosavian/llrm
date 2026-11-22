//! A `select` is a branch on the m68k (`M68kISD::CMOV` is a diamond,
//! `EmitLoweredSelect`; no conditional move). The target said it had one, so a
//! destination with three phis was folded into selects (the pin's limit of
//! three) where targets without a conditional move keep two: both branches of
//! the common destination stay. `Tti::has_cmov` was never overridden.

use llrm_m68k::driver::{Level, assembly_dumping, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i32 @f(i1 %a, i1 %b, i32 %x, i32 %y, i32 %z) {\n\
b0:\n  br i1 %a, label %d, label %b1\n\
b1:\n  br i1 %b, label %d, label %e\n\
d:\n\
  %p = phi i32 [ %x, %b0 ], [ %y, %b1 ]\n\
  %q = phi i32 [ %y, %b0 ], [ %z, %b1 ]\n\
  %r = phi i32 [ %z, %b0 ], [ %x, %b1 ]\n\
  %t1 = add i32 %p, %q\n  %t2 = add i32 %t1, %r\n  ret i32 %t2\n\
e:\n  ret i32 0\n}\n";

/// `br i1` of `@f` after the mid-end.
fn conditional_branches(cpu: &str) -> usize {
    let dir = std::env::temp_dir().join(format!("llrm-no-cmov-{cpu}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assembly_dumping(IR, cpu_of(cpu).unwrap(), Level::O2, Some(&dir)).expect("compiles");
    let mut stages = Vec::new();
    let mut pending = vec![dir.clone()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "ll") {
                stages.push(path);
            }
        }
    }
    stages.sort();
    let last = std::fs::read_to_string(stages.last().expect("a stage")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    last.matches("br i1").count()
}

#[test]
fn a_destination_with_three_phis_keeps_its_branches() {
    for cpu in ["68000", "68020", "68040"] {
        assert_eq!(conditional_branches(cpu), 2, "{cpu}: both branches of the common destination stay");
    }
}
