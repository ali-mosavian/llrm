//! countgt at 68020 saved `%d2-%d3` with `movem.l` out and back: 14 + 20 clocks
//! against gcc's four `move.l`, 5 + 5 + 6 + 6, at the same 8 bytes; the matrix
//! counted 483 clocks against gcc's 471. The frame is chosen among the pin's,
//! the `movem` push and a `move.l` a register, by the CPU's price.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::expand::{frame_cost, frame_ops_choosing};
use llrm_m68k::frame::{self, FrameOp};
use llrm_m68k::reg;

fn layout(saved: Vec<llrm_lir::registers::RegId>) -> frame::Layout {
    frame::layout(&frame::Function { saved, ..Default::default() })
}

fn chosen(
    saved: Vec<llrm_lir::registers::RegId>,
    cpu: Cpu,
    size: bool,
) -> Vec<FrameOp> {
    let layout = layout(saved);
    frame_ops_choosing(
        &layout,
        true,
        true,
        |ops| frame_cost(ops, cpu),
        |bytes, clocks| llrm_m68k::timing::order(cpu, size, clocks, bytes as u32),
    )
    .unwrap()
    .0
}

fn each(ops: &[FrameOp]) -> bool {
    ops.iter().any(|op| matches!(op, FrameOp::PushEach { .. }))
}

#[test]
fn two_saved_registers_are_pushed_one_by_one_where_that_is_faster() {
    for cpu in [Cpu::M68020, Cpu::M68040] {
        let ops = chosen(vec![reg::D2, reg::D3], cpu, false);
        assert!(each(&ops), "{cpu:?}: {ops:?}");
    }
}

/// At the 68060 the two cost the same clocks and at -Os three registers are
/// 4 bytes with `movem` against 6: the `movem` push stays.
#[test]
fn the_movem_push_stays_on_a_tie_and_where_it_is_smaller() {
    let tie = chosen(vec![reg::D2, reg::D3], Cpu::M68060, false);
    assert!(!each(&tie), "{tie:?}");
    let size = chosen(vec![reg::D2, reg::D3, reg::D4], Cpu::M68020, true);
    assert!(!each(&size), "{size:?}");
}

const COUNTGT: &str = include_str!("../bench/kernels/countgt.ll");

#[test]
fn countgt_at_68020_saves_its_registers_with_moves() {
    let module = llrm_mir::parse::module(COUNTGT).expect("parses");
    let out = text_with(&module, "k", Cpu::M68020, Options::default()).expect("compiles");
    assert!(out.contains("move.l\t%d3, -(%sp)") && !out.contains("movem"), "{out}");
}
