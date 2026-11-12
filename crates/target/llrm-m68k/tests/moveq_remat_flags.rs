//! A `moveq` made again sets the condition codes, so it may not stand between
//! a compare and the branch that reads it. `select_of_constants` and
//! `sat_builtins` printed wrong hashes: `cmp.l (8,%sp), %d0; moveq #0, %d0;
//! blt` branched on the `moveq`'s codes.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

#[test]
fn a_remade_moveq_does_not_stand_between_a_compare_and_its_branch() {
    let ir = include_str!("data/select_constant_after_compare.ll");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = assembly(ir, cpu, Level::O1).expect("compiles");
        let lines: Vec<&str> = out.lines().map(str::trim).collect();
        for window in lines.windows(3) {
            let between = window[0].starts_with("cmp") && window[1].starts_with("moveq") && window[2].starts_with('b');
            assert!(!between, "{cpu:?}: a moveq between a compare and its branch\n{out}");
        }
    }
}
