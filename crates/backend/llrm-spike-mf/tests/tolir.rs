//! `MFunction` -> `LirBody`: what greedy reads of a spike function.

use llrm_codegen::greedy::shadow::{Mode, decide_with_split};
use llrm_codegen::greedy::tii::TargetInstrInfo;
use llrm_lir::registers::{RegId, Regs};
use llrm_lir::{Loc, Operation};
use llrm_spike_mf::regalloc::{Join, Side, Target, eliminate_phis, two_address};
use llrm_spike_mf::tolir::{from_lir, to_lir};
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

/// A split makes new values, and the class of each is its parent's: the rewrite
/// back to a machine function reads the parent of every value it did not
/// start with. Inline mode edits the function; the table of parents was not
/// returned, so a clone had no class to be written with.
#[test]
fn test_every_clone_a_split_makes_names_its_parent() {
    struct Frame(llrm_spike_mf::tolir::SpikeFrame);
    let f = lowered(across_a_call(4));
    let converted = to_lir::<Flat>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let mut frame = Frame(llrm_spike_mf::tolir::SpikeFrame::of(&f));
    let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame.0 }, true).expect("allocates");
    let first = f.classes.len() as u32 + 1 + converted.body.pins.len() as u32;
    let clones: Vec<u32> = decided.placed.iter().map(|(v, _)| *v).filter(|v| *v >= first).collect();
    assert!(!clones.is_empty(), "premise: the split made new values: {:?}", decided.placed);
    for clone in clones {
        assert!(decided.parents.contains_key(&clone), "clone {clone} has no parent: {:?}", decided.parents);
    }
}

/// What greedy left, checked on its own terms: at every instruction the
/// virtual registers live together hold distinct registers of `Flat`, and
/// none is live across a call (which loses them all).
fn assert_allocated(
    function: &MFunction<()>,
    assignment: &llrm_spike_mf::regalloc::Assignment,
) {
    use std::collections::BTreeSet;
    let virt = |operand: &Operand| match operand {
        Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => Some(*v),
        _ => None,
    };
    let succ = llrm_spike_mf::regalloc::successors(function);
    let mut live_in: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); function.blocks.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..function.blocks.len()).rev() {
            let mut live: BTreeSet<u32> = succ[b].iter().flat_map(|to| live_in[*to].iter().copied()).collect();
            for inst in function.blocks[b].insts.iter().rev() {
                for operand in &inst.operands {
                    if let (Operand::Def(Reg::Virt(v)), true) = (operand, true) {
                        live.remove(v);
                    }
                }
                for operand in &inst.operands {
                    if let Operand::Use(Reg::Virt(v)) = operand {
                        live.insert(*v);
                    }
                }
            }
            if live != live_in[b] {
                live_in[b] = live;
                changed = true;
            }
        }
    }
    for b in 0..function.blocks.len() {
        let mut live: BTreeSet<u32> = succ[b].iter().flat_map(|to| live_in[*to].iter().copied()).collect();
        for inst in function.blocks[b].insts.iter().rev() {
            let defs: Vec<u32> =
                inst.operands.iter().filter(|o| matches!(o, Operand::Def(_))).filter_map(virt).collect();
            let together: Vec<u32> = live.iter().copied().chain(defs.iter().copied()).collect();
            for (i, one) in together.iter().enumerate() {
                for other in &together[i + 1..] {
                    if one != other {
                        assert_ne!(
                            assignment.0[*one as usize], assignment.0[*other as usize],
                            "v{one} and v{other} are live together in block {b} at {:?}",
                            inst.kind
                        );
                    }
                }
            }
            if inst.call {
                assert!(
                    live.iter().all(|v| defs.contains(v)),
                    "v{:?} lives across a call in block {b}",
                    live.difference(&defs.iter().copied().collect())
                );
            }
            for d in &defs {
                live.remove(d);
            }
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    live.insert(*v);
                }
            }
        }
    }
}

fn rewritten(f: &MFunction<()>) -> llrm_spike_mf::tolir::Rewritten<()> {
    rewritten_with(f, false)
}

fn rewritten_with(
    f: &MFunction<()>,
    join: bool,
) -> llrm_spike_mf::tolir::Rewritten<()> {
    let mut converted = to_lir::<Flat>(f, regs()).expect("converts");
    if join {
        converted.coalesce(regs(), false);
    }
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let mut frame = llrm_spike_mf::tolir::SpikeFrame::of(f);
    let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true).expect("allocates");
    from_lir::<Flat>(f, &converted, &decided, regs()).expect("writes back")
}

/// A function that needs no spill comes back as it went in, with a register
/// for each value.
#[test]
fn test_a_function_that_needs_no_spill_comes_back_unchanged() {
    let f = lowered(set_here_read_there());
    let back = rewritten(&f);
    assert_eq!(format!("{:?}", back.function.blocks), format!("{:?}", f.blocks));
    assert_eq!(back.function.classes.len(), f.classes.len());
    assert!(back.slots.is_empty());
    assert_allocated(&back.function, &back.assignment);
}

