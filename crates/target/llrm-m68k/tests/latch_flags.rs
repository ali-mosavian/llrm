//! The latch of `partition` (`n - 1` iterations, counted down, branching on
//! the decrement's flags) became `move.l %d2, %d3; subq.l #1, %d2; subq.l #1,
//! %d3; beq`: after #7558 made `CCR` costly to copy, the scheduler cloned the
//! decrement whenever the copy hold-back (`copy_clobbers`, which the pin does
//! not have) left every candidate waiting for the live flags. Two extra
//! instructions a trip: 1268 -> 1444 clk at 68000 -O2, memset and memcpy +30%.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const PARTITION: &str = include_str!("fixtures/partition_cc.ll");

fn decrements(cpu: Cpu) -> usize {
    let module = llrm_mir::parse::module(PARTITION).expect("parses");
    let text = text_with(&module, "k", cpu, Options::default()).expect("compiles");
    text.lines().filter(|line| line.starts_with("\tsubq.l\t#1, %d")).count()
}

/// One decrement before the loop (`n - 1`) and one in the latch.
#[test]
fn the_latch_decrement_is_not_cloned_for_its_flags() {
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
        assert_eq!(decrements(cpu), 2, "{cpu:?}");
    }
}
