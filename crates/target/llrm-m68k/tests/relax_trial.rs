//! The class relaxation (`Options::relax_classes`) is decided per function by
//! building the whole trial sequence both ways, the cheaper kept: at -Os it
//! never makes a program larger, and makes `fib` smaller (26 -> 22 B).
//!
//! `stft_fixed` -Os came out 4 B larger (68000, 1143 -> 1147 B) when the
//! relaxed build was kept unpriced: one more spill slot in a pressured
//! function.

use llrm_m68k::compile::{Options, items_bytes, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, lowered_for};
use llrm_m68k::kernels::source;

/// The bytes of every function of `ir` built at -Os (`items_bytes`: a branch
/// not yet sized counts 6).
fn bytes(
    ir: &str,
    cpu: Cpu,
    relax_classes: bool,
) -> usize {
    let module = lowered_for(ir, cpu, Level::Os).expect("lowers");
    let options = Options { size: true, mid_end: false, relax_classes, ..Options::default() };
    module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, global, _)| {
            let name = global.name.owned().expect("a name");
            let items = items_with(&module, &name, cpu, options).expect("compiles");
            items_bytes(&items)
        })
        .sum()
}

#[test]
fn at_os_a_pressured_program_is_no_larger_with_the_relaxation() {
    let ir = include_str!("data/stft_fixed.ll");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let (on, off) = (bytes(ir, cpu, true), bytes(ir, cpu, false));
        assert!(on <= off, "{cpu:?}: {on} B with, {off} B without");
    }
}

#[test]
fn fib_is_smaller_with_the_relaxation() {
    let ir = source("fib");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let (on, off) = (bytes(&ir, cpu, true), bytes(&ir, cpu, false));
        assert!(on < off, "{cpu:?}: {on} B with, {off} B without");
    }
}
