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
/// which share one source position, end a block. The spill victim was looked for at the last
/// branch, where none is live: qb-qrender's d_faces.c was refused.
#[test]
fn test_a_compare_crowded_before_its_branches_spills() {
    let (body, mut frame) = before_float_assign();
    let cpu = crate::backend::cpu::profile("486").unwrap();

    // The premise, as `assigned` sees the body before its first spill.
    let mut premise = frame.clone();
    let loaded = _integer_loads(&body, Some(&mut premise), None).unwrap();
    let stored = _aliased(&_integer_stores(&loaded, Some(&mut premise), false).unwrap());
    let (block, position) =
        _crowded(&stored, &_floating_values(&stored)).expect("more than eight floats live somewhere");
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

/// Nine floats copy at once into a loop's phis (fpbench, #358). Each spilled
/// copy was stored after all nine were made, nine registers wide, and FloatAlloc refused the
/// body with "floating instruction requires too many stack operands".
#[test]
fn test_spilled_phi_copies_are_not_all_held_at_once() {
    refuses_nothing("x87phis", "_k");
}

/// Ten floats rotate through a loop: its phi copies are one cycle, each reading what the next
/// writes. A step of such copies fell back to all-at-once and was refused; the order is one
/// copy at a time, the cycle broken with one temporary.
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
    let mut alloc = crate::backend::floatalloc::FloatAlloc { frame: Some(Rc::new(RefCell::new(frame))) };
    let done = alloc.transform(assigned);
    assert!(done.is_ok(), "{:?}", done.err());
}
