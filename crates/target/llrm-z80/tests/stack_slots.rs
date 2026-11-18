//! Spill slots become `push` and `pop` only where that is what the code does: a
//! byte slot saves its pair, so the other half must be dead once the pair is
//! restored.

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::compile::{frameless_slots_are_a_stack, nest_reloads, slots_are_a_stack, stack_slot_report_by};
use llrm_z80::reg;
use llrm_z80::regalloc::Class;

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

/// `c` is written between the push and the pop and read after: `pop bc` would
/// put the old `c` back.
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

fn word_function(tail: Vec<Inst>) -> MFunction<Class> {
    let mut insts = vec![Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::DE))])];
    insts.extend(tail);
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    function
}

/// A slot read twice stayed in the frame (`LoadNotOnTop` at the second read),
/// so the `pop bc; push bc` read of the slot on top was lost and two IX loads
/// stayed: the first read now leaves it on the stack, the last pops it.
#[test]
fn a_slot_read_twice_is_peeked_then_popped() {
    let reload = || Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Frame(0)]);
    let mut f = word_function(vec![reload(), reload()]);
    assert!(slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    let kinds: Vec<Kind> = f.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds, [Kind::Op("PUSHP"), Kind::Op("PEEKP"), Kind::Op("POPP")]);
}

/// Four words saved and read back the way a queue is read, three of them under
/// the fourth, which is read last: the slot loaded first was sent to the
/// frame, then the next, then the next, and only the top one stayed a `push`
/// (1 of 4). The top one is in every conflict, so it goes: three stay.
#[test]
fn the_slot_in_the_most_conflicts_goes_back_to_the_frame() {
    let save = |slot| Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(slot), Operand::Use(Reg::Phys(reg::DE))]);
    let read = |slot| Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Frame(slot)]);
    let body = || {
        let mut f = MFunction::new("f");
        let insts = vec![save(0), save(1), save(2), save(3), read(2), read(1), read(0), read(3)];
        f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
        f
    };
    let first_violation = stack_slot_report_by(&mut body(), &Assignment(Vec::new()), false, false);
    assert_eq!(first_violation.pushed, [3]);
    let by_conflicts = stack_slot_report_by(&mut body(), &Assignment(Vec::new()), false, true);
    assert_eq!(by_conflicts.pushed, [0, 1, 2]);
}

/// x87_arguments -O2 (with the soft-float bitcast split of #8590) was REFUSED "a pushed spill inside the stack
/// arguments of a call": the whole-function stack form made every spill a `push`/`pop`, those between the pushes of
/// a call's arguments included, and the printer cannot place them. They stay slots; outside the arguments the same
/// spill is a stack.
#[test]
fn a_spill_inside_the_stack_arguments_of_a_call_stays_a_slot() {
    let adjust = |kind, bytes| Inst::new(Kind::Op(kind), vec![Operand::Imm(bytes)]);
    let save = || Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::DE))]);
    let read = || Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Frame(0)]);
    let build = |insts: Vec<Inst>| {
        let mut f = MFunction::new("f");
        f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
        f
    };
    let mut inside = build(vec![adjust("ADJCALLSTACKDOWN", 4), save(), read(), adjust("ADJCALLSTACKUP", 4)]);
    assert!(!frameless_slots_are_a_stack(&mut inside, &Assignment(Vec::new())));
    assert_eq!(inside.blocks[0].insts[1].kind, Kind::Op("SPILL16"));
    let mut outside = build(vec![adjust("ADJCALLSTACKDOWN", 4), adjust("ADJCALLSTACKUP", 4), save(), read()]);
    assert!(frameless_slots_are_a_stack(&mut outside, &Assignment(Vec::new())));
    assert_eq!(outside.blocks[0].insts[2].kind, Kind::Op("PUSHP"));
}

/// A byte saved from `e` and reloaded into `d` came back through `pop de` as it was in `d`: the frameless form
/// pushed and popped a pair whatever half each access named (memory_expansion `ref_fill` -Os stored 0 for 123).
#[test]
fn a_byte_slot_saved_from_one_half_and_reloaded_into_the_other_stays_a_slot() {
    let mut f = MFunction::new("f");
    let insts = vec![
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::E))]),
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Phys(reg::D)), Operand::Frame(0)]),
    ];
    f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    assert!(!frameless_slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    assert_eq!(f.blocks[0].insts[0].kind, Kind::Op("SPILL8"));
}

