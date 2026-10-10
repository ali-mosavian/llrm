//! Port of tests/test_schedule.py: post-allocation instruction scheduling
//! must retain physical dependencies.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::*;
use crate::backend::cpu;
use crate::model::ir::{Addr, AddressRef, Mem, Reg, Semantics};
use crate::model::lir::LirBlock;
use crate::support::hash::IndexMap;

fn _insn(
    at: i64,
    operation: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Arc<Insn> {
    Arc::new(Insn::new(
        at,
        Some((at, at + 1)),
        Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(operation) }),
        Vec::new(),
        Vec::new(),
    ))
}

fn _body(insns: Vec<Arc<Insn>>) -> LirBody {
    LirBody::new("latency", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default())
}

fn reg(
    register: RegId,
    width: u32,
) -> Loc {
    Loc::Reg(Reg { register, width })
}

/// Python `is`: the same body, every occurrence the same object.
fn same(
    result: &LirBody,
    original: &LirBody,
) -> bool {
    result == original && result.insns().iter().zip(original.insns()).all(|(one, other)| Arc::ptr_eq(one, &other))
}

fn names(body: &LirBody) -> Vec<String> {
    body.insns().iter().map(|one| one.what.as_ref().unwrap().name.clone().unwrap()).collect()
}

fn dests(body: &LirBody) -> Vec<RegId> {
    body.insns()
        .iter()
        .map(|one| match &one.what.as_ref().unwrap().dests[0] {
            Loc::Reg(reg) => reg.register,
            other => panic!("{other:?}"),
        })
        .collect()
}

/// C matmul had independent register work after a multiply's consumer.
fn _latency_chain() -> LirBody {
    let multiply =
        _insn(0, Operation::Multiply, "imul", vec![reg(RegId::EAX, 4)], vec![reg(RegId::EAX, 4), reg(RegId::ECX, 4)]);
    let consume =
        _insn(1, Operation::Binary, "add", vec![reg(RegId::EAX, 4)], vec![reg(RegId::EAX, 4), reg(RegId::EDX, 4)]);
    let independent = _insn(2, Operation::Move, "mov", vec![reg(RegId::EBX, 4)], vec![reg(RegId::ESI, 4)]);
    _body(vec![multiply, consume, independent])
}

/// P6 emitted `imul; add; mov` although the move hides multiply latency.
#[test]
fn test_out_of_order_profile_fills_an_imul_dependency_gap() {
    let result = scheduled(&_latency_chain(), cpu::profile("P6").unwrap()).unwrap();

    assert_eq!(names(&result), ["imul", "mov", "add"]);
}

/// C CRC32 emitted `mov ax,bx; add eax,esi; mov di,si` across P6's merge stall.
#[test]
fn test_p6_partial_write_penalty_exposes_independent_work() {
    let partial = _insn(0, Operation::Move, "mov", vec![reg(RegId::AX, 2)], vec![reg(RegId::BX, 2)]);
    let wide_use =
        _insn(1, Operation::Binary, "add", vec![reg(RegId::EAX, 4)], vec![reg(RegId::EAX, 4), reg(RegId::ESI, 4)]);
    let independent = _insn(2, Operation::Move, "mov", vec![reg(RegId::DI, 2)], vec![reg(RegId::SI, 2)]);

    let result = scheduled(&_body(vec![partial, wide_use, independent]), cpu::profile("P6").unwrap()).unwrap();

    assert_eq!(dests(&result), [RegId::AX, RegId::DI, RegId::EAX]);
}

/// 386/486 have no safe latency-hiding issue window to exploit.
#[test]
fn test_in_order_profiles_keep_the_established_source_order() {
    let original = _latency_chain();

    assert!(same(&scheduled(&original, cpu::profile("386").unwrap()).unwrap(), &original));
    assert!(same(&scheduled(&original, cpu::profile("486").unwrap()).unwrap(), &original));
}

