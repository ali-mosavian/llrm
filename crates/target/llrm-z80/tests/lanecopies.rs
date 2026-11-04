//! `PACK` is one instruction to greedy; the lanes it fills in the pair are
//! recorded on it (C1 step 1).

use llrm_lir::registers::Regs;
use llrm_lir::valuelanes::ValueLanes;
use llrm_spike_mf::tolir::to_lir;
use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_z80::regalloc::{Class, Z80Registers};

/// `PACK` read as a plain instruction left each byte's lane in the pair
/// unsaid: nothing for the lane model to read.
#[test]
fn a_pack_records_the_lane_each_byte_fills_in_the_pair() {
    let mut f = MFunction::new("f");
    f.classes = vec![Class::Gr8, Class::Gr8, Class::Gr16];
    let pack = Inst::new(
        Kind::Op("PACK"),
        vec![Operand::Def(Reg::Virt(2)), Operand::Use(Reg::Virt(0)), Operand::Use(Reg::Virt(1))],
    );
    f.blocks.push(Block { id: 0, insts: vec![pack], phis: Vec::new() });
    let converted = to_lir::<Z80Registers>(&f, Regs(&llrm_z80::REGISTER_INFO)).expect("converts");
    let insns = &converted.body.blocks[0].insns;
    assert_eq!(insns.len(), 2, "one copy per byte");
    assert_eq!(insns[0].def_lanes, vec![(3, ValueLanes::at(0, 1))]);
    assert!(insns[0].undef_super, "the low byte starts the pair");
    assert_eq!(insns[1].def_lanes, vec![(3, ValueLanes::at(1, 1))]);
    assert_eq!(insns[1].partial_defs, vec![3], "the high byte keeps the low one");
}

/// The split editor and the spiller write a lane alone (series step 4), so the
/// z80 lowers `PACK` to lane copies and joins them.
#[test]
fn z80_lowers_and_joins_lanes() {
    use llrm_spike_mf::regalloc::Target;
    assert!(Z80Registers::lowers_lanes() && Z80Registers::joins_lanes());
}
