use super::*;
use crate::model::ir::{Addr, Mem, Space};
use crate::model::lir::LirBlock;
use crate::support::hash::IndexMap;

fn fp(at: usize) -> Loc {
    Loc::Reg(Reg { register: llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO).rfp_registers()[at], width: 10 })
}

fn cell() -> Loc {
    Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, -8)), 8))
}

fn insn(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

fn copy(
    at: i64,
    dest: usize,
    source: usize,
) -> Arc<Insn> {
    insn(at, Operation::Move, "mov", vec![fp(dest)], vec![fp(source)])
}

fn store(
    at: i64,
    source: usize,
) -> Arc<Insn> {
    insn(at, Operation::FloatStore, "fstp", vec![cell()], vec![fp(source)])
}

fn load(
    at: i64,
    dest: usize,
) -> Arc<Insn> {
    insn(at, Operation::FloatLoad, "fld", vec![fp(dest)], vec![cell()])
}

fn body(blocks: Vec<LirBlock>) -> LirBody {
    let mut made = LirBody::new("fp", 0, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
    made.registers = Some(Regs(&llrm_x86_m16::REGISTER_INFO));
    made
}

/// Each instruction's name and sources.
fn shape(made: &LirBody) -> Vec<(String, Vec<Loc>)> {
    made.blocks[0]
        .insns
        .iter()
        .map(|one| {
            let what = one.what.as_ref().unwrap();
            (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.sources.clone())
        })
        .collect()
}

/// A copy read once stayed, and the stackifier made it a `fld st(i)` and a pop:
/// nbody's FP instructions were 101 with the copies, 95 without.
#[test]
fn test_a_read_of_a_copy_reads_its_source_and_the_dead_copy_goes() {
    let block = LirBlock::new(0, vec![load(0, 1), copy(1, 3, 1), store(2, 3)]);
    let made = propagated(&body(vec![block])).expect("a copy");
    let shaped = shape(&made);
    assert_eq!(shaped[2], ("fstp".to_owned(), vec![fp(1)]));
    assert_eq!(shaped[1].0, "", "the copy is an anchor");
}

/// The source rewritten between the copy and the read: the read keeps the copy.
#[test]
fn test_a_copy_whose_source_is_rewritten_is_read_through_itself() {
    let block = LirBlock::new(0, vec![load(0, 1), copy(1, 3, 1), load(2, 1), store(3, 3)]);
    let made = propagated(&body(vec![block])).expect("a copy");
    let shaped = shape(&made);
    assert_eq!(shaped[1].0, "mov");
    assert_eq!(shaped[3], ("fstp".to_owned(), vec![fp(3)]));
}

/// A copy live out of its block is read in the next one.
#[test]
fn test_a_copy_live_out_of_the_block_stays() {
    let mut first = LirBlock::new(0, vec![load(0, 1), copy(1, 3, 1)]);
    first.succ = vec![1].into();
    let second = LirBlock::new(1, vec![store(2, 3)]);
    let made = propagated(&body(vec![first, second])).expect("a copy");
    assert_eq!(made.blocks[0].insns[1].what.as_ref().unwrap().name.as_deref(), Some("mov"));
}

/// A call empties the stack: a copy made before it is not read after it.
#[test]
fn test_a_call_ends_the_copies() {
    let call = Arc::new(Insn::new(
        2,
        Some((2, 2)),
        Some(Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) }),
        vec![],
        vec![],
    ));
    let block = LirBlock::new(0, vec![load(0, 1), copy(1, 3, 1), call, store(3, 3)]);
    let made = propagated(&body(vec![block])).expect("a copy");
    assert_eq!(shape(&made)[3], ("fstp".to_owned(), vec![fp(3)]));
}

/// A copy of a register that is copied again repeats the first.
#[test]
fn test_a_copy_that_repeats_a_valid_one_is_dropped() {
    let block = LirBlock::new(0, vec![load(0, 1), copy(1, 3, 1), copy(2, 3, 1), store(3, 3)]);
    let made = propagated(&body(vec![block])).expect("a copy");
    let shaped = shape(&made);
    assert_eq!(shaped[2].0, "");
    assert_eq!(shaped[3], ("fstp".to_owned(), vec![fp(1)]));
}

fn cell_at(disp: i64) -> Loc {
    Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), 8))
}

fn names(made: &LirBody) -> Vec<String> {
    made.blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().unwrap().name.as_deref().unwrap_or_default().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// pow_stack_depth rounded each double through a scratch cell and stored it on
/// (`fstp [t]; fld [t]; fstp [h]`): 1644 B at m16 -Os where main's is 1462.
#[test]
fn test_a_rounding_through_a_dead_cell_is_stored_once() {
    let first = insn(0, Operation::FloatStore, "fstp", vec![cell_at(-8)], vec![fp(0)]);
    let again = insn(1, Operation::FloatLoad, "fld", vec![fp(1)], vec![cell_at(-8)]);
    let home = insn(2, Operation::FloatStore, "fstp", vec![cell_at(-40)], vec![fp(1)]);
    let next = insn(3, Operation::FloatStore, "fstp", vec![cell_at(-8)], vec![fp(2)]);
    let made = propagated(&body(vec![LirBlock::new(0, vec![first, again, home, next])])).expect("a rounding");
    assert_eq!(names(&made), ["fstp", "fstp"]);
    assert_eq!(made.blocks[0].insns[2].what.as_ref().unwrap().sources, vec![fp(0)]);
}

#[test]
fn test_a_rounding_through_a_cell_read_later_stays() {
    let first = insn(0, Operation::FloatStore, "fstp", vec![cell_at(-8)], vec![fp(0)]);
    let again = insn(1, Operation::FloatLoad, "fld", vec![fp(1)], vec![cell_at(-8)]);
    let home = insn(2, Operation::FloatStore, "fstp", vec![cell_at(-40)], vec![fp(1)]);
    let read = insn(3, Operation::FloatLoad, "fld", vec![fp(2)], vec![cell_at(-4)]);
    assert!(propagated(&body(vec![LirBlock::new(0, vec![first, again, home, read])])).is_none());
}

/// data_after_checked rounded a double through a cell and stored it back into
/// the same cell (`fstp [t]; fld [t]; fstp [t]`): 926 B at m16 -Os.
#[test]
fn test_a_rounding_stored_back_into_its_cell_is_stored_once() {
    let first = insn(0, Operation::FloatStore, "fstp", vec![cell_at(-8)], vec![fp(0)]);
    let again = insn(1, Operation::FloatLoad, "fld", vec![fp(1)], vec![cell_at(-8)]);
    let back = insn(2, Operation::FloatStore, "fstp", vec![cell_at(-8)], vec![fp(1)]);
    let read = insn(3, Operation::FloatLoad, "fld", vec![fp(2)], vec![cell_at(-8)]);
    let made = propagated(&body(vec![LirBlock::new(0, vec![first, again, back, read])])).expect("a rounding");
    assert_eq!(names(&made), ["fstp", "fld"]);
}
