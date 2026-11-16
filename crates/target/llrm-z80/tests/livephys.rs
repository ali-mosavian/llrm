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

/// A sign test of `hl` reads `h` only. Counting `l` live made crc16 copy a byte
/// nothing read: `ld l, e` in its inner loop.
#[test]
fn a_sign_test_of_a_pair_reads_its_high_byte() {
    let test = Inst::new(Kind::Op("BIT7_16"), vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(Reg::Phys(reg::HL))]);
    let f = function(vec![vec![test]]);
    assert!(live(&f, 0, 0, reg::H) && !live(&f, 0, 0, reg::L));
}

/// `live_before` at every position of a block of 40,000 instructions stepped
/// back from the block's end each time, 800 M steps: the printer asks at every
/// instruction, and `select_of_constants` -O2 spent 140 M steps and
/// half its compile there for 800 call sites. The answers come from
/// checkpoints, `STRIDE` steps at most each.
#[test]
fn every_position_of_a_long_block_is_asked_without_walking_to_its_end() {
    let n = 40_000;
    let insts: Vec<Inst> = (0..n).map(|at| if at % 2 == 0 { def(reg::B) } else { read(reg::B) }).collect();
    let f = function(vec![insts]);
    let assignment = Assignment(Vec::new());
    let start = std::time::Instant::now();
    let live = PhysLiveness::of::<Z80Registers>(&f, &assignment, &REGISTER_INFO);
    for at in 0..n {
        let before = live.live_before::<Z80Registers>(&f, &assignment, &REGISTER_INFO, 0, at);
        // `b` is read at the odd positions and written at the even ones just
        // before
        assert_eq!(before.is_live(&REGISTER_INFO, reg::B), at % 2 == 1, "at {at}");
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(3), "40,000 queries took {:?}", start.elapsed());
}

/// Solving the live-outs swept every block again to see that nothing moved: an
/// acyclic function of 50 straight blocks was stepped twice over
/// (switch_jump_table -O2: 5% of the compile in 270 solves). Each instruction
/// is stepped once where no loop feeds a block's live-in back, and the answers
/// are the same.
#[test]
fn an_acyclic_function_is_stepped_over_once_when_its_live_outs_are_solved() {
    let blocks: Vec<Vec<Inst>> = (0..50)
        .map(|at| {
            vec![def(reg::B), read(reg::B), read(reg::D), def(reg::E), read(if at % 2 == 0 { reg::A } else { reg::C })]
        })
        .collect();
    let f = function(blocks);
    let assignment = Assignment(Vec::new());
    llrm_spike_mf::livephys::take_steps();
    let live = PhysLiveness::of::<Z80Registers>(&f, &assignment, &REGISTER_INFO);
    assert_eq!(llrm_spike_mf::livephys::take_steps(), 50 * 5, "every instruction once");
    // `d` is read in every block and written in none: live before the first
    // instruction of block 0
    let before = live.live_before::<Z80Registers>(&f, &assignment, &REGISTER_INFO, 0, 0);
    assert!(before.is_live(&REGISTER_INFO, reg::D) && !before.is_live(&REGISTER_INFO, reg::B));
}

/// The worklist must reach the sweeps' answer where blocks loop, branch out of order and fall through: 400
/// functions of 2 to 12 blocks, a third of the branches backward.
#[test]
fn the_worklist_solves_the_same_live_outs_as_sweeps_on_functions_with_loops() {
    let registers = [reg::A, reg::B, reg::C, reg::D, reg::E, reg::H, reg::L, reg::BC, reg::DE, reg::HL];
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move |bound: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % bound
    };
    let mut loops = 0;
    for _ in 0..400 {
        let count = 2 + next(11);
        let blocks: Vec<Vec<Inst>> = (0..count)
            .map(|at| {
                let mut insts: Vec<Inst> = (0..1 + next(6))
                    .map(|_| {
                        let one = registers[next(registers.len())];
                        if next(2) == 0 { def(one) } else { read(one) }
                    })
                    .collect();
                if next(3) > 0 {
                    let to = next(count);
                    loops += usize::from(to <= at);
                    let mut jump = Inst::new(Kind::Op("JP"), vec![Operand::Block(to as u32)]);
                    jump.barrier = next(2) == 0;
                    insts.push(jump);
                }
                insts
            })
            .collect();
        let f = function(blocks);
        let assignment = Assignment(Vec::new());
        let worked = PhysLiveness::of::<Z80Registers>(&f, &assignment, &REGISTER_INFO);
        let swept = PhysLiveness::of_by_sweeps::<Z80Registers>(&f, &assignment, &REGISTER_INFO);
        for b in 0..count {
            assert!(worked.live_out_of(b) == swept.live_out_of(b), "block {b} of {f:?}");
        }
    }
    assert!(loops > 100, "{loops} backward branches");
}
