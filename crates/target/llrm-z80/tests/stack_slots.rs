//! Spill slots become `push` and `pop` only where that is what the code does: a
//! byte slot saves its pair, so the other half must be dead once the pair is
//! restored.

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::compile::{slots_are_a_stack, stack_slot_report_by};
use llrm_z80::reg;
use llrm_z80::regalloc::Class;

fn function(tail: Vec<Inst>) -> MFunction<Class> {
    let mut insts = vec![
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::B))]),
        Inst::new(Kind::Op("DEF"), vec![Operand::Def(Reg::Phys(reg::C)), Operand::Imm(0)]),
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Phys(reg::B)), Operand::Frame(0)]),
    ];
    insts.extend(tail);
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    function
}

/// `c` is written between the push and the pop and read after: `pop bc` would
/// put the old `c` back.
#[test]
fn a_byte_slot_is_not_a_pop_while_the_other_half_of_its_pair_is_live() {
    let mut f = function(vec![Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Phys(reg::C))])]);
    assert!(!slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    assert_eq!(f.blocks[0].insts[0].kind, Kind::Op("SPILL8"));
}

#[test]
fn a_byte_slot_is_a_pop_when_the_other_half_is_dead() {
    let mut f = function(vec![Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Phys(reg::B))])]);
    assert!(slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    assert_eq!(f.blocks[0].insts[0].kind, Kind::Op("PUSHP"));
    assert_eq!(f.blocks[0].insts[2].kind, Kind::Op("POPP"));
}

fn word_function(tail: Vec<Inst>) -> MFunction<Class> {
    let mut insts = vec![Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::DE))])];
    insts.extend(tail);
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    function
}

/// A slot read twice stayed in the frame (`LoadNotOnTop` at the second read),
/// so the `pop bc; push bc` read of the slot on top was lost and two IX loads
/// stayed: the first read now leaves it on the stack, the last pops it.
#[test]
fn a_slot_read_twice_is_peeked_then_popped() {
    let reload = || Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Frame(0)]);
    let mut f = word_function(vec![reload(), reload()]);
    assert!(slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    let kinds: Vec<Kind> = f.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds, [Kind::Op("PUSHP"), Kind::Op("PEEKP"), Kind::Op("POPP")]);
}

/// Four words saved and read back the way a queue is read, three of them under
/// the fourth, which is read last: the slot loaded first was sent to the
/// frame, then the next, then the next, and only the top one stayed a `push`
/// (1 of 4). The top one is in every conflict, so it goes: three stay.
#[test]
fn the_slot_in_the_most_conflicts_goes_back_to_the_frame() {
    let save = |slot| Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(slot), Operand::Use(Reg::Phys(reg::DE))]);
    let read = |slot| Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Frame(slot)]);
    let body = || {
        let mut f = MFunction::new("f");
        let insts = vec![save(0), save(1), save(2), save(3), read(2), read(1), read(0), read(3)];
        f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
        f
    };
    let first_violation = stack_slot_report_by(&mut body(), &Assignment(Vec::new()), false, false);
    assert_eq!(first_violation.pushed, [3]);
    let by_conflicts = stack_slot_report_by(&mut body(), &Assignment(Vec::new()), false, true);
    assert_eq!(by_conflicts.pushed, [0, 1, 2]);
}
