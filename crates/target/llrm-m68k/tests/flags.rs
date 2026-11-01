//! `preserve_flags` on hand-built functions: a copy between the instruction
//! that set the condition codes and the branch that reads them is a `move`
//! into a data register, which sets them again.

use llrm_m68k::flags::preserve_flags;
use llrm_m68k::mfunc::MFunction;
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::{Block, Inst, Kind, Operand, Reg};

fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

fn op(
    name: &'static str,
    operands: Vec<Operand>,
    sets: bool,
    reads: bool,
) -> Inst {
    let mut inst = Inst::new(Kind::Op(name), operands);
    inst.sets_flags = sets;
    inst.reads_flags = reads;
    inst
}

fn function(insts: Vec<Inst>) -> MFunction {
    let mut function = MFunction::new("f");
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    function
}

/// d0..d3 for v0..v3.
fn assignment() -> Assignment {
    Assignment(vec![reg::D0, reg::D1, reg::D2, reg::D3])
}

fn kinds(function: &MFunction) -> Vec<Kind> {
    function.blocks[0].insts.iter().map(|inst| inst.kind).collect()
}

/// `cmp; move d2,d3; bne` branched on the `move`'s codes, not the compare's.
#[test]
fn a_copy_that_does_not_depend_on_the_compare_moves_above_it() {
    let mut f = function(vec![
        op("CMP32dr", vec![Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(2), v(3)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(kinds(&f), [Kind::Copy, Kind::Op("CMP32dr"), Kind::Op("Bne8")]);
}

/// `add d0; move d2,d0; bne` branched on the copy's codes; the result is
/// compared with zero again right before the branch (the pin has no `tst`).
#[test]
fn a_copy_of_the_setters_result_is_followed_by_a_test_of_it() {
    let mut f = function(vec![
        op("ADD32dr", vec![Operand::Def(v(0)), Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(2), v(0)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(kinds(&f), [Kind::Op("ADD32dr"), Kind::Copy, Kind::Op("CMP32di"), Kind::Op("Bne8")]);
}

/// A copy that overwrites an operand of the compare leaves nothing to compare
/// again: the codes are pushed before it and popped after (the pin's save and
/// restore), not branched on wrong.
#[test]
fn a_copy_over_the_compares_operand_pushes_and_pops_the_codes() {
    let mut f = function(vec![
        op("CMP32dr", vec![Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        Inst::copy(v(0), v(3)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(
        kinds(&f),
        [Kind::Op("CMP32dr"), Kind::Op("CCR_PUSH"), Kind::Copy, Kind::Op("CCR_POP"), Kind::Op("Bne8")]
    );
}

/// A load into the compare's operand register between the compare and the
/// branch, then a copy that cannot move above it: the compare was issued again
/// with the new value in its operand (differential seed 1260 compared an
/// address register that held another value and crashed). The codes are saved
/// instead.
#[test]
fn an_instruction_that_overwrites_the_compares_operand_prevents_issuing_it_again() {
    let mut f = function(vec![
        op("CMP32dr", vec![Operand::Use(v(0)), Operand::Use(v(1))], true, false),
        op("MOV32ri", vec![Operand::Def(v(1))], false, false),
        Inst::copy(v(2), v(1)),
        op("Bne8", vec![], false, true),
    ]);
    preserve_flags(&mut f, &assignment()).unwrap();
    assert_eq!(
        kinds(&f),
        [
            Kind::Op("CMP32dr"),
            Kind::Op("MOV32ri"),
            Kind::Op("CCR_PUSH"),
            Kind::Copy,
            Kind::Op("CCR_POP"),
            Kind::Op("Bne8")
        ]
    );
}

/// The displacement of the spill store the function holds, with or without a
/// push of the codes round it.
fn spill_displacement(pushed: bool) -> i64 {
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::expand::{Line, finish};
    use llrm_m68k::mc::Operand as Mc;
    let mut insts = Vec::new();
    if pushed {
        insts.push(op("CCR_PUSH", vec![], false, true));
    }
    insts.push(op("MOVM32pm_P", vec![Operand::Imm(0), Operand::Frame(0), Operand::Use(v(0))], false, false));
    if pushed {
        insts.push(op("CCR_POP", vec![], true, false));
    }
    insts.push(op("RET", vec![Operand::Imm(0)], false, false));
    let lines = finish(&function(insts), &assignment(), 0, &[0], Cpu::M68000, false).unwrap();
    lines
        .iter()
        .find_map(|line| match line {
            Line::Inst(inst) if inst.opcode.starts_with("MOVM32pm") => inst.operands.iter().find_map(|o| match o {
                Mc::Imm(disp) => Some(*disp),
                _ => None,
            }),
            _ => None,
        })
        .expect("the spill store")
}

/// A slot named from the stack pointer between `move %sr, -(%sp)` and `move
/// (%sp)+, %ccr` is two bytes further: the store of a spilled copy landed on
/// the saved codes, and the pop restored garbage (differential seed 673 gave
/// 242 for 247).
#[test]
fn a_frame_slot_between_a_push_and_a_pop_of_the_codes_is_two_bytes_further() {
    assert_eq!(spill_displacement(true), spill_displacement(false) + 2);
}
