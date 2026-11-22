//! The m68k mid-end runs once per module, not once per function.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module_of(functions: usize) -> String {
    let mut ir = format!("target datalayout = \"{DATALAYOUT}\"\ntarget triple = \"m68k-unknown-linux-gnu\"\n");
    for n in 0..functions {
        ir.push_str(&format!(
            "define i32 @f{n}(ptr %p, i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [ 0, %entry ], [ %j, %l ]\n  \
             %s = phi i32 [ 0, %entry ], [ %t, %l ]\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  \
             %t = add i32 %s, %v\n  %j = add i32 %i, 1\n  %c = icmp slt i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %t\n}}\n"
        ));
    }
    ir
}

/// `assembly` ran the mid-end over the whole module once for each function it
/// printed: memory_expansion (1614 blocks, ~40 functions) took 294 s of CPU,
/// 25,000 liveness solves. The test counted `liveness::solves()`, which the
/// mid-end stopped calling: it read 0 solves and passed on any behaviour
/// (the guard `four > 0` failed instead). It counts the runs of the mid-end
/// itself now: one per module, however many functions.
#[test]
fn the_mid_end_is_not_run_again_for_each_function() {
    let runs = |functions: usize| {
        let before = llrm_m68k::compile::mid_end_runs();
        assembly(&module_of(functions), Cpu::M68020, Level::O2).expect("compiles");
        llrm_m68k::compile::mid_end_runs() - before
    };
    let (four, eight) = (runs(4), runs(8));
    assert!(four > 0, "the mid-end never ran: the counter measures nothing");
    assert_eq!((four, eight), (1, 1), "four functions {four} runs, eight {eight}");
}
