//! Instruction text against the pin's `llc -mtriple=m68k-linux-gnu` output.

use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

fn inst(
    opcode: &'static str,
    operands: Vec<Operand>,
) -> String {
    print(&Inst { opcode, operands }).expect("prints")
}

/// `move.l (4,%sp), %d0` and `add.l (8,%sp), %d0`, the body of `@f`.
#[test]
fn a_memory_operand_with_a_displacement_prints_as_the_pin_does() {
    let load = vec![Operand::Reg(reg::D0), Operand::Imm(4), Operand::Reg(reg::SP)];
    assert_eq!(inst("MOV32rp", load), "move.l\t(4,%sp), %d0");
    let add = vec![Operand::Reg(reg::D0), Operand::Reg(reg::D0), Operand::Imm(8), Operand::Reg(reg::SP)];
    assert_eq!(inst("ADD32dp", add), "add.l\t(8,%sp), %d0");
}

/// `move.l %d0, (%sp)`, `jsr g`, `suba.l #12, %sp`, `adda.l #12, %sp`, `rts`.
#[test]
fn stores_calls_and_frame_adjustments() {
    assert_eq!(inst("MOV32jr", vec![Operand::Reg(reg::SP), Operand::Reg(reg::D0)]), "move.l\t%d0, (%sp)");
    assert_eq!(inst("CALLb", vec![Operand::Symbol("g".into(), 0)]), "jsr\tg");
    let sub = vec![Operand::Reg(reg::SP), Operand::Reg(reg::SP), Operand::Imm(12)];
    assert_eq!(inst("SUB32ai", sub.clone()), "suba.l\t#12, %sp");
    assert_eq!(inst("ADD32ai", sub), "adda.l\t#12, %sp");
    assert_eq!(inst("RTS", vec![]), "rts");
}

/// `movem.l %d2-%d3, (4,%sp)`; a mask crossing data and address registers is
/// printed as two parts joined by `/`.
#[test]
fn a_register_mask_prints_runs_and_halves() {
    let store = |mask| inst("MOVM32pm", vec![Operand::Imm(4), Operand::Reg(reg::SP), Operand::Imm(mask)]);
    assert_eq!(store(0b1100), "movem.l\t%d2-%d3, (4,%sp)");
    assert_eq!(store(0b0010_0000_0000_0110), "movem.l\t%d1-%d2/%a5, (4,%sp)");
}

/// `link.w %a6, #-48`, `unlk %a6`, `blt .LBB0_1`, `lea (-40,%a6), %a0`.
#[test]
fn link_unlink_branch_and_lea() {
    assert_eq!(inst("LINK16", vec![Operand::Reg(reg::A6), Operand::Imm(-48)]), "link.w\t%a6, #-48");
    assert_eq!(inst("UNLK", vec![Operand::Reg(reg::A6)]), "unlk\t%a6");
    assert_eq!(inst("Blt8", vec![Operand::Symbol(".LBB0_1".into(), 0)]), "blt\t.LBB0_1");
    let lea = vec![Operand::Reg(reg::A0), Operand::Imm(-40), Operand::Reg(reg::A6)];
    assert_eq!(inst("LEA32p", lea), "lea\t(-40,%a6), %a0");
}
