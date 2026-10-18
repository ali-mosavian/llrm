use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::abi::runtime::{EVERY, Reg};
use crate::backend::constpool::Pool;
use crate::backend::cpu::ProfileOrName;
use crate::backend::{frame, isel, target};
use crate::model::ir::Operation;

/// x87crowd.c's `_deep` through isel and the machine phases before
/// FloatAssign, under Borland C's medium model; and its frame.
fn before_float_assign() -> (LirBody, Frame) {
    before("x87crowd", "_deep")
}

/// `function` of tests/check/mir/`file`.ll, as `before_float_assign` takes it.
fn before(
    file: &str,
    function: &str,
) -> (LirBody, Frame) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../../tests/check/mir/{file}.ll"));
    let module = llrm_mir::parse::module(&std::fs::read_to_string(path).unwrap()).expect("parses");
    let clobbered = [Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Es, Reg::Flags];
    let abi = crate::abi::qb::HirAbi {
        runtime: crate::hir::model::RuntimeProfile::Freestanding,
        objects: Default::default(),
        preserved: EVERY.iter().copied().filter(|one| !clobbered.contains(one)).collect(),
        stack_check: None,
    };
    let cpu = crate::backend::cpu::profile("486").unwrap();
    let pool = Rc::new(RefCell::new(Pool::new(0)));
    let selected = isel::selected(
        &module,
        function,
        &abi,
        &mut pool.borrow_mut(),
        cpu,
        &target::BUILT_IN,
        isel::m16(),
        &llrm_x86_m16::M16,
        false,
        0,
    )
    .expect("selects");
    let mut made = frame::of(&selected.body, Some(&selected.calls), "", None).unwrap();
    made.floor = made.floor.min(-selected.depth);
    let shared = Rc::new(RefCell::new(made));
    let pinned = selected.body.pins.clone();
    let mut body = selected.body;
    for mut phase in crate::flow::machine(
        &pinned,
        Some(Rc::clone(&shared)),
        Some(Rc::clone(&pool)),
        Some(&selected.calls),
        false,
        ProfileOrName::Profile(cpu),
        &target::BUILT_IN,
        false,
    )
    .unwrap()
    {
        if phase.class_name() == "FloatAssign" {
            break;
        }
        body = phase.transform(body).unwrap();
    }
    let frame = shared.borrow().clone();
    (body, frame)
}

/// More than eight floats are live where a float compare and its branches,
/// which share one source position, end a block. The spill victim was looked
/// for at the last branch, where none is live: qb-qrender's d_faces.c was
/// refused.
#[test]
fn test_a_compare_crowded_before_its_branches_spills() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();

    // The premise, as `assigned` sees the body before its first spill.
    let mut premise = frame.clone();
    let loaded = _integer_loads(&body, Some(&mut premise), None).unwrap();
    let stored = _aliased(&_integer_stores(&loaded, Some(&mut premise), false).unwrap());
    let (block, position) = _crowded(&stored, &_floating_values(&stored), &spillplacement::bundles(&stored), 0)
        .expect("more than eight floats live somewhere");
    let insns = &stored.blocks.iter().find(|one| one.at == block).unwrap().insns;
    let shared: Vec<&Insn> = insns.iter().filter(|one| one.at == insns[position].at).map(|one| &**one).collect();
    let op = |one: &Insn| one.what.as_ref().map(|what| what.op);
    assert!(
        shared.iter().any(|one| op(one) == Some(Operation::Compare))
            && shared.last().is_some_and(|one| matches!(op(one), Some(Operation::Branch | Operation::Jump))),
        "the crowded point's position is a float compare and its branches: {shared:#?}"
    );

    let assigned = assigned(&body, Some(&mut frame), None, false, cpu);
    assert!(assigned.is_ok(), "{:?}", assigned.err());
}

/// Each spill took one more round, and each round wrote every block again: an
/// unrolled fpbench spilled 24 values in 25 rounds, a third of its compile.
/// A block is written again only when a victim reaches it.
#[test]
fn test_a_round_writes_only_the_blocks_a_new_spill_reaches() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();
    WRITTEN.with(|one| one.set(0));
    assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    let (rounds, written) = (ROUNDS.with(|one| one.get()), WRITTEN.with(|one| one.get()));
    assert!(rounds > 1, "the premise: a body that spills ({rounds} rounds)");
    assert!(
        written < rounds * body.blocks.len(),
        "{written} blocks written in {rounds} rounds of {}",
        body.blocks.len()
    );
}

/// `assigned` solved the body's liveness five times (d_faces -O2: 43 of the
/// pass's 128 M instructions): before and after aliasing, then again in
/// `_aliased`, `_homes` and `_allocnos`. The body is solved twice at most, once
/// before and once after aliasing, each a pair of block sets.
#[test]
fn test_the_body_liveness_is_solved_at_most_twice() {
    use crate::backend::allocate::SETS_BUILT;
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();
    SETS_BUILT.with(|one| one.set(0));
    assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    let built = SETS_BUILT.with(|one| one.get());
    assert!(built <= 4, "{built} block sets built: more than two solutions");
}

/// Nine floats copy at once into a loop's phis (fpbench, #358). Each spilled
/// copy was stored after all nine were made, nine registers wide, and
/// FloatAlloc refused the body with "floating instruction requires too many
/// stack operands".
#[test]
fn test_spilled_phi_copies_are_not_all_held_at_once() {
    refuses_nothing("x87phis", "_k");
}

