//! `PACK` and `UNPACK` are one instruction to greedy; the lanes they move
//! between a pair and its bytes are recorded on it (C1 step 1).

use llrm_6502::alloc::{Class, Mos};
use llrm_lir::registers::Regs;
use llrm_lir::valuelanes::ValueLanes;
use llrm_spike_mf::tolir::to_lir;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg};

fn converted(inst: Inst) -> Vec<(u32, u32, ValueLanes)> {
    let mut f = MFunction::new("f");
    f.classes = vec![Class::ANY, Class::ANY, Class::PAIR];
    f.blocks.push(Block { id: 0, insts: vec![inst], phis: Vec::new() });
    let converted = to_lir::<Mos<false>>(&f, Regs(&llrm_6502::REGISTER_INFO)).expect("converts");
    converted.body.blocks[0].insns[0].lane_copies.clone()
}

/// `UNPACK` read as a plain instruction left each byte's lane in the pair
/// unsaid: `lane_copies` empty.
#[test]
fn an_unpack_records_the_lane_each_byte_comes_from() {
    let unpack = Inst::new(
        Kind::Op("UNPACK"),
        vec![Operand::Def(Reg::Virt(0)), Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(2))],
    );
    assert_eq!(converted(unpack), vec![(1, 3, ValueLanes::at(0, 1)), (2, 3, ValueLanes::at(1, 1))]);
}

/// The 6502 lowers a `PACK` to one copy per byte, each into its lane of the
/// pair: the first starts the pair, the second keeps it.
#[test]
fn a_pack_is_lowered_to_a_copy_per_lane() {
    let mut f = MFunction::new("f");
    f.classes = vec![Class::ANY, Class::ANY, Class::PAIR];
    let pack = Inst::new(
        Kind::Op("PACK"),
        vec![Operand::Def(Reg::Virt(2)), Operand::Use(Reg::Virt(0)), Operand::Use(Reg::Virt(1))],
    );
    f.blocks.push(Block { id: 0, insts: vec![pack], phis: Vec::new() });
    let insns = to_lir::<Mos<false>>(&f, Regs(&llrm_6502::REGISTER_INFO)).expect("converts").body.blocks[0].insns.clone();
    let lanes: Vec<_> = insns.iter().map(|one| one.def_lanes.clone()).collect();
    assert_eq!(lanes, vec![vec![(3, ValueLanes::at(0, 1))], vec![(3, ValueLanes::at(1, 1))]]);
    assert!(insns[0].undef_super && insns[1].partial_defs == vec![3]);
}
