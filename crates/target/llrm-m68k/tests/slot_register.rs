//! A byte value spilled while an address register is free went to a frame
//! slot (`move.b %d2,(4,%sp)`, 4 B and 16 clocks a side at the 68000), where
//! gcc keeps it in `a0` (`movea.l %d2,%a0`, 2 B and 4 clocks):
//! known_loads_walks -Os, 92 frame references to gcc's 25 (DEVIATE SLOT-REG,
//! `src/slotreg.rs`).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

mod support;

const IR: &str = include_str!("fixtures/slot_register.ll");

fn asm(slot_registers: bool) -> String {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let options = Options { short: true, slot_registers, ..Options::default() };
    text_with(&module, "f", Cpu::M68000, options).expect("compiles")
}

fn frame_references(text: &str) -> usize {
    text.lines().filter(|line| line.contains(",%sp)") && !line.trim_start().starts_with("lea")).count()
}

#[test]
fn a_spilled_byte_lives_in_a_free_address_register() {
    let off = asm(false);
    let on = asm(true);
    assert!(frame_references(&off) > 0, "premise: the build spills\n{off}");
    assert!(frame_references(&on) < frame_references(&off), "{on}");
    assert!(on.contains("%a0") || on.contains("%a1"), "{on}");
    assert!(frame_size(&on) < frame_size(&off), "a slot in a register still has a frame slot\n{on}");
}

/// The bytes the prologue's `lea (-N,%sp),%sp` opens.
fn frame_size(text: &str) -> i32 {
    text.lines()
        .find_map(|line| line.trim_start().strip_prefix("lea\t(-")?.split(',').next()?.parse().ok())
        .unwrap_or(0)
}
