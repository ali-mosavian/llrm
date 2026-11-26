//! `Tti::memory_idioms_at_optsize`: a size build keeps a store or copy loop
//! of unknown length, since the call is more bytes than the loop, and calls
//! the library for a constant length, where the loop ran many times the
//! clocks. Speed builds call either way.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const MEMSET: &str = include_str!("../../bench/library/memset.ll");
const MEMCPY: &str = include_str!("../../bench/library/memcpy.ll");

#[test]
fn a_size_build_keeps_its_fill_and_copy_loops() {
    for cpu in ["68000", "68020"] {
        for (name, ir, routine) in [("memset", MEMSET, "memset"), ("memcpy", MEMCPY, "memcpy")] {
            let out = assembly(ir, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
            assert!(!out.contains(&format!("jsr\t{routine}")), "{cpu} {name}: a library call at -Os:\n{out}");
        }
    }
}

#[test]
fn a_size_build_calls_the_library_for_a_loop_of_constant_length() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
@a = global [2000 x i16] zeroinitializer, align 2
@b = global [2000 x i16] zeroinitializer, align 2
define void @keep() optsize {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %from = getelementptr inbounds i16, ptr @a, i32 %i
  %cell = load i16, ptr %from, align 2
  %to = getelementptr inbounds i16, ptr @b, i32 %i
  store i16 %cell, ptr %to, align 2
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 2000
  br i1 %done, label %exit, label %loop
exit:
  ret void
}";
    for cpu in ["68000", "68020"] {
        let out = assembly(ir, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(out.contains("memcpy"), "{cpu}: the copy stayed a loop:\n{out}");
    }
}

#[test]
fn a_speed_build_still_calls_the_library_for_a_fill() {
    let out = assembly(MEMSET, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(out.contains("memset"), "-O2 no longer calls memset:\n{out}");
}