/// A value live across a call lives in a slot there: the way back writes the
/// spill and the reload the target states, the registers hold, and a slot is
/// named for each (the emitter reads only these).
#[test]
fn test_what_greedy_spills_across_a_call_comes_back_as_stores_and_loads() {
    let f = lowered(across_a_call(4));
    let back = rewritten(&f);
    assert_allocated(&back.function, &back.assignment);
    // the control: the checker sees every value in one register
    let crowded = llrm_spike_mf::regalloc::Assignment(vec![RegId::EBX; back.assignment.0.len()]);
    let caught = std::panic::catch_unwind(|| assert_allocated(&back.function, &crowded));
    assert!(caught.is_err(), "the checker accepts values live together in one register");
    let count = |name| back.function.blocks.iter().flat_map(|b| &b.insts).filter(|i| i.kind == Kind::Op(name)).count();
    assert!(count("st") > 0 && count("ld") > 0, "{:?}", back.function.blocks);
    let slot_operands: std::collections::BTreeSet<i32> = back
        .function
        .blocks
        .iter()
        .flat_map(|b| &b.insts)
        .flat_map(|i| &i.operands)
        .filter_map(|o| if let Operand::Frame(slot) = o { Some(*slot) } else { None })
        .collect();
    assert_eq!(slot_operands, back.slots.iter().map(|(slot, _)| *slot).collect());
}

/// A block no path reaches may read what nothing defines. Greedy asserts that
/// every use has a reaching def, so the converter keeps such a block out of its
/// way and the way back returns it as it was (`spt`, a differential seed with
/// two such blocks, panicked "Use not jointly dominated by defs").
#[test]
fn test_a_block_no_path_reaches_is_kept_as_it_was() {
    let mut f = set_here_read_there();
    let mut stray = Block { id: 2, ..Block::default() };
    stray.insts.push(op("store", vec![Operand::Use(v(2))]));
    stray.insts.push(ret());
    f.classes.push(());
    f.blocks.push(stray);
    let f = lowered(f);
    let back = rewritten(&f);
    assert_eq!(format!("{:?}", back.function.blocks[2]), format!("{:?}", f.blocks[2]));
    assert_allocated(&back.function, &back.assignment);
}

