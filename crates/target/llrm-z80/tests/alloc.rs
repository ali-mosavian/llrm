//! What greedy does with the Z80's overlapping registers, on hand-built machine
//! functions: bytes live together take `b c d e h l a` (seven values, no
//! spill), and a byte never takes half of a pair that holds a word.

use llrm_lir::registers::Regs;
use llrm_spike_mf::regalloc::{Target, eliminate_phis, two_address};
use llrm_spike_mf::{Block, Inst, Kind, Operand, Reg};
use llrm_z80::regalloc::{Class, MFunction, Z80Registers};
use llrm_z80::{REGISTER_INFO, reg};

fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

/// Every class in `classes` defined in turn, then all read by one instruction,
/// so all are live together.
fn live_together(classes: &[Class]) -> MFunction {
    let mut f = MFunction::new("together");
    f.classes = classes.to_vec();
    let mut entry = Block { id: 0, ..Block::default() };
    for n in 0..classes.len() as u32 {
        entry.insts.push(Inst::new(Kind::Op("def"), vec![Operand::Def(v(n))]));
    }
    entry.insts.push(Inst::new(Kind::Op("use"), (0..classes.len() as u32).map(|n| Operand::Use(v(n))).collect()));
    let mut ret = Inst::new(Kind::Op("ret"), vec![]);
    ret.terminator = true;
    ret.barrier = true;
    entry.insts.push(ret);
    f.blocks = vec![entry];
    eliminate_phis(&mut f);
    two_address(&mut f);
    f
}

fn allocated(classes: &[Class]) -> Vec<llrm_lir::registers::RegId> {
    let mut f = live_together(classes);
    let (assignment, slots) =
        llrm_spike_mf::tolir::allocate_greedy::<Z80Registers>(&mut f, Regs(&REGISTER_INFO), false)
            .unwrap_or_else(|why| panic!("{why}\n{f:#?}"));
    assert!(slots.is_empty(), "spilled: {slots:?}");
    assignment.0
}

/// Seven bytes at once is every byte register. If a byte took a whole pair only
/// four would fit.
#[test]
fn seven_live_bytes_take_all_seven_byte_registers() {
    let got = allocated(&[Class::Gr8; 7]);
    let mut names: Vec<_> = got.iter().map(|r| REGISTER_INFO.name(*r).unwrap()).collect();
    names.sort_unstable();
    assert_eq!(names, ["a", "b", "c", "d", "e", "h", "l"], "{got:?}");
}

/// A word in a pair and five bytes: the bytes are `a` and the halves of the
/// other two pairs, never a half of the word's pair.
#[test]
fn a_byte_never_takes_half_of_a_live_pair() {
    let got = allocated(&[Class::Gr16, Class::Gr8, Class::Gr8, Class::Gr8, Class::Gr8, Class::Gr8]);
    for byte in &got[1..] {
        assert!(
            !Z80Registers::overlaps(*byte, got[0]),
            "{} overlaps {}",
            REGISTER_INFO.name(*byte).unwrap(),
            REGISTER_INFO.name(got[0]).unwrap()
        );
    }
    let _ = reg::A;
}

/// The probe can fail: eight bytes defined up front and read two at a time
/// cannot all stay in seven registers, so something is spilled.
#[test]
fn eight_live_bytes_spill_one() {
    let mut f = MFunction::new("eight");
    f.classes = vec![Class::Gr8; 8];
    let mut entry = Block { id: 0, ..Block::default() };
    for n in 0..8 {
        entry.insts.push(Inst::new(Kind::Op("def"), vec![Operand::Def(v(n))]));
    }
    for n in (0..8).step_by(2) {
        entry.insts.push(Inst::new(Kind::Op("use"), vec![Operand::Use(v(n)), Operand::Use(v(n + 1))]));
    }
    let mut ret = Inst::new(Kind::Op("ret"), vec![]);
    ret.terminator = true;
    ret.barrier = true;
    entry.insts.push(ret);
    f.blocks = vec![entry];
    let (_, slots) =
        llrm_spike_mf::tolir::allocate_greedy::<Z80Registers>(&mut f, Regs(&REGISTER_INFO), false).expect("allocates");
    assert!(!slots.is_empty(), "eight values in seven registers spilled nothing");
}
