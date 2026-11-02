//! `PhysLiveness` on allocated Z80 functions: a byte register that is free for
//! part of a block, a pair half live out of a block, a call's clobbers. A pass
//! that picks a scratch register after allocation asks these.

use llrm_spike_mf::livephys::PhysLiveness;
use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::regalloc::{Class, Z80Registers};
use llrm_z80::{REGISTER_INFO, reg};

fn def(register: llrm_lir::registers::RegId) -> Inst {
    Inst::new(Kind::Op("DEF"), vec![Operand::Def(Reg::Phys(register)), Operand::Imm(0)])
}

fn read(register: llrm_lir::registers::RegId) -> Inst {
    Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Phys(register))])
}

fn function(blocks: Vec<Vec<Inst>>) -> MFunction<Class> {
    let mut function = MFunction::new("f");
    function.blocks =
        blocks.into_iter().enumerate().map(|(id, insts)| Block { id: id as u32, insts, phis: Vec::new() }).collect();
    function
}

fn live(
    function: &MFunction<Class>,
    b: usize,
    at: usize,
    register: llrm_lir::registers::RegId,
) -> bool {
    let assignment = Assignment(Vec::new());
    PhysLiveness::of::<Z80Registers>(function, &assignment, &REGISTER_INFO)
        .live_before::<Z80Registers>(function, &assignment, &REGISTER_INFO, b, at)
        .is_live(&REGISTER_INFO, register)
}

/// `c` is written at 1 and read at 2, `b` is written at 0 and read at 3: before
/// 1, `c` is free and `b` is live, and the pair `bc` is live through its `b`
/// half. A query that answered per root would call `c` live there, or `b` free.
#[test]
fn a_byte_is_free_in_part_of_a_block_while_its_pair_mate_is_live() {
    let f = function(vec![vec![def(reg::B), def(reg::C), read(reg::C), read(reg::B)]]);
    assert!(!live(&f, 0, 0, reg::B) && !live(&f, 0, 0, reg::BC));
    assert!(live(&f, 0, 1, reg::B) && !live(&f, 0, 1, reg::C) && live(&f, 0, 1, reg::BC));
    assert!(live(&f, 0, 2, reg::C) && live(&f, 0, 2, reg::B));
    assert!(live(&f, 0, 3, reg::B) && !live(&f, 0, 3, reg::C));
}

/// What the next block reads is live out of this one, and a write in this one
/// that the next block does not follow with a read of its own ends the range.
#[test]
fn a_register_the_next_block_reads_is_live_out() {
    let f = function(vec![vec![def(reg::D), read(reg::A)], vec![read(reg::D)]]);
    assert!(live(&f, 0, 1, reg::D) && !live(&f, 0, 0, reg::D));
    assert!(live(&f, 0, 1, reg::A) && live(&f, 0, 0, reg::A));
}

/// A call leaves the caller-saved registers unspecified: nothing is live across
/// it that it does not read.
#[test]
fn a_call_ends_the_ranges_of_the_registers_it_clobbers() {
    let mut call = Inst::new(Kind::Op("CALL"), vec![Operand::Symbol("g".to_owned(), 0)]);
    call.call = true;
    let f = function(vec![vec![def(reg::E), call, read(reg::E)]]);
    assert!(!live(&f, 0, 1, reg::E), "e is read after the call, but the call clobbers it, so it is not live before");
}