/// A value copied into a register the next instruction reads, and read again
/// after it: without the coalescer it overlaps the register's pinned range and
/// lives elsewhere, copying in on every pass. Joined into the pinned value it
/// is the register, and the copy goes (the 6502's `COPY x, i` before an
/// indexed load: sum4 spent `txa; tay` and `clc; adc #1` where `inx` fits).
#[test]
fn test_a_copy_into_a_pinned_register_joins_into_it() {
    let mut f = MFunction::<()>::new("join");
    f.classes = vec![(); 1];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(op("load", vec![Operand::Def(v(0))]));
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Use(v(0))]));
    only.insts.push(op("get", vec![Operand::Def(Reg::Phys(RegId::ECX)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(op("store", vec![Operand::Use(v(0))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let f = lowered(f);
    let copies = |back: &llrm_spike_mf::tolir::Rewritten<()>| {
        back.function.blocks[0].insts.iter().filter(|inst| inst.kind == Kind::Copy).count()
    };
    assert_eq!(copies(&rewritten_with(&f, false)), 1, "premise: the copy stays without the coalescer");
    let joined = rewritten_with(&f, true);
    assert_eq!(copies(&joined), 0, "{:?}", joined.function.blocks[0].insts);
    let store = joined.function.blocks[0].insts.iter().find(|inst| inst.kind == Kind::Op("store")).unwrap();
    assert_eq!(store.operands, vec![Operand::Use(Reg::Phys(RegId::EBX))], "the value is the register now");
}

/// A value copied from the register an argument arrives in joins into that
/// register's pinned value, not the other way: the argument's range then keeps
/// other values out of the register. Joined the other way round the value was
/// an ordinary one with the register as its only choice, and values placed
/// before it took the register's cells: crc16's pointer spilled with 26 cells
/// free.
#[test]
fn test_a_value_copied_from_an_argument_register_stays_the_register() {
    let mut f = MFunction::<()>::new("arg");
    f.classes = vec![(); 1];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(op("store", vec![Operand::Use(v(0))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let f = lowered(f);
    let mut converted = to_lir::<Flat>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    let pinned: Vec<u32> = converted.body.pins.keys().copied().collect();
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided =
        decide_with_split(&inputs, Mode::Inline { frame: &mut llrm_spike_mf::tolir::SpikeFrame::of(&f) }, true)
            .expect("allocates");
    let survivors: Vec<u32> =
        decided.edited.as_ref().unwrap().blocks[0].insns.iter().flat_map(|i| i.uses.clone()).collect();
    assert!(survivors.iter().all(|value| pinned.contains(value)), "the store reads {survivors:?}, pinned {pinned:?}");
}

/// A value taken from the register an argument arrives in and put back after a
/// call: the call clobbers the register, so the value cannot be the register
/// across it. The coalescer joined it into the register anyway (the call
/// wrote no pinned value) and `rotv`'s `x` came back from a clobbered `rc2`:
/// `rotl 1 by 15` gave 0x4000 where 0x8000 was due.
#[test]
fn test_a_value_across_a_call_is_not_joined_into_a_register_the_call_clobbers() {
    let mut f = MFunction::<()>::new("keep");
    f.classes = vec![(); 1];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(call());
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Use(v(0))]));
    only.insts.push(op("get", vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let f = lowered(f);
    let back = rewritten_with(&f, true);
    let count = |name| back.function.blocks.iter().flat_map(|b| &b.insts).filter(|i| i.kind == Kind::Op(name)).count();
    assert!(
        count("st") > 0 && count("ld") > 0,
        "the value is kept in a slot across the call: {:?}",
        back.function.blocks[0].insts
    );
}

/// A call writes a pair and each of its bytes in one instruction: that is not
/// two values meeting in a register. Greedy refused every function that named
/// both (`pass`, `put`: a pointer argument in `rs1`, a byte in `rc2`).
#[test]
fn test_a_call_that_clobbers_a_pair_and_its_byte_is_no_conflict() {
    let mut f = MFunction::<()>::new("both");
    f.classes = vec![(); 2];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(call());
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(Reg::Phys(RegId::BL)), Operand::Use(v(0))]));
    only.insts.push(op("get", vec![Operand::Def(v(1)), Operand::Use(Reg::Phys(RegId::BL))]));
    only.insts.push(op("store", vec![Operand::Use(v(1))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    assert!(decide_with_split(&inputs, Mode::Simple, true).is_some(), "greedy refused the function");
}

/// A call that reads a pair as an argument and clobbers its bytes meets the
/// pair's range at the call itself, not before it (`put`'s memcpy call).
#[test]
fn test_a_call_that_reads_a_pair_and_clobbers_its_byte_is_no_conflict() {
    let mut f = MFunction::<()>::new("reads");
    f.classes = vec![(); 1];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX))]));
    let mut with_argument = call();
    with_argument.operands.push(Operand::Use(Reg::Phys(RegId::EBX)));
    only.insts.push(with_argument);
    only.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::BL))]));
    only.insts.push(op("get", vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::BL))]));
    only.insts.push(op("store", vec![Operand::Use(v(0))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    assert!(decide_with_split(&inputs, Mode::Simple, true).is_some(), "greedy refused the function");
}

/// A value copied out of a pair is not joined into the pair when one of its
/// bytes is written while the value lives: the join would make the value hold
/// the byte's cell. crc16's pointer, joined into `rs1` while `rc3` carried a
/// loop counter, spilled with 26 cells free (and the byte's range was the
/// coalescer's blind spot: it sees two unrelated pinned values).
#[test]
fn test_a_value_is_not_joined_into_a_pair_whose_byte_is_written_while_it_lives() {
    let mut f = MFunction::<()>::new("lanes");
    f.classes = vec![(); 1];
    let mut only = Block { id: 0, ..Block::default() };
    only.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(0)), Operand::Use(Reg::Phys(RegId::EBX))]));
    only.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::BL))]));
    only.insts.push(op("use", vec![Operand::Use(Reg::Phys(RegId::BL))]));
    only.insts.push(op("store", vec![Operand::Use(v(0))]));
    only.insts.push(ret());
    f.blocks = vec![only];
    let back = rewritten_with(&lowered(f), true);
    let store = back.function.blocks[0].insts.iter().find(|inst| inst.kind == Kind::Op("store")).unwrap();
    assert!(matches!(store.operands[0], Operand::Use(Reg::Virt(_))), "{:?}", back.function.blocks[0].insts);
}

/// Spill code that overwrites a register (the 6502's stack-mode spill is
/// `ldy #n; sta (rc0),y`) is written after greedy has chosen registers: a value
/// held in `y` across a spill would be lost. No virtual value is offered a
/// register the spill overwrites.
#[test]
fn test_no_value_is_offered_a_register_the_spill_code_overwrites() {
    struct Clobbering;
    impl Target for Clobbering {
        type Class = ();
        fn meet(
            _: (),
            _: (),
        ) -> Option<()> {
            Some(())
        }
        fn any() {}
        fn candidates(_: ()) -> Vec<RegId> {
            vec![RegId::EBX, RegId::ECX]
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
            Vec::new()
        }
        fn spill_clobbers() -> Vec<RegId> {
            vec![RegId::EBX]
        }
    }
    let converted = to_lir::<Clobbering>(&lowered(set_here_read_there()), regs()).expect("converts");
    assert_eq!(converted.order(1), vec![RegId::ECX]);
}

