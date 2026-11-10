//! The driver runs no mid-end (second LSR) after the pipeline.

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

/// `assembly` ran a second loop strength reduction (`compile::mid_end`) after
/// the pipeline's own, which had already run LoopTermFold: with the fold
/// enabled, partition's pointer-compare exit became a count-to-zero again
/// (98 B, as with the fold off). The driver runs no mid-end after the
/// pipeline now. (Earlier form: one run per module, not per function, after
/// 294 s of CPU on memory_expansion.)
#[test]
fn the_driver_runs_no_second_mid_end_after_the_pipeline() {
    let runs = |functions: usize| {
        let before = llrm_m68k::compile::mid_end_runs();
        assembly(&module_of(functions), Cpu::M68020, Level::O2).expect("compiles");
        llrm_m68k::compile::mid_end_runs() - before
    };
    let (four, eight) = (runs(4), runs(8));
    assert_eq!((four, eight), (0, 0), "four functions {four} runs, eight {eight}");
}
