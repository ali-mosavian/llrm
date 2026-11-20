//! The call-target choice expanded and priced the whole function once per
//! repeated callee, though a callee called six times cannot make a speed
//! build cheaper (`lea` and a saved register cost more clocks than six `jsr`
//! through a register save): `choose` was 23% of grep -O2's compile.

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::expand::expansions;

const HEADER: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @g(i32)
declare void @g1(i32)
declare void @g2(i32)
declare void @g3(i32)
declare void @g4(i32)
declare void @g5(i32)
declare void @g6(i32)
";

fn function(callees: [&str; 6]) -> String {
    let body: String =
        callees.iter().enumerate().map(|(n, callee)| format!("  call void @{callee}(i32 {n})\n")).collect();
    format!("{HEADER}define void @f() {{\nentry:\n{body}  ret void\n}}\n")
}

fn expanded(ir: &str) -> usize {
    let before = expansions();
    assembly(ir, cpu_of("68020").unwrap(), Level::O1).expect("compiles");
    expansions() - before
}

#[test]
fn a_repeated_callee_that_cannot_pay_at_a_speed_level_is_not_priced() {
    let repeated = expanded(&function(["g"; 6]));
    let distinct = expanded(&function(["g1", "g2", "g3", "g4", "g5", "g6"]));
    // the one expansion the choice needs to know the function's price; the
    // callee's own trial (3 more) is what the bound spares
    let extra = repeated.saturating_sub(distinct);
    assert!(extra <= 1, "{extra} extra expansions for the one repeated callee");
}
