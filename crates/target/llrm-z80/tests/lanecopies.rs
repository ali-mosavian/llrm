//! `PACK` is one instruction to greedy; the lanes it fills in the pair are
//! recorded on it (C1 step 1).

use llrm_lir::registers::Regs;
use llrm_lir::valuelanes::ValueLanes;
use llrm_spike_mf::tolir::to_lir;
use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_z80::regalloc::{Class, Z80Registers};

/// `PACK` read as a plain instruction left each byte's lane in the pair
/// unsaid: nothing for the lane model to read, `lane_copies` empty.
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
    let insn = &converted.body.blocks[0].insns[0];
    assert_eq!(insn.lane_copies, vec![(1, 3, ValueLanes::at(0, 1)), (2, 3, ValueLanes::at(1, 1))]);
    assert_eq!(insn.defines, vec![3], "liveness still takes the pair whole");
}