/// The first use of a callee-saved register costs a save and a restore, and
/// a range that has not been split yet is split rather than take it
/// (`tryAssignCSRFirstTime`). A cost that never reached greedy would leave the
/// range whole, whatever the register costs.
#[test]
fn test_the_cost_of_a_first_use_of_a_callee_saved_register_reaches_greedy() {
    struct Saver;
    impl Target for Saver {
        type Class = ();
        fn meet(
            _: (),
            _: (),
        ) -> Option<()> {
            Some(())
        }
        fn any() {}
        fn candidates(_: ()) -> Vec<RegId> {
            vec![RegId::EBX, RegId::ECX]
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
            vec![RegId::ECX]
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
    let splits_with = |cost: u32| {
        let f = lowered(across_a_call(1));
        let mut converted = to_lir::<Saver>(&f, regs()).expect("converts");
        converted.tri.csr_first_use_cost = cost;
        let order = |value: u32| converted.order(value);
        let inputs = converted.inputs(regs(), &order);
        let mut frame = llrm_spike_mf::tolir::SpikeFrame::of(&f);
        let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true).expect("allocates");
        (decided.parents.len(), decided.placed.iter().filter(|(_, r)| *r == RegId::EBX).count())
    };
    assert_eq!(splits_with(0), splits_with(0), "the control: the answer does not depend on chance");
    assert_ne!(splits_with(0), splits_with(1_000), "the cost never changed what greedy did");
}

/// A literal live across a call is made again where it is read, not stored and
/// loaded. The spiller puts that copy of the definition beside the read; the
/// way back wrote the read's own instruction in its place and refused it
/// ("a spill with no cell": 558 refusals in 300 differential programs).
#[test]
fn test_a_literal_made_again_after_a_call_comes_back_as_the_literal() {
    let mut f = MFunction::<()>::new("literal");
    f.classes = vec![(); 3];
    let mut entry = Block { id: 0, ..Block::default() };
    let mut literal = op("ldi", vec![Operand::Def(v(0)), Operand::Imm(7)]);
    literal.rematerializable = true;
    entry.insts.push(literal);
    entry.insts.push(op("load", vec![Operand::Def(v(1))]));
    entry.insts.push(call());
    entry.insts.push(op("add", vec![Operand::Def(v(2)), Operand::Use(v(1)), Operand::Use(v(0))]));
    entry.insts.push(op("store", vec![Operand::Use(v(2))]));
    entry.insts.push(ret());
    f.blocks = vec![entry];
    let back = rewritten(&lowered(f));
    let insts: Vec<&Inst> = back.function.blocks.iter().flat_map(|b| &b.insts).collect();
    let literals = insts.iter().filter(|i| i.kind == Kind::Op("ldi")).count();
    // the one before the call is gone: nothing reads it
    assert_eq!(literals, 1, "{insts:?}");
    let call_at = insts.iter().position(|i| i.call).unwrap();
    assert_eq!(insts[call_at + 1].kind, Kind::Op("ldi"), "{insts:?}");
    for ldi in insts.iter().filter(|i| i.kind == Kind::Op("ldi")) {
        assert!(matches!(ldi.operands.as_slice(), [Operand::Def(_), Operand::Imm(7)]), "{ldi:?}");
    }
    assert_allocated(&back.function, &back.assignment);
    every_read_is_set(&back.function).unwrap();
}

/// Every virtual register an instruction reads is set by one.
fn every_read_is_set(function: &MFunction<()>) -> Result<(), String> {
    let mut set = std::collections::BTreeSet::new();
    let mut read = std::collections::BTreeSet::new();
    for inst in function.blocks.iter().flat_map(|b| &b.insts) {
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(n)) => {
                    set.insert(*n);
                }
                Operand::Use(Reg::Virt(n)) => {
                    read.insert(*n);
                }
                _ => {}
            }
        }
    }
    match read.difference(&set).next() {
        Some(n) => Err(format!("%{n} is read and never set")),
        None => Ok(()),
    }
}

/// A target whose copies, spills and reloads overwrite the status flags (the
/// 6502's `lda`), and which says so: a compare and the instruction that reads
/// its flags are one instruction to greedy.
struct Flags;

