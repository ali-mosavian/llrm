//! `MFunction` -> `LirBody`: what greedy reads of a spike function.

use llrm_codegen::greedy::shadow::{Mode, decide_with_split};
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::{Loc, Operation};
use llrm_spike_mf::regalloc::{Target, eliminate_phis, two_address};
use llrm_spike_mf::tolir::to_lir;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg};

/// Three allocatable registers, one reserved, all lost to a call.
struct Flat;

const REGISTERS: [RegId; 3] = [RegId::EBX, RegId::ECX, RegId::EDX];

impl Target for Flat {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        REGISTERS.to_vec()
    }

    fn view(
        root: RegId,
        _: (),
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(root: RegId) -> bool {
        root == RegId::ESP
    }

    fn caller_saved() -> Vec<RegId> {
        REGISTERS.to_vec()
    }

    fn spill_store(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("st"), vec![Operand::Frame(slot), Operand::Use(register)]))
    }

    fn spill_load(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("ld"), vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

fn regs() -> Regs {
    Regs(&llrm_x86_m16::REGISTER_INFO)
}

fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

fn op(
    name: &'static str,
    operands: Vec<Operand>,
) -> Inst {
    Inst::new(Kind::Op(name), operands)
}

fn branch(to: u32) -> Inst {
    let mut inst = op("branch", vec![Operand::Block(to)]);
    inst.terminator = true;
    inst
}

fn ret() -> Inst {
    let mut inst = op("ret", vec![]);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

fn call() -> Inst {
    let mut inst = op("call", vec![Operand::Symbol("f".into(), 0)]);
    inst.call = true;
    inst
}

/// A value read in a loop and again after a call that destroys every register.
fn across_a_call(reads: usize) -> MFunction<()> {
    let mut f = MFunction::<()>::new("across");
    f.classes = vec![(); 3];
    let mut entry = Block { id: 0, ..Block::default() };
    entry.insts.push(op("load", vec![Operand::Def(v(0))]));
    entry.insts.push(op("load", vec![Operand::Def(v(1))]));
    let mut body = Block { id: 1, ..Block::default() };
    for _ in 0..reads {
        body.insts.push(op("add", vec![Operand::Def(v(1)), Operand::Use(v(1)), Operand::Use(v(0))]));
    }
    body.insts.push(branch(1));
    let mut exit = Block { id: 2, ..Block::default() };
    exit.insts.push(call());
    exit.insts.push(op("add", vec![Operand::Def(v(2)), Operand::Use(v(1)), Operand::Use(v(0))]));
    exit.insts.push(op("store", vec![Operand::Use(v(2))]));
    exit.insts.push(ret());
    f.blocks = vec![entry, body, exit];
    f
}

fn lowered(mut f: MFunction<()>) -> MFunction<()> {
    eliminate_phis(&mut f);
    two_address(&mut f);
    f
}

/// Greedy takes no phi and no tie; the converter says so rather than hand it
/// a body it would refuse.
#[test]
fn test_a_function_with_a_phi_is_refused() {
    let mut f = across_a_call(1);
    f.blocks[2].phis.push(llrm_spike_mf::Phi { result: v(2), incoming: vec![(0, v(0)), (1, v(1))] });
    assert!(to_lir::<Flat>(&f, regs()).is_err());
}

/// The blocks, edges, calls and physical operands greedy reads.
#[test]
fn test_the_converted_body_has_the_cfg_the_calls_and_the_pins() {
    let mut f = across_a_call(2);
    // `rcall` leaves its result in ebx; the block reads it.
    f.blocks[2].insts.insert(1, op("get", vec![Operand::Def(v(2)), Operand::Use(Reg::Phys(RegId::EBX))]));
    f.blocks[2].insts[0].operands.push(Operand::Def(Reg::Phys(RegId::EBX)));
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let body = &converted.body;
    assert_eq!(body.blocks.len(), 3);
    assert_eq!(*body.blocks[0].succ, vec![1], "falls through to the loop");
    assert_eq!(*body.blocks[1].succ, vec![1, 2], "branches back and falls out");
    let call = &body.blocks[2].insns[0];
    assert_eq!(call.what.as_ref().map(|w| w.op), Some(Operation::Call));
    assert_eq!(call.clobbers.len(), 3, "a call loses every register: {call:?}");
    assert_eq!(call.delivers.len(), 1, "the register it leaves its result in is delivered");
    let read = &body.blocks[2].insns[1];
    assert_eq!(read.requires.len(), 1, "the read of ebx requires it");
    assert_eq!(read.requires[0].1, RegId::EBX);
    assert_eq!(read.uses, vec![call.defines[0]], "and reads the value the call delivered");
    assert_eq!(converted.order(call.defines[0]), vec![RegId::EBX], "a pinned value takes only its register");
    assert!(converted.unspillable.contains(&call.defines[0]));
    // The instruction's own ordinal is its `at`: the way back to the MFunction.
    assert_eq!(converted.origin[(read.at / 2) as usize], (2, 1));
}

/// A tied operand is read and written by one instruction, as an x86 two-address
/// form is: the value is live through it, not defined twice.
#[test]
fn test_a_tied_operand_is_one_value_read_and_written() {
    let converted = to_lir::<Flat>(&lowered(across_a_call(1)), regs()).expect("converts");
    let add = &converted.body.blocks[1].insns[0];
    assert_eq!(add.defines, vec![2]);
    assert!(add.uses.contains(&2) && add.uses.contains(&1), "{add:?}");
    assert!(matches!(add.what.as_ref().map(|w| &w.dests[0]), Some(Loc::Held(held)) if held.value == 2));
}

/// The spike allocator stores and reloads `function`'s spilled values in the
/// code it leaves: its count of `st` and `ld` instructions.
fn spike_slot_traffic(mut f: MFunction<()>) -> (usize, usize) {
    llrm_spike_mf::spill::allocate_spilling::<Flat>(&mut f, 100).expect("the spike allocator fits it");
    let count =
        |name| f.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op(name)).count();
    (count("st"), count("ld"))
}

/// Where the spike allocator spills a value that crosses a call for its whole
/// range, greedy keeps it in a register in the loop and reloads it after the
/// call (lz4's call-crossing ranges, 52 slot stores). Record mode never
/// splits, so Simple mode shows it.
#[test]
fn test_greedy_splits_a_range_the_spike_allocator_spills_whole() {
    let f = lowered(across_a_call(4));
    let (spike_stores, spike_loads) = spike_slot_traffic(f.clone());
    let converted = to_lir::<Flat>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let whole = decide_with_split(&inputs, Mode::Simple, false).expect("allocates");
    let split = decide_with_split(&inputs, Mode::Simple, true).expect("allocates");
    assert!(spike_loads >= 4, "premise: the spike allocator reloads in the loop ({spike_stores} st, {spike_loads} ld)");
    assert!(
        split.report.reloads < spike_loads,
        "reloads: spike {spike_loads}, greedy whole {} split {}",
        whole.report.reloads,
        split.report.reloads
    );
    assert!(split.report.reloads < whole.report.reloads);
    assert!(
        split.report.stores < spike_stores && split.report.stores < whole.report.stores,
        "stores: spike {spike_stores}, greedy whole {} split {}",
        whole.report.stores,
        split.report.stores
    );
}

/// Record mode only notes what to spill and writes nothing, so a split (which
/// edits the function) is off in it (`decide_with_split`): the reason the test
/// above runs in Simple mode.
#[test]
fn test_record_mode_never_splits() {
    let converted = to_lir::<Flat>(&lowered(across_a_call(4)), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let plain = decide_with_split(&inputs, Mode::Record, false).expect("allocates");
    let asked = decide_with_split(&inputs, Mode::Record, true).expect("allocates");
    assert_eq!(plain.spilled, asked.spilled);
    assert_eq!(plain.report.signature, asked.report.signature);
}

/// One pair root with two byte lanes (`rs1` = `rc2` + `rc3`, as `ebx` = `bl` +
/// `bh`): a byte class names the root, and each byte value takes a lane of it.
struct Bytes;

impl Target for Bytes {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        vec![RegId::EBX]
    }

    fn view(
        _: RegId,
        _: (),
    ) -> RegId {
        RegId::BL
    }

    fn root(register: RegId) -> RegId {
        regs().root(register)
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }
}

/// Two byte values live together fit the two lanes of one pair root. If a
/// value held its whole root (`_whole`), the second would find none.
#[test]
fn test_two_byte_values_take_the_two_lanes_of_one_root() {
    let mut f = MFunction::<()>::new("lanes");
    f.classes = vec![(); 3];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(op("load", vec![Operand::Def(v(1))]));
    block.insts.push(op("add", vec![Operand::Def(v(2)), Operand::Use(v(0)), Operand::Use(v(1))]));
    block.insts.push(op("store", vec![Operand::Use(v(2))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let converted = to_lir::<Bytes>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, true).expect("allocates");
    let place = |value| decided.placed.iter().find(|(v, _)| *v == value).map(|(_, r)| *r);
    assert_eq!(decided.report.stats.failed, 0, "{:?}", decided.placed);
    assert_ne!(place(1), place(2), "the two bytes are live together: {:?}", decided.placed);
    assert!(decided.spilled.is_empty(), "{:?}", decided.spilled);
}

/// The control of the test above: a third byte live with two others finds no
/// lane, so the two lanes are the whole capacity and not an artefact of the
/// toy file.
#[test]
fn test_a_third_byte_value_live_with_two_finds_no_lane() {
    let mut f = MFunction::<()>::new("crowd");
    f.classes = vec![(); 4];
    let mut block = Block { id: 0, ..Block::default() };
    for n in 0..3 {
        block.insts.push(op("load", vec![Operand::Def(v(n))]));
    }
    block.insts.push(op("add", vec![Operand::Def(v(3)), Operand::Use(v(0)), Operand::Use(v(1))]));
    block.insts.push(op("add", vec![Operand::Def(v(3)), Operand::Use(v(3)), Operand::Use(v(2))]));
    block.insts.push(op("store", vec![Operand::Use(v(3))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let converted = to_lir::<Bytes>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, false).expect("allocates");
    assert!(
        decided.report.stats.failed > 0 || !decided.spilled.is_empty() || decided.report.reloads > 0,
        "three live bytes fit two lanes: {:?}",
        decided.placed
    );
}

/// A byte pinned to one lane leaves the other to a value live over it. The
/// pin was the whole root, so `bl` held in `bh`'s way too.
#[test]
fn test_a_pin_to_one_lane_leaves_the_other_lane() {
    let mut f = MFunction::<()>::new("pinned");
    f.classes = vec![(); 2];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::BL))]));
    block.insts.push(op("add", vec![Operand::Def(v(1)), Operand::Use(v(0)), Operand::Use(Reg::Phys(RegId::BL))]));
    block.insts.push(op("store", vec![Operand::Use(v(1))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let converted = to_lir::<Bytes>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, false).expect("allocates");
    let held = decided.placed.iter().find(|(value, _)| *value == 1).map(|(_, register)| *register);
    assert_eq!(held, Some(RegId::BH), "{:?}", decided.placed);
    assert_eq!(decided.report.reloads, 0);
}

/// A byte class that names its registers, high lane first.
struct HighFirst;

impl Target for HighFirst {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        vec![RegId::BH, RegId::BL]
    }

    fn view(
        root: RegId,
        _: (),
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        regs().root(register)
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }
}

/// A target orders its byte registers (the 6502: index registers, scratch
/// cells, kept cells, then `a`); the order of the file's `@bytes` replaced it,
/// so a value took the file's first lane and not the class's.
#[test]
fn test_a_byte_value_takes_its_classs_first_register() {
    let mut f = MFunction::<()>::new("order");
    f.classes = vec![(); 2];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(op("store", vec![Operand::Use(v(0))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let converted = to_lir::<HighFirst>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, false).expect("allocates");
    assert_eq!(decided.placed.iter().find(|(v, _)| *v == 1).map(|(_, r)| *r), Some(RegId::BH), "{:?}", decided.placed);
}

/// A register set in one block and read in the next, with a value that lives
/// only between them. The converter began every block with no memory of the
/// registers, so the read became a fresh input and the set a dead def: the
/// register looked free in between and the value took `ebx` (first in its
/// order), clobbering what the next block reads.
fn set_here_read_there() -> MFunction<()> {
    let mut f = MFunction::<()>::new("flow");
    f.classes = vec![(); 2];
    let mut first = Block { id: 0, ..Block::default() };
    first.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX))]));
    first.insts.push(op("load", vec![Operand::Def(v(0))]));
    first.insts.push(op("store", vec![Operand::Use(v(0))]));
    first.insts.push(branch(1));
    let mut second = Block { id: 1, ..Block::default() };
    second.insts.push(op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::EBX))]));
    second.insts.push(op("store", vec![Operand::Use(v(1))]));
    second.insts.push(ret());
    f.blocks = vec![first, second];
    f
}

