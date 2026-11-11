//! `partition` (swap the elements below the pivot to the front) at 68000 -O2
//! put the loop's latch, which exits and loops back, first: the loop was
//! entered by a `bra` over it and the swap path left by another `bra` to it,
//! 3 jumps and 42 clk (1268 against gcc's 1226) where the header-first order
//! has the swap path fall through into the latch.
//!
//! The pin lays the loop out the same way (it counts every fall-through);
//! the fix is the named deviation M68K-TOP.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const PARTITION: &str = include_str!("../bench/library/partition.ll");

fn jumps(
    cpu: Cpu,
    options: Options,
) -> usize {
    let module = llrm_mir::parse::module(PARTITION).expect("parses");
    let text = text_with(&module, "k", cpu, options).expect("compiles");
    text.lines().filter(|line| line.trim_start().starts_with("bra\t")).count()
}

/// Entry into the loop falls through and the swap path falls into the latch:
/// one `bra` is left, the one over the pivot-swap-less exit.
#[test]
fn the_loop_is_laid_out_header_first() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
        let got = jumps(cpu, Options::default());
        assert!(got <= 1, "{cpu:?}: {got} unconditional jumps");
    }
}

/// The pin's order is kept for the pinned output.
#[test]
fn the_option_off_keeps_the_pin_s_order() {
    let got = jumps(Cpu::M68000, Options { loop_top_by_jumps: false, ..Options::default() });
    assert!(got >= 3, "{got} unconditional jumps");
}
