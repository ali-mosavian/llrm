//! `fib` at -Os kept the carried `a` in a data register: the add's source
//! copy kept the `DR32` of the tied operand and joined `a`, so `s`, `i` and
//! `a` needed three data registers and the counter took `d2` (a push and a
//! pop, 26 B against gcc's 22 B).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};
use llrm_m68k::kernels::source;

#[test]
fn the_carried_value_does_not_cost_a_saved_register() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = assembly(&source("fib"), cpu, Level::Os).expect("compiles");
        assert!(!out.contains("-(%sp)"), "{cpu:?}: a register is saved\n{out}");
    }
}