impl Target for Flags {
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
    fn reads_flags_set_just_before(inst: &Inst) -> bool {
        inst.reads_flags
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

/// A reload for the instruction that reads the flags of a compare stood
/// between the two, and overwrote the flags: a conditional move took the wrong
/// side (differential seed 212, `RELOAD` between a `CMP` and its `CMOV`).
#[test]
fn test_nothing_stands_between_a_compare_and_the_instruction_that_reads_its_flags() {
    let mut f = MFunction::<()>::new("flags");
    f.classes = vec![(); 6];
    let mut entry = Block { id: 0, ..Block::default() };
    for n in 0..4 {
        entry.insts.push(op("load", vec![Operand::Def(v(n))]));
    }
    entry.insts.push(call());
    let mut compare = op("cmp", vec![Operand::Use(v(0))]);
    compare.sets_flags = true;
    entry.insts.push(compare);
    let mut select = op("cmov", vec![Operand::Def(v(4)), Operand::Use(v(1)), Operand::Use(v(2))]);
    select.reads_flags = true;
    entry.insts.push(select);
    entry.insts.push(op("add", vec![Operand::Def(v(5)), Operand::Use(v(4)), Operand::Use(v(3))]));
    entry.insts.push(op("store", vec![Operand::Use(v(5))]));
    entry.insts.push(ret());
    f.blocks = vec![entry];
    let f = lowered(f);
    let mut converted = to_lir::<Flags>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let mut frame = llrm_spike_mf::tolir::SpikeFrame::of(&f);
    let decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true).expect("allocates");
    let back = from_lir::<Flags>(&f, &converted, &decided, regs()).expect("writes back");
    let insts = &back.function.blocks[0].insts;
    let read = insts.iter().position(|i| i.reads_flags).unwrap();
    assert!(insts[read - 1].sets_flags, "{insts:?}");
    converted.coalesce(regs(), false);
}

/// A literal made again stands beside whatever instruction needs it, and
/// carries that instruction's position. Beside another literal, the way back
/// took the neighbour for its source and wrote the neighbour's immediate:
/// `ldi 0` came back as `ldi 3` (seed 850269 of the 6502 difftest: 171 for
/// 129).
#[test]
fn test_a_literal_made_again_beside_another_literal_keeps_its_own_immediate() {
    use llrm_codegen::model::ir::Held;
    let mut f = MFunction::<()>::new("neighbours");
    f.classes = vec![(); 2];
    let mut entry = Block { id: 0, ..Block::default() };
    let mut seven = op("ldi", vec![Operand::Def(v(0)), Operand::Imm(7)]);
    seven.rematerializable = true;
    entry.insts.push(seven);
    entry.insts.push(op("ldi", vec![Operand::Def(v(1)), Operand::Imm(9)]));
    entry.insts.push(op("store", vec![Operand::Use(v(0))]));
    entry.insts.push(op("store", vec![Operand::Use(v(1))]));
    entry.insts.push(ret());
    f.blocks = vec![entry];
    let f = lowered(f);
    let converted = to_lir::<Flat>(&f, regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let mut frame = llrm_spike_mf::tolir::SpikeFrame::of(&f);
    let mut decided = decide_with_split(&inputs, Mode::Inline { frame: &mut frame }, true).expect("allocates");
    let edited = decided.edited.as_mut().expect("an edited body");
    edited
        .blocks[0]
        .insns
        .edit(
            |insns| {
                let nine = insns.iter().position(|one| one.defines == [2]).expect("the second literal");
                let seven = insns.iter().position(|one| one.defines == [1]).expect("the first literal");
                let mut again = (*insns[seven]).clone();
                again.at = insns[nine].at;
                again.covers = Some((again.at, again.at));
                again.defines = vec![3];
                again.spill_reload = true;
                again.rematerialized = true;
                let what = again.what.as_mut().expect("semantics");
                what.dests = vec![Loc::Held(Held { value: 3, width: 1 })];
                let store = insns.iter().position(|one| one.uses == [1]).expect("the read");
                insns[store] = insns[store].renamed(&|name| if name == 1 { 3 } else { name });
                insns.insert(nine + 1, std::sync::Arc::new(again));
            },
        );
    let back = from_lir::<Flat>(&f, &converted, &decided, regs()).expect("writes back");
    let literals: Vec<i64> = back
        .function
        .blocks
        .iter()
        .flat_map(|b| &b.insts)
        .filter(|i| i.kind == Kind::Op("ldi"))
        .filter_map(|i| i.operands.iter().find_map(|o| if let Operand::Imm(n) = o { Some(*n as i64) } else { None }))
        .collect();
    assert_eq!(literals, vec![7, 9, 7], "{literals:?}");
}

/// A function states no branch odds, so greedy's block frequencies took every
/// edge as 1 in 2 and a loop body ran 1.0 times per entry: spill weights
/// treated loop code as cold (6502 `lz4` grew cycles by 545 over the spike
/// allocator). The edge that stays in a loop is 31 in 32, as LLVM's loop branch
/// heuristic.
#[test]
fn test_a_loop_body_runs_many_times_per_entry() {
    let mut f = MFunction::<()>::new("counted");
    f.classes = vec![(); 3];
    let mut entry = Block { id: 0, ..Block::default() };
    entry.insts.push(op("load", vec![Operand::Def(v(0))]));
    entry.insts.push(branch(1));
    let mut body = Block { id: 1, ..Block::default() };
    body.insts.push(op("add", vec![Operand::Def(v(1)), Operand::Use(v(0)), Operand::Use(v(0))]));
    let mut again = op("br", vec![Operand::Block(1), Operand::Block(2)]);
    again.terminator = true;
    body.insts.push(again);
    let mut exit = Block { id: 2, ..Block::default() };
    exit.insts.push(op("store", vec![Operand::Use(v(1))]));
    exit.insts.push(ret());
    f.blocks = vec![entry, body, exit];
    let converted = to_lir::<Flat>(&lowered(f), regs()).expect("converts");
    let freq = llrm_codegen::liveness::blockfrequency::MachineBlockFrequencyInfo::of(&converted.body);
    let loop_body = freq.get_block_freq_relative_to_entry_block(1);
    assert!(loop_body >= 8.0, "the loop body runs {loop_body} times per entry");
}

/// Three registers, and an `inc` that encodes shorter in the last.
struct IncInLast;

impl Target for IncInLast {
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

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }

    fn operand_hint(
        inst: &Inst,
        _: usize,
    ) -> Option<RegId> {
        (inst.kind == Kind::Op("inc")).then_some(RegId::EDX)
    }
}

/// A target says an instruction is cheaper in one register (the 6502's `inc`
/// is `inx` in `x` and `clc; adc #1` in `a`); greedy took the first register
/// of the class (`ebx` here) and never asked, as `getRegAllocationHints` of
/// Thumb2 does for its 16-bit encodings.
#[test]
fn test_a_value_an_instruction_is_cheaper_with_takes_that_register() {
    let mut f = MFunction::<()>::new("inc");
    f.classes = vec![(); 1];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(op("inc", vec![Operand::Def(v(0)), Operand::Use(v(0))]));
    block.insts.push(op("store", vec![Operand::Use(v(0))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let converted = to_lir::<IncInLast>(&lowered(f), regs()).expect("converts");
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, false).expect("allocates");
    let held = decided.placed.iter().find(|(value, _)| *value == 1).map(|(_, register)| *register);
    assert_eq!(held, Some(RegId::EDX), "{:?}", decided.placed);
}

/// The coalescer joins the values of a loop's phi copies and renames the
/// instructions; a hint kept by value named the value `inc` had before the
/// join, which no instruction held any more, so it never reached the allocator
/// (6502 corpus: the hook changed no row). Every register a hint names must be
/// one that an instruction holds.
#[test]
fn test_a_hint_names_a_value_its_instruction_holds_after_the_coalescer() {
    use llrm_codegen::greedy::tii::TargetInstrInfo;
    let mut f = MFunction::<()>::new("counted");
    f.classes = vec![(); 3];
    let mut entry = Block { id: 0, ..Block::default() };
    entry.insts.push(op("load", vec![Operand::Def(v(0))]));
    entry.insts.push(branch(1));
    let mut body = Block { id: 1, ..Block::default() };
    body.phis.push(llrm_spike_mf::Phi { result: v(1), incoming: vec![(0, v(0)), (1, v(2))] });
    body.insts.push(op("inc", vec![Operand::Def(v(2)), Operand::Use(v(1))]));
    let mut again = op("br", vec![Operand::Block(1), Operand::Block(2)]);
    again.terminator = true;
    body.insts.push(again);
    let mut exit = Block { id: 2, ..Block::default() };
    exit.insts.push(op("store", vec![Operand::Use(v(2))]));
    exit.insts.push(ret());
    f.blocks = vec![entry, body, exit];
    let mut converted = to_lir::<IncInLast>(&lowered(f), regs()).expect("converts");
    converted.coalesce(regs(), false);
    let mut named = 0;
    for block in &converted.body.blocks {
        for insn in block.insns.iter() {
            let held: Vec<u32> = insn.uses.iter().chain(&insn.defines).copied().collect();
            for (value, _) in converted.tii.preferred_registers(insn) {
                named += 1;
                assert!(held.contains(&value), "hint names v{value}, the instruction holds {held:?}");
            }
        }
    }
    assert!(named > 0, "no hint reached an instruction");
}

/// Two registers are all a narrow class allows; a wide class has three. A
/// target with `DECLINE` refuses a join that narrows a class.
struct Narrowing<const DECLINE: bool>;

impl<const DECLINE: bool> Target for Narrowing<DECLINE> {
    type Class = bool;

    fn meet(
        a: bool,
        b: bool,
    ) -> Option<bool> {
        Some(a || b)
    }

    fn any() -> bool {
        false
    }

    fn candidates(narrow: bool) -> Vec<RegId> {
        if narrow { vec![RegId::EBX, RegId::ECX] } else { REGISTERS.to_vec() }
    }

    fn view(
        root: RegId,
        _: bool,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }

    fn larger_order(order: &[RegId]) -> Option<Vec<RegId>> {
        (order.len() < REGISTERS.len()).then(|| REGISTERS.to_vec())
    }

    fn memory_saving(
        inst: &Inst,
        operand: usize,
    ) -> u32 {
        // `dec` in memory is one instruction; in a register it is three
        match (inst.kind, inst.operands.get(operand)) {
            (Kind::Op("dec"), Some(Operand::Use(_))) => 2,
            _ => 0,
        }
    }

    /// A join that takes the memory form from a value is worth less than the
    /// copy it saves: it saves one.
    fn should_coalesce(join: &Join) -> bool {
        let loses = |side: &Side| side.registers.len() > join.joined.len() && side.memory_saving > 1;
        !DECLINE || !(loses(&join.source) || loses(&join.destination))
    }
}

/// A copy of a wide value into a narrow class (the 6502's compare takes `a`,
/// `x` or `y`) joined the two, and the joined value took the narrow class for
/// its whole range: it could not sit in the register the code after the
/// compare keeps busy and went to a slot (6502 strrchr: `tay; sty s_...`,
/// +52 cycles; this pin's coalescer joins it, and greedy splits it again).
#[test]
fn test_a_copy_into_a_narrow_class_does_not_narrow_the_value_that_lives_past_it() {
    let mut f = MFunction::<bool>::new("narrowing");
    f.classes = vec![false, true];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(1)), Operand::Use(v(0))]));
    block.insts.push(op("compare", vec![Operand::Use(v(1))]));
    block.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Def(Reg::Phys(RegId::ECX))]));
    block.insts.push(op("get", vec![Operand::Use(Reg::Phys(RegId::EBX)), Operand::Use(Reg::Phys(RegId::ECX))]));
    block.insts.push(op("store", vec![Operand::Use(v(0))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let mut converted = to_lir::<Narrowing<false>>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, true).expect("allocates");
    assert_eq!((decided.report.stores, decided.report.reloads), (0, 0), "{:?}", decided.placed);
}

/// Every use of a value is a compare, so the joins narrow it to the compare
/// class for its whole range; a block between the two compares keeps both
/// narrow registers busy. Only a copy around each compare, with the stretch
/// between them in the wide class (`recomputeRegClass` over a piece with no
/// compare, `SingleInstrs` in the block split), keeps it out of a slot (6502
/// strrchr: `tay; sty s_...`, +3 B and +52 cycles a trip).
#[test]
fn test_a_value_whose_every_use_is_narrow_lives_between_them_in_the_wide_class() {
    let mut f = MFunction::<bool>::new("narrowing");
    f.classes = vec![false, true, true];
    let mut first = Block { id: 0, ..Block::default() };
    first.insts.push(op("load", vec![Operand::Def(v(0))]));
    first.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(1)), Operand::Use(v(0))]));
    first.insts.push(op("compare", vec![Operand::Use(v(1))]));
    first.insts.push(branch(1));
    let mut middle = Block { id: 1, ..Block::default() };
    middle.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Def(Reg::Phys(RegId::ECX))]));
    middle.insts.push(op("get", vec![Operand::Use(Reg::Phys(RegId::EBX)), Operand::Use(Reg::Phys(RegId::ECX))]));
    middle.insts.push(branch(2));
    let mut last = Block { id: 2, ..Block::default() };
    last.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(2)), Operand::Use(v(0))]));
    last.insts.push(op("compare", vec![Operand::Use(v(2))]));
    last.insts.push(ret());
    f.blocks = vec![first, middle, last];
    let mut converted = to_lir::<Narrowing<false>>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, true).expect("allocates");
    assert_eq!((decided.report.stores, decided.report.reloads), (0, 0), "{:?}", decided.placed);
}