/// Pentium lost a U/V pair when `mov di,si` preceded prefixed `mov eax,ecx`.
#[test]
fn test_pentium_orders_a_prefixed_move_before_its_uv_pair() {
    let pairable = _insn(0, Operation::Move, "mov", vec![reg(RegId::DI, 2)], vec![reg(RegId::SI, 2)]);
    let u_only = _insn(1, Operation::Move, "mov", vec![reg(RegId::EAX, 4)], vec![reg(RegId::ECX, 4)]);

    let result = scheduled(&_body(vec![pairable, u_only]), cpu::profile("P5").unwrap()).unwrap();

    assert_eq!(dests(&result), [RegId::EAX, RegId::DI]);
}

/// P5 left `lea bx,[bp-4]; mov eax,ecx` unpaired by treating LEA as a load.
#[test]
fn test_pentium_pairs_a_frame_lea_after_an_independent_prefixed_move() {
    let address = AddressRef {
        through: RegId::BP,
        offset: -4,
        disp_width: 1,
        ..AddressRef::new(Some(Addr::new(Space::Frame, -4)))
    };
    let lea = _insn(0, Operation::Address, "lea", vec![reg(RegId::BX, 2)], vec![Loc::Address(address)]);
    // The 32-bit move has the operand-size prefix in this 16-bit mode and
    // consequently consumes P5's U pipe. GCC's Pentium model classifies a
    // non-prefixed LEA as U/V, so it may issue in the second V slot.
    let r#move = _insn(1, Operation::Move, "mov", vec![reg(RegId::EAX, 4)], vec![reg(RegId::ECX, 4)]);

    let result = scheduled(&_body(vec![lea, r#move]), cpu::profile("P5").unwrap()).unwrap();

    assert_eq!(dests(&result), [RegId::EAX, RegId::BX]);
}

/// A LEA with a non-frame symbol owns relocation/segment meaning, unlike a
/// frame address.
#[test]
fn test_scheduler_keeps_symbolic_or_nonframe_addresses_out_of_its_window() {
    let address = AddressRef {
        through: RegId::BX,
        offset: 0,
        disp_width: 2,
        ..AddressRef::new(Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }))
    };
    let lea = _insn(1, Operation::Address, "lea", vec![reg(RegId::DI, 2)], vec![Loc::Address(address)]);

    assert!(_safe(16, &lea).is_none());
}

/// A flag-producing add must not cross imul before a later flag reader.
#[test]
fn test_flag_writers_remain_in_program_order() {
    let multiply = Arc::clone(&_latency_chain().insns()[0]);
    let add =
        _insn(1, Operation::Binary, "add", vec![reg(RegId::EBX, 4)], vec![reg(RegId::EBX, 4), reg(RegId::ESI, 4)]);
    let independent = _insn(2, Operation::Move, "mov", vec![reg(RegId::EDI, 4)], vec![reg(RegId::EDX, 4)]);

    let result = scheduled(&_body(vec![multiply, add, independent]), cpu::profile("P6").unwrap()).unwrap();

    let names = names(&result);
    let index = |name: &str| names.iter().position(|one| one == name).unwrap();
    assert!(index("imul") < index("add"));
}

/// A potentially trapping source load must retain its position exactly.
#[test]
fn test_memory_access_is_a_hard_scheduling_boundary() {
    let multiply = Arc::clone(&_latency_chain().insns()[0]);
    let load = _insn(
        1,
        Operation::Move,
        "mov",
        vec![reg(RegId::EBX, 4)],
        vec![Loc::Mem(Mem { through: RegId::ESI, ..Mem::new(None, 4) })],
    );
    let consume = Arc::clone(&_latency_chain().insns()[1]);
    let independent = Arc::clone(&_latency_chain().insns()[2]);
    let original = _body(vec![multiply, load, consume, independent]);

    assert!(same(&scheduled(&original, cpu::profile("P6").unwrap()).unwrap(), &original));
}
