//! Spill slots become `push` and `pop` only where that is what the code does: a byte slot saves its pair, so the other
//! half must be dead once the pair is restored.

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::compile::slots_are_a_stack;
use llrm_z80::regalloc::Class;
use llrm_z80::reg;

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

/// `c` is written between the push and the pop and read after: `pop bc` would put the old `c` back.
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
