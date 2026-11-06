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

/// A copy the spiller made beside the lane copy of a built pair took the lane
/// copy's operand classes: its pair was handed a byte register, and
/// `sum3UL -O2` was refused ("d has no byte halves").
#[test]
fn a_pair_copied_beside_a_lane_copy_keeps_its_pair_class() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join("llrm-z80-pair-beside-lane.s");
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/data/pair_reload_beside_lane_copy.ll")])
        .output()
        .expect("llrm-z80-llc runs");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

/// Lowering and joining lanes stay off until the split editor copies by lane
/// (DEVIATE 3b-1): lowering alone grew the corpus (+505 lines at -O2), and
/// joining summed a stale byte in `mul_constants` -O2.
#[test]
fn z80_keeps_lanes_off_until_the_split_copies_by_lane() {
    use llrm_spike_mf::regalloc::Target;
    assert!(!Z80Registers::lowers_lanes() && !Z80Registers::joins_lanes());
}