/// The same value in one block: no region or block to split around, so the
/// piece around each compare comes from `tryInstructionSplit` (6502 strrchr's
/// loop body is one block).
#[test]
fn test_a_value_whose_every_use_is_narrow_is_split_around_each_instruction_in_one_block() {
    let mut f = MFunction::<bool>::new("narrowing");
    f.classes = vec![false, true, true];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(1)), Operand::Use(v(0))]));
    block.insts.push(op("compare", vec![Operand::Use(v(1))]));
    block.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Def(Reg::Phys(RegId::ECX))]));
    block.insts.push(op("get", vec![Operand::Use(Reg::Phys(RegId::EBX)), Operand::Use(Reg::Phys(RegId::ECX))]));
    block.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(2)), Operand::Use(v(0))]));
    block.insts.push(op("compare", vec![Operand::Use(v(2))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let mut converted = to_lir::<Narrowing<false>>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    let order = |value: u32| converted.order(value);
    let inputs = converted.inputs(regs(), &order);
    let decided = decide_with_split(&inputs, Mode::Simple, true).expect("allocates");
    assert_eq!((decided.report.stores, decided.report.reloads), (0, 0), "{:?}", decided.placed);
}

/// Two registers are all a narrow class allows; the wide class prefers a third.
struct Bundled;

impl Target for Bundled {
    type Class = bool;

    fn meet(
        a: bool,
        b: bool,
    ) -> Option<bool> {
        Some(a || b)
    }

    fn any() -> bool {
        false
    }

    fn candidates(narrow: bool) -> Vec<RegId> {
        if narrow { vec![RegId::EBX, RegId::ECX] } else { vec![RegId::EDX, RegId::EBX, RegId::ECX] }
    }

    fn view(
        root: RegId,
        _: bool,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }

    fn larger_order(order: &[RegId]) -> Option<Vec<RegId>> {
        (order.len() < 3).then(|| vec![RegId::EDX, RegId::EBX, RegId::ECX])
    }

    fn reads_flags_set_just_before(inst: &Inst) -> bool {
        inst.reads_flags
    }

    fn spill_store(
        _: bool,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("st"), vec![Operand::Frame(slot), Operand::Use(register)]))
    }

    fn spill_load(
        _: bool,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("ld"), vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

/// The compare and the instruction that reads its flags are one instruction to
/// greedy, and it names the reader's operands too. It named the compare's
/// alone, so a piece only the bundle names was widened to the larger class and
/// the reader read a register its class does not allow (6502 seed 116: `lda
/// #1` held in `a` across the compare's own `lda`, 157 for 19).
#[test]
fn test_a_bundle_names_the_operands_of_the_instruction_that_reads_its_flags() {
    let mut f = MFunction::<bool>::new("bundled");
    f.classes = vec![false, true, false, false];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    block.insts.push(op("load", vec![Operand::Def(v(2))]));
    block.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(1)), Operand::Use(v(0))]));
    block.insts.push(op("set", vec![Operand::Def(Reg::Phys(RegId::EBX)), Operand::Def(Reg::Phys(RegId::ECX))]));
    block.insts.push(op("get", vec![Operand::Use(Reg::Phys(RegId::EBX)), Operand::Use(Reg::Phys(RegId::ECX))]));
    let mut compare = op("cmp", vec![Operand::Use(v(2))]);
    compare.sets_flags = true;
    block.insts.push(compare);
    let mut select = op("cmov", vec![Operand::Def(v(3)), Operand::Use(v(1))]);
    select.reads_flags = true;
    block.insts.push(select);
    block.insts.push(op("store", vec![Operand::Use(v(3))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let mut converted = to_lir::<Bundled>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    let bundle = converted
        .body
        .blocks
        .iter()
        .flat_map(|b| b.insns.iter())
        .find(|i| i.what.as_ref().and_then(|w| w.name.as_deref()) == Some("BUNDLE"))
        .expect("the compare and its reader are one instruction");
    // the reader's operand is the value coalesced from v0 and v1 (narrow), and
    // the bundle says so
    let narrow = converted.tii.operand_orders(bundle).into_iter().filter(|(_, order)| order.len() == 2).count();
    assert_eq!(narrow, 1, "{:?}", converted.tii.operand_orders(bundle));
}

/// A counter `dec`ed in place and compared: the compare copy would join it
/// into the narrow class, which has no memory, and the `dec` loses its memory
/// form (6502 fib: `dec rc2` became `ldy rc5; dey; sty rc5; cpy #0`, +5 B).
/// A target whose rule weighs what the narrowed value's other instructions
/// save (`TargetRegisterInfo::shouldCoalesce`) keeps the copy.
fn counter(narrows: bool) -> usize {
    let mut f = MFunction::<bool>::new("counter");
    f.classes = vec![false, true];
    let mut block = Block { id: 0, ..Block::default() };
    block.insts.push(op("load", vec![Operand::Def(v(0))]));
    if narrows {
        block.insts.push(op("dec", vec![Operand::Def(v(0)), Operand::Use(v(0))]));
    }
    block.insts.push(Inst::new(Kind::Copy, vec![Operand::Def(v(1)), Operand::Use(v(0))]));
    block.insts.push(op("compare", vec![Operand::Use(v(1))]));
    block.insts.push(op("store", vec![Operand::Use(v(0))]));
    block.insts.push(ret());
    f.blocks = vec![block];
    let mut converted = to_lir::<Narrowing<true>>(&f, regs()).expect("converts");
    converted.coalesce(regs(), false);
    converted.body.blocks.iter().flat_map(|b| b.insns.iter()).filter(|i| i.copy_of().is_some()).count()
}

#[test]
fn test_a_target_can_decline_a_join_that_takes_a_memory_form_from_a_value() {
    assert_eq!(counter(true), 1, "the copy into the narrow class was joined");
}

/// The same rule joins where the value has no memory form to lose.
#[test]
fn test_a_join_that_takes_no_memory_form_goes_ahead() {
    assert_eq!(counter(false), 0, "the copy was kept");
}