/// huffman_decode -O1: a loop counter spilled from `c` and reloaded into `b`
/// stayed in the frame (`DifferentLane`, 6 B of IX loads and stores a round
/// trip). `push bc` / `pop bc` brings the low byte back into `c`, so the
/// reload into `b` is the pop and `ld b, c` (3 B), with `c`, which the pop
/// overwrites, dead afterwards.
#[test]
fn a_byte_loaded_into_the_other_half_of_a_pair_is_a_pop_and_a_copy() {
    let save = || Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::C))]);
    let load = || Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Phys(reg::B)), Operand::Frame(0)]);
    let body = |tail: Inst| {
        let mut f = MFunction::new("f");
        let insts = vec![save(), Inst::new(Kind::Op("DEF"), vec![Operand::Def(Reg::Phys(reg::E)), Operand::Imm(0)]), load(), tail];
        f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
        f
    };
    let mut f = body(Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Phys(reg::B))]));
    assert!(slots_are_a_stack(&mut f, &Assignment(Vec::new())));
    let insts = &f.blocks[0].insts;
    assert_eq!(insts[0].kind, Kind::Op("PUSHP"));
    assert_eq!(insts[2].kind, Kind::Op("POPP"));
    assert_eq!(insts[3].kind, Kind::Copy);
    assert_eq!(insts[3].operands, [Operand::Def(Reg::Phys(reg::B)), Operand::Use(Reg::Phys(reg::C))]);
    // `c` live after the reload: the pop would overwrite it
    let mut live = body(Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Phys(reg::C))]));
    assert!(!slots_are_a_stack(&mut live, &Assignment(Vec::new())));
    assert_eq!(live.blocks[0].insts[0].kind, Kind::Op("SPILL8"));
}

/// Two stores from different halves reach one load: no pop can bring back both.
#[test]
fn a_byte_slot_stored_from_both_halves_stays_in_the_frame() {
    let mut f = MFunction::new("f");
    let insts = vec![
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::C))]),
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::B))]),
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Phys(reg::B)), Operand::Frame(0)]),
    ];
    f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    assert!(!slots_are_a_stack(&mut f, &Assignment(Vec::new())));
}

/// overflow_builtins -O1 (+249 B, +11,934 T) when the slots that need a copy
/// joined the others in the conflict count: they displaced slots that were
/// pushes. They are tried after the others and kept only where they take none.
#[test]
fn a_slot_that_needs_a_copy_never_takes_another_slot_from_the_stack() {
    let mut f = MFunction::new("f");
    let insts = vec![
        Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(0), Operand::Use(Reg::Phys(reg::DE))]),
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(1), Operand::Use(Reg::Phys(reg::C))]),
        // slot 0 is read with slot 1 on top of it
        Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(reg::HL)), Operand::Frame(0)]),
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Phys(reg::B)), Operand::Frame(1)]),
    ];
    f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    let report = stack_slot_report_by(&mut f, &Assignment(Vec::new()), false, false);
    assert_eq!(report.pushed, [0]);
    assert_eq!(f.blocks[0].insts[1].kind, Kind::Op("SPILL8"));
}

/// switch_peeled_table -O1: two words saved around a call and read back in the order they were saved left the
/// first one in the IX frame (`ld -2(ix), c` ... `ld c, -2(ix)`, 12 B and a frame of 13 B) where reads in the
/// reverse order make both a `push`/`pop`.
#[test]
fn words_saved_in_order_and_read_in_order_are_read_in_reverse_so_both_pop() {
    let save = |slot, from| Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(slot), Operand::Use(Reg::Phys(from))]);
    let read = |slot, into| Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(into)), Operand::Frame(slot)]);
    let body = || {
        let mut f = MFunction::new("f");
        f.blocks = vec![Block { id: 0, insts: vec![save(0, reg::BC), save(1, reg::DE), read(0, reg::BC), read(1, reg::DE)], phis: Vec::new() }];
        f
    };
    let crossed = stack_slot_report_by(&mut body(), &Assignment(Vec::new()), false, false);
    assert_eq!(crossed.pushed.len(), 1, "the control: without the pass one slot stays in the frame");
    let mut f = body();
    assert!(nest_reloads(&mut f, &Assignment(Vec::new())));
    let nested = stack_slot_report_by(&mut f, &Assignment(Vec::new()), false, false);
    assert_eq!(nested.pushed, [0, 1]);
}