#[test]
fn test_a_register_set_in_one_block_is_held_to_its_read_in_the_next() {
    let converted = to_lir::<Flat>(&lowered(set_here_read_there()), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, true)
        .expect("greedy refused: the set and the read are two pinned values meeting in ebx");
    let place = |value| decided.placed.iter().find(|(v, _)| *v == value).map(|(_, r)| *r);
    assert_eq!(decided.report.stats.failed, 0, "{:?}", decided.placed);
    assert!(place(1).is_some_and(|r| r != RegId::EBX), "the value between the set and the read: {:?}", decided.placed);
}

/// A register read in a later block and set on no path to it is live on entry;
/// one set on every path is not.
#[test]
fn test_only_a_register_read_before_any_set_is_an_input() {
    let mut f = set_here_read_there();
    f.blocks[1].insts.insert(1, op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::ECX))]));
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let pinned = |register| converted.body.pins.iter().find(|(_, r)| **r == register).map(|(v, _)| *v).unwrap();
    assert!(converted.body.inputs.contains(&pinned(RegId::ECX)));
    assert!(!converted.body.inputs.contains(&pinned(RegId::EBX)));
}

/// An incoming pair that is read first and a byte of the same cell written
/// later (`take`: `x` arrives in `rs1`, then `rc2` carries a call argument)
/// are never live together: the 6502's kernels do this, and it must allocate.
#[test]
fn test_a_pair_read_then_one_of_its_bytes_set_allocates() {
    let mut f = set_here_read_there();
    f.blocks[0].insts[0] = op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::EBX))]);
    f.blocks[1].insts[0] = op("set", vec![Operand::Def(Reg::Phys(RegId::BL))]);
    f.blocks[1].insts.insert(1, op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::BL))]));
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    assert!(decide_with_split(&inputs, Mode::Simple, true).is_some(), "the pair is dead before its byte is set");
}

/// A pair set and then read as a byte is one cell seen two ways, live
/// together: greedy refuses the function rather than place a value in it.
#[test]
fn test_a_pair_set_then_read_as_a_byte_is_refused() {
    let mut f = set_here_read_there();
    f.blocks[1].insts[0] = op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::BL))]);
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    assert!(decide_with_split(&inputs, Mode::Simple, true).is_none());
}