/// Ten floats rotate through a loop: its phi copies are one cycle, each reading
/// what the next writes. A step of such copies fell back to all-at-once and was
/// refused; the order is one copy at a time, the cycle broken with one
/// temporary.
#[test]
fn test_a_cycle_of_spilled_phi_copies_is_ordered_not_held_at_once() {
    refuses_nothing("x87rotate", "_rot");
}

fn refuses_nothing(
    file: &str,
    function: &str,
) {
    let (body, mut frame) = before(file, function);
    let cpu = crate::backend::cpu::profile("486").unwrap();
    let assigned = assigned(&body, Some(&mut frame), None, false, cpu).expect("assigns");
    let mut alloc = crate::backend::floatalloc::FloatAlloc {
        frame: Some(Rc::new(RefCell::new(frame))),
        classes: crate::backend::classes::RegisterClasses::m16(),
    };
    let done = alloc.transform(assigned);
    assert!(done.is_ok(), "{:?}", done.err());
}

/// Every round that wrote a block worked out its steps and the floats live
/// after each again, though they are the block's own and no spill changes
/// them: nbody_single -O3 (35 rounds over 4 blocks) spent a tenth of
/// `floatassign` on it. A block's are worked out once.
#[test]
fn test_a_blocks_steps_and_liveness_are_worked_out_once() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();
    SHAPED.with(|one| one.set(0));
    assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    let (rounds, shaped) = (ROUNDS.with(|one| one.get()), SHAPED.with(|one| one.get()));
    assert!(rounds > 1, "the premise: a body that spills ({rounds} rounds)");
    assert!(shaped <= body.blocks.len(), "{shaped} shapes for {} blocks in {rounds} rounds", body.blocks.len());
}

/// A round wrote every step of a block again, though a new victim touches a
/// few of them: nbody_single -O3 (35 rounds over a 691-instruction block)
/// spent 531 Minstr in `floatassign`. A step is written again only when the
/// spill state of a root it touches changed.
#[test]
fn test_a_round_writes_only_the_steps_a_new_spill_reaches() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();
    STEPPED.with(|one| one.set(0));
    assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    let (rounds, stepped) = (ROUNDS.with(|one| one.get()), STEPPED.with(|one| one.get()));
    let insns: usize = body.blocks.iter().map(|one| one.insns.len()).sum();
    assert!(rounds > 2, "the premise: a body that spills ({rounds} rounds)");
    assert!(stepped < rounds * insns / 2, "{stepped} steps written in {rounds} rounds over {insns} instructions");
}

/// Every round looked for the first crowded block from the first block, and
/// the blocks before the last victim's were as they had been: R rounds over B
/// blocks looked at R x B (`fjoins` 512: `lir floatassign` 100 G, cubic). The
/// look resumes at the earlier of the last crowded block and the first block
/// the victim is in.
#[test]
fn test_a_round_looks_for_the_crowded_block_from_where_the_last_left_off() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();
    CROWDED_BLOCKS.with(|one| one.set(0));
    assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    let (rounds, looked) = (ROUNDS.with(|one| one.get()), CROWDED_BLOCKS.with(|one| one.get()));
    assert!(rounds > 1, "the premise: a body that spills ({rounds} rounds)");
    let blocks = body.blocks.len();
    // Resumed it is 37 for these 9 rounds; from the first block each time, 49.
    assert!(looked < 45, "{looked} blocks looked at over {rounds} rounds of {blocks}");
}

/// `lir floatassign` ran 13 times over d_faces -O1 for 137 M instructions,
/// mostly on bodies with no x87 work: its integer passes cloned every
/// instruction's semantics and rebuilt every block before the check for floats.
/// A body with no float is returned as it came, its blocks the same
/// allocations.
#[test]
fn test_a_body_with_no_float_is_returned_without_rebuilding_a_block() {
    let (body, mut frame) = before("tilesum", "_tile_sum");
    assert!(_floating_values(&body).is_empty(), "the premise: no float in it");
    let cpu = crate::backend::cpu::profile("486").unwrap();
    let again = assigned(&body, Some(&mut frame), None, false, cpu).unwrap();
    for (one, other) in body.blocks.iter().zip(&again.blocks) {
        assert!(one.insns.same_as(&other.insns), "block {:#x} rebuilt", one.at);
    }
}

/// `pressuresink` took the body's liveness as a set of values for each block,
/// blocks times values, though it asks of each load only whether a value is
/// live out of its block: the gate's `cases` program (128 and 256 cases) read
/// 10.3 and 33.3 Minstr there, x3.2 for twice the program. It asks the rows
/// of bits instead, and builds no set.
#[test]
fn test_pressuresink_builds_no_set_of_live_values_per_block() {
    use crate::backend::allocate::SETS_BUILT;
    let (body, _) = before_float_assign();
    SETS_BUILT.with(|one| one.set(0));
    crate::backend::pressuresink::sunk(
        &body,
        &crate::backend::target::BUILT_IN,
        &crate::backend::classes::RegisterClasses::m16(),
    );
    let built = SETS_BUILT.with(|one| one.get());
    assert_eq!(built, 0, "{built} block sets built");
}
