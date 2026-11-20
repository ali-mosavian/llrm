//! One loop storing to eight arrays kept all eight as `(An)+` walks, six
//! address registers' worth: `early_exit_widths` `reset` -O1 spilled and
//! reloaded four pointers and shuffled three more between `a` registers every
//! trip (+22% clocks over gcc at 68020). A loop that reaches more objects than
//! the address registers hold keeps the index form for the rest. (-O2 and up
//! hoist the eight symbol addresses too, M68K-LICM-SPEED: not this test.)

use llrm_m68k::driver::{Level, assembly, cpu_of};

const EIGHT: &str = include_str!("../data/eight_array_stores.ll");

#[test]
fn a_loop_over_eight_arrays_keeps_no_pointer_in_a_stack_slot() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::Os] {
            let asm = assembly(EIGHT, cpu_of(cpu).unwrap(), level).expect("compiles");
            let body = asm.split("reset:").nth(1).expect("the function is emitted");
            let body = body.split("\n.size").next().unwrap_or(body);
            let slots: Vec<&str> =
                body.lines().filter(|line| line.contains("%sp)") && !line.contains("movem")).collect();
            assert!(slots.is_empty(), "{cpu} {level:?}: stack slots in the loop {slots:?}:\n{asm}");
        }
    }
}
