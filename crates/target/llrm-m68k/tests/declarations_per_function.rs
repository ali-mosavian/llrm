//! The pipeline lists the module's declarations a few times, not once per function.

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

/// `Walk` asked for the module's `Declarations` once per function, and the
/// attributes inferred for each function changed them each time: every global's
/// declaration was made again for every function (83, 227, 707 and 2435 made
/// for 4, 8, 16 and 32 functions). It asks for the names now, which an attribute
/// does not change.
#[test]
fn declarations_are_not_listed_again_for_each_function() {
    let made = |functions: usize| {
        let before = llrm_mir::module::declarations_made();
        assembly(&module_of(functions), Cpu::M68020, Level::O2).expect("compiles");
        llrm_mir::module::declarations_made() - before
    };
    let (eight, sixteen, thirty_two) = (made(8), made(16), made(32));
    println!("8: {eight}, 16: {sixteen}, 32: {thirty_two}");
    assert!(thirty_two <= 2 * sixteen + 64 && sixteen <= 2 * eight + 64, "8: {eight}, 16: {sixteen}, 32: {thirty_two}");
}
