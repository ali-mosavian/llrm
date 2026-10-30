//! Port of `tests/test_floatalloc.py` and the floatalloc-only test in
//! `tests/test_float_constants.py`.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::{_truncating, allocated};
use crate::backend::cpu;
use crate::backend::floatassign::_equivalent_loads;
use crate::backend::frame::Frame;
use crate::backend::select;
use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::support::hash::HashMap;
use crate::support::hash::IndexMap;

pub(crate) fn sem(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
}

pub(crate) fn fl(value: u32) -> Loc {
    Loc::Held(Held { value, width: 10 })
}

pub(crate) fn m(cell: &Mem) -> Loc {
    Loc::Mem(cell.clone())
}

pub(crate) fn st(index: u32) -> Loc {
    Loc::st(index)
}

pub(crate) fn frame_cell(
    disp: i64,
    width: u32,
) -> Mem {
    Mem::new(Some(Addr::new(Space::Frame, disp)), width)
}

pub(crate) fn emits(what: &Semantics) -> bool {
    select::emit(what, 0, None, false, false, None).is_some()
}

pub(crate) fn name(one: &Insn) -> &str {
    one.what.as_ref().unwrap().name.as_deref().unwrap_or("")
}

pub(crate) fn what(one: &Insn) -> &Semantics {
    one.what.as_ref().unwrap()
}

fn held_values(args: &[Loc]) -> Vec<u32> {
    args.iter()
        .filter_map(|arg| match arg {
            Loc::Held(held) => Some(held.value),
            _ => None,
        })
        .collect()
}

pub(crate) fn _body(operations: Vec<Semantics>) -> LirBody {
    let insns = operations
        .into_iter()
        .enumerate()
        .map(|(index, what)| {
            let at = index as i64 * 8;
            let (defines, uses) = (held_values(&what.dests), held_values(&what.sources));
            Arc::new(Insn::new(at, Some((at, at + 8)), Some(what), defines, uses))
        })
        .collect();
    LirBody {
        float_stack: llrm_target::Target::float_stack(&llrm_x86_m16::M16),
        ..LirBody::new("floating", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default())
    }
}

pub(crate) fn block(
    at: i64,
    insns: Vec<Arc<Insn>>,
    succ: Vec<i64>,
) -> LirBlock {
    LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
}

/// As the pipeline runs it: phis are copies by now.
pub(crate) fn run(body: &LirBody) -> LirBody {
    allocated(&crate::phielimination::lowered(body).unwrap(), None, None, true, "386").unwrap()
}

pub(crate) fn with_frame(
    body: &LirBody,
    frame: &mut Frame,
) -> LirBody {
    allocated(&crate::phielimination::lowered(body).unwrap(), Some(frame), None, true, "386").unwrap()
}

fn arith(
    name: &str,
    d: f64,
    s: f64,
) -> f64 {
    match name {
        "fadd" => d + s,
        "fsub" => d - s,
        "fsubr" => s - d,
        "fmul" => d * s,
        "fdiv" => d / s,
        "fdivr" => s / d,
        _ => panic!("{name}"),
    }
}

/// Run allocated instructions over `memory`, cells to numbers: the memory after
/// and the stack left.
pub(crate) fn _x87(
    insns: &[Arc<Insn>],
    memory: &[(&Mem, f64)],
) -> (HashMap<Mem, f64>, Vec<f64>) {
    let (memory, stack, _) = _x87_traced(insns, memory);
    (memory, stack)
}

/// `_x87`, and every store to memory in order.
pub(crate) fn _x87_traced(
    insns: &[Arc<Insn>],
    memory: &[(&Mem, f64)],
) -> (HashMap<Mem, f64>, Vec<f64>, Vec<(Mem, f64)>) {
    let mut memory: HashMap<Mem, f64> = memory.iter().map(|(cell, value)| ((*cell).clone(), *value)).collect();
    let (mut stack, mut stores): (Vec<f64>, Vec<(Mem, f64)>) = (Vec::new(), Vec::new());
    for one in insns {
        let what = what(one);
        if matches!(
            what.op,
            Operation::Nothing | Operation::Jump | Operation::Branch
        ) {
            continue;
        }
        if what.op == Operation::Barrier {
            assert!(stack.is_empty());
            continue;
        }
        assert!(emits(what), "{what:?}");
        let read = |arg: &Loc, stack: &Vec<f64>, memory: &HashMap<Mem, f64>| match arg {
            stack_slot if stack_slot.st_index().is_some() => stack[stack_slot.st_index().unwrap() as usize],
            Loc::Mem(cell) => memory[cell],
            _ => panic!("{arg:?}"),
        };
        let index_of = |arg: &Loc| match arg.st_index() {
            Some(index) => index as usize,
            None => panic!("{arg:?}"),
        };
        let name = what.name.as_deref().unwrap_or("");
        match what.op {
            Operation::FloatLoad => {
                let value = what.sources.first().map_or(0.0, |source| read(source, &stack, &memory));
                stack.insert(0, value);
                assert!(stack.len() <= 8);
            }
            Operation::Exchange => {
                let index = index_of(&what.sources[1]);
                stack.swap(0, index);
            }
            Operation::FloatStore => match &what.dests[0] {
                slot if slot.st_index().is_some() => {
                    stack[slot.st_index().unwrap() as usize] = stack[0];
                    stack.remove(0);
                }
                Loc::Mem(cell) => {
                    memory.insert(cell.clone(), stack[0]);
                    stores.push((cell.clone(), stack[0]));
                    if name.ends_with('p') {
                        stack.remove(0);
                    }
                }
                other => panic!("{other:?}"),
            },
            Operation::FloatUnary => {
                assert_eq!(name, "fchs");
                stack[0] = -stack[0];
            }
            Operation::FloatArith => {
                let name = if name.starts_with("fi") { format!("f{}", &name[2..]) } else { name.to_owned() };
                let index = index_of(&what.dests[0]);
                let source = read(&what.sources[1], &stack, &memory);
                stack[index] = arith(&name, stack[index], source);
            }
            Operation::FloatArithPop => {
                let index = index_of(&what.dests[0]);
                stack[index] = arith(&name[..name.len() - 1], stack[index], stack[0]);
                stack.remove(0);
            }
            _ => {}
        }
    }
    (memory, stack, stores)
}

/// The instructions run along `path`, through any block allocation put on an
/// edge of it.
pub(crate) fn _along(
    result: &LirBody,
    path: &[i64],
) -> Vec<Arc<Insn>> {
    let by_at: HashMap<i64, &Block> = result.blocks.iter().map(|block| (block.at, block)).collect();
    let mut insns = Vec::new();
    for (index, at) in path.iter().enumerate() {
        insns.extend(by_at[at].insns.iter().cloned());
        if let Some(next) = path.get(index + 1).filter(|next| !by_at[at].succ.contains(next)) {
            let edge =
                by_at[at].succ.iter().find(|edge| by_at[*edge].succ == vec![*next]).expect("a block on the edge");
            insns.extend(by_at[edge].insns.iter().cloned());
        }
    }
    insns
}

fn _cells(
    displacements: impl IntoIterator<Item = i64>,
    width: u32,
) -> Vec<Mem> {
    displacements.into_iter().map(|disp| frame_cell(disp, width)).collect()
}

pub(crate) fn _load(
    value: u32,
    cell: &Mem,
) -> Semantics {
    sem(Operation::FloatLoad, "fld", vec![fl(value)], vec![m(cell)])
}

pub(crate) fn _store(
    cell: &Mem,
    value: u32,
) -> Semantics {
    sem(Operation::FloatStore, "fstp", vec![m(cell)], vec![fl(value)])
}

pub(crate) fn _arithmetic(
    name: &str,
    result: u32,
    left: Loc,
    right: Loc,
) -> Semantics {
    sem(Operation::FloatArith, name, vec![fl(result)], vec![left, right])
}

pub(crate) fn fchs(
    result: u32,
    source: u32,
) -> Semantics {
    sem(Operation::FloatUnary, "fchs", vec![fl(result)], vec![fl(source)])
}

fn reads(
    result: &LirBody,
    cell: &Mem,
) -> usize {
    result.insns().iter().filter(|one| what(one).sources.contains(&m(cell))).count()
}

/// With the registers allocated (RFP) no value is left to hold
/// the control word: the truncating word is made in memory alone. A `mov` of a
/// value the allocator never saw reached the encoder as an unplaced value.
#[test]
fn test_the_truncating_word_is_made_in_memory_under_rfp() {
    let insn = |at: i64, what: Semantics| Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]));
    let cell = Mem { through: RegId::BP, offset: 0, disp_width: 2, ..frame_cell(-2, 2) };
    let blocks =
        vec![block(0, vec![insn(0, sem(Operation::FloatStore, "fisttp", vec![m(&cell)], vec![st(0)]))], vec![])];
    let mut frame = Frame::new(-8);
    let result = _truncating(
        &LirBody::new("t", 0, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default()),
        Some(&mut frame),
    )
    .unwrap();
    assert!(
        result
            .insns()
            .iter()
            .all(|one| one.what.as_ref().is_none_or(|what| {
                what.dests.iter().chain(&what.sources).all(|place| !matches!(place, Loc::Held(_)))
            })),
        "a value is named"
    );
    assert_eq!(&names_of(&result, 0)[..3], ["fnstcw", "fnstcw", "or"]);
}

fn names_of(
    body: &LirBody,
    at: i64,
) -> Vec<&str> {
    let block = body.blocks.iter().find(|block| block.at == at).unwrap();
    block.insns.iter().map(|one| name(one)).filter(|name| !name.is_empty()).collect()
}

#[test]
fn test_a_load_read_by_several_arithmetics_is_loaded_once() {
    // NBODYS: `falloff` read by two multiplies. Priced by GCC's i386 table,
    // one load and two register multiplies cost less than two memory forms.
    let cells = _cells([-4, -8, -12, -16, -20], 4);
    let (x, y, falloff, px, py) = (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4]);
    let body = _body(vec![
        _load(1, x),
        _load(2, y),
        _load(3, falloff),
        _arithmetic("fmul", 4, fl(1), fl(3)),
        _store(px, 4),
        _arithmetic("fmul", 5, fl(2), fl(3)),
        _store(py, 5),
    ]);
    let result = run(&body);
    let reads = result.insns().iter().filter(|one| what(one).sources.contains(&m(falloff))).count();
    assert_eq!(reads, 1);
    let (memory, stack) = _x87(&result.insns(), &[(x, 3.0), (y, 5.0), (falloff, 0.5)]);
    assert_eq!((memory[px], memory[py], stack), (1.5, 2.5, vec![]));
}

#[test]
fn test_a_load_is_not_read_again_after_a_store_that_may_reach_it() {
    // Reading a cell again is the value only while nothing may have written it.
    for written in ["same", "indexed"] {
        let falloff = Mem::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0x10) }), 4);
        let target = if written == "same" {
            falloff.clone()
        } else {
            Mem {
                through: RegId::SI,
                base: Some(Held { value: 9, width: 2 }),
                disp_width: 2,
                ..Mem::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }), 4)
            }
        };
        let cells = _cells([-4, -8, -20], 4);
        let (x, y, py) = (&cells[0], &cells[1], &cells[2]);
        let body = _body(vec![
            _load(3, &falloff),
            _load(1, x),
            _arithmetic("fmul", 4, fl(1), fl(3)),
            _store(&target, 4),
            _load(2, y),
            _arithmetic("fmul", 5, fl(2), fl(3)),
            _store(py, 5),
        ]);
        let result = run(&body);
        assert_eq!(reads(&result, &falloff), 1, "{written}");
        let (memory, stack) = _x87(&result.insns(), &[(x, 3.0), (y, 5.0), (&falloff, 0.5)]);
        assert_eq!((memory[py], stack), (2.5, vec![]));
    }
}

#[test]
fn test_arithmetic_overwrites_the_operand_that_dies() {
    // Neither operand on top, the second dying: the first was exchanged up and
    // duplicated where one exchange does.
    let cells = _cells([-10, -20, -30, -40, -50, -60], 10);
    let (x, y, z, first, second, third) = (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4], &cells[5]);
    let mut operations = Vec::new();
    for (load, value, cell) in [(1, 4, x), (2, 5, y), (3, 6, z)] {
        operations.extend([_load(load, cell), fchs(value, load)]);
    }
    operations.extend([_arithmetic("fsub", 7, fl(4), fl(5)), _store(first, 7), _store(second, 6), _store(third, 4)]);
    let result = run(&_body(operations));
    let insns = result.insns();
    assert_eq!(insns.iter().filter(|one| name(one) == "fxch").count(), 1);
    assert!(!insns.iter().any(|one| name(one) == "fld" && what(one).sources[0].st_index().is_some()));
    let (memory, stack) = _x87(&insns, &[(x, 8.0), (y, 2.0), (z, 1.0)]);
    assert_eq!((memory[first], memory[second], memory[third], stack), (-6.0, -1.0, -8.0, vec![]));
}

/// A call that lists its writes as sparing the frame leaves the cell; one
/// that lists nothing may write it, and was read as writing nothing.
fn _sparing_the_frame(body: LirBody) -> LirBody {
    let call = Arc::new(crate::model::lir::CallMemory {
        effects: llrm_mir::memory::Effects { reads: false, writes: true },
        private: vec![crate::model::lir::WHOLE_FRAME],
        disturbs: Default::default(),
    });
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            block.with_insns(
                block
                    .insns
                    .iter()
                    .map(|one| match &one.what {
                        Some(what) if matches!(what.op, Operation::Call | Operation::Barrier) => {
                            Arc::new(Insn { call: Some(Arc::clone(&call)), ..Insn::clone(one) })
                        }
                        _ => Arc::clone(one),
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}

#[test]
fn test_square_keeps_the_next_used_operand_on_top() {
    // FPDEEP shuffled p back to the top immediately after forming p*p.
    let cell = frame_cell(-4, 4);
    let body = _body(vec![
        _load(1, &cell),
        _arithmetic("fmul", 2, fl(1), fl(1)),
        _arithmetic("fadd", 3, fl(1), fl(1)),
        _arithmetic("fdiv", 4, fl(2), fl(3)),
        _store(&cell, 4),
    ]);
    let result = run(&body);
    let insns = result.insns();
    assert!(!insns.iter().any(|one| name(one) == "fxch"));
    let product = what(insns.iter().find(|one| name(one) == "fmul").unwrap());
    assert_eq!(product.dests, vec![st(1)]);
    assert_eq!(product.sources, vec![st(1), st(0)]);
    assert!(insns.iter().all(|one| emits(what(one))));
}

#[test]
fn test_float_survives_fork_join_and_loop_without_rereading_source() {
    // A dominating extended value was refused at forks, joins and loop
    // boundaries. It now stays on the stack through all of them, with no
    // memory between.
    for path in [vec![0, 16, 48], vec![0, 32, 48], vec![0, 16, 16, 48]] {
        let cell = frame_cell(-20, 10);
        let source = frame_cell(-10, 10);
        let original = _body(vec![_load(1, &source), _store(&cell, 1)]);
        let (load, store) = (original.insns()[0].clone(), original.insns()[1].clone());
        let moved = |at: i64| Arc::new(Insn { at, covers: Some((at, at + 8)), ..(*store).clone() });
        let mut body = original.clone();
        body.blocks = vec![
            block(48, vec![moved(48)], vec![]).into(),
            block(0, vec![load], vec![16, 32]).into(),
            block(16, vec![moved(16)], vec![16, 48]).into(),
            block(32, vec![moved(32)], vec![48]).into(),
        ];
        let mut slots = Frame::new(-20);
        let before = slots.size();
        let result = with_frame(&body, &mut slots);
        assert_eq!(
            result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(),
            body.blocks.iter().map(|block| block.at).collect::<Vec<_>>()
        );
        let insns = _along(&result, &path);
        assert!(insns.iter().all(|one| emits(what(one))));
        assert_eq!(insns.iter().filter(|one| what(one).sources.contains(&m(&source))).count(), 1);
        // Python used Fraction(1) + 2**-63: any value only moved, never
        // computed.
        let precise = 1.25;
        let (_, stack, stores) = _x87_traced(&insns, &[(&source, precise)]);
        assert_eq!(stores, vec![(cell.clone(), precise); path.len() - 1]);
        assert!(stack.is_empty());
        assert_eq!(slots.size(), before);
    }
}

#[test]
fn test_floating_loop_phis_swap_in_parallel_on_the_critical_backedge() {
    // Floating loop phis were refused; serial slot copies would turn (1,2) into
    // (2,2).
    for target in [16, 48] {
        let cells: Vec<Mem> = [1, 2, 3].iter().map(|index| frame_cell(-10 * index, 10)).collect();
        let mut body = _body(vec![
            _load(1, &cells[0]),
            _load(2, &cells[1]),
            _store(&cells[2], 3),
            _store(&cells[2], 4),
            Semantics { target: Some(target), ..sem(Operation::Branch, "jne", vec![], vec![]) },
        ]);
        let insns = body.insns();
        body.blocks = vec![
            block(0, insns[..2].to_vec(), vec![16]).into(),
            LirBlock {
                phis: vec![
                    Phi { result: 3, incoming: vec![(0, 1), (16, 4)] },
                    Phi { result: 4, incoming: vec![(0, 2), (16, 3)] },
                ]
                .into(),
                ..block(16, insns[2..].to_vec(), vec![16, 48])
            }
            .into(),
            block(48, vec![], vec![]).into(),
        ];
        let result = with_frame(&body, &mut Frame::new(-30));
        let insns = _along(&result, &[0, 16, 16, 48]);
        assert!(
            insns.iter().all(|one| matches!(what(one).op, Operation::Branch | Operation::Jump) || emits(what(one)))
        );
        let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
        let answers: Vec<f64> = stores.iter().filter(|(cell, _)| *cell == cells[2]).map(|(_, value)| *value).collect();
        assert_eq!(answers, vec![1.0, 2.0, 2.0, 1.0]);
        assert!(stack.is_empty());
        // The swap renames the stack: nothing reaches memory but the answers.
        assert_eq!(stores.len(), 4);
    }
}

#[test]
fn test_live_store_uses_nonpopping_encoding_when_available() {
    // Exact-store reuse duplicated ST0 solely to pop the duplicate into memory.
    for width in [4, 8, 10] {
        let cell = frame_cell(-16, width);
        let body = _body(vec![_load(1, &cell), _store(&cell, 1), _store(&cell, 1)]);
        let result = run(&body);
        assert_eq!(
            result.insns().iter().map(|one| name(one)).collect::<Vec<_>>(),
            if width == 4 || width == 8 { vec!["fld", "fst", "fstp"] } else { vec!["fld", "fld", "fstp", "fstp"] }
        );
        assert!(result.insns().iter().all(|one| emits(what(one))));
    }
}

#[test]
fn test_buried_float_operand_is_exchanged_not_duplicated() {
    // A second live value must not prevent using the first, or reverse
    // subtraction.
    for operation in ["fsub", "fchs", "fstp", "fsubp"] {
        let cells = _cells([-4, -8, -12, -16], 4);
        let (first_cell, second_cell, answer, kept) = (&cells[0], &cells[1], &cells[2], &cells[3]);
        let (first_load, second_load, first, second, result) = (1, 2, 3, 4, 5);
        let mut operations = vec![
            _load(first_load, first_cell),
            fchs(first, first_load),
            _load(second_load, second_cell),
            fchs(second, second_load),
        ];
        match operation {
            "fsub" => operations.push(_arithmetic("fsub", result, fl(first), m(second_cell))),
            "fchs" => operations.push(fchs(result, first)),
            "fstp" => operations.push(_store(answer, first)),
            _ => operations.push(sem(Operation::FloatArithPop, "fsubp", vec![fl(result)], vec![fl(second), fl(first)])),
        }
        if operation != "fstp" {
            operations.push(_store(answer, result));
        }
        if operation != "fsubp" {
            operations.push(_store(kept, second));
        }
        let allocated = run(&_body(operations));
        let insns = allocated.insns();
        let moves: Vec<&Arc<Insn>> = insns
            .iter()
            .filter(|one| {
                let what = what(one);
                what.op == Operation::Exchange
                    || !what.sources.is_empty()
                        && what.sources[0].st_index().is_some()
                        && what.op == Operation::FloatLoad
            })
            .collect();
        // Both operands of the popping subtraction die, so the result
        // overwrites one in place.
        assert_eq!(
            moves.iter().map(|one| name(one)).collect::<Vec<_>>(),
            if operation == "fsubp" { vec![] } else { vec!["fxch"] }
        );
        assert!(moves.iter().all(|one| one.covers.unwrap().0 == one.covers.unwrap().1
            && one.uses.is_empty()
            && one.defines.is_empty()));
        let (memory, stack) = _x87(&insns, &[(first_cell, 7.0), (second_cell, 2.0)]);
        let expected = match operation {
            "fsub" => -9.0,
            "fchs" => 7.0,
            "fstp" => -7.0,
            _ => 5.0,
        };
        assert!(memory[answer] == expected && stack.is_empty(), "{operation}");
        assert!(operation == "fsubp" || memory[kept] == -2.0);
    }
}

#[test]
fn test_last_register_operand_is_consumed_without_reversing_arithmetic() {
    // Forwarded floating memory operands must die without leaking a stack slot
    // or reversing division.
    for (op, popping, expected) in
        [("fadd", "faddp", 9.0), ("fmul", "fmulp", 14.0), ("fsub", "fsubrp", 5.0), ("fdiv", "fdivrp", 3.5)]
    {
        for top in ["left", "right"] {
            let (left, right, result) = (1, 2, 3);
            // No arithmetic reads m80, so the loads stay register operands.
            let cell = frame_cell(-10, 10);
            let inputs = if top == "left" { [right, left] } else { [left, right] };
            let body = _body(vec![
                _load(inputs[0], &cell),
                _load(inputs[1], &cell),
                _arithmetic(op, result, fl(left), fl(right)),
                _store(&cell, result),
            ]);
            let allocated = run(&body);
            let insns = allocated.insns();
            assert!(!insns.iter().any(|one| name(one) == "fxch"));
            let (mut stack, mut stored): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
            let mut values = if top == "left" { [2.0, 7.0] } else { [7.0, 2.0] }.into_iter();
            for one in &insns {
                let what = what(one);
                assert!(emits(what));
                match name(one) {
                    "fld" => stack.insert(0, values.next().unwrap()),
                    "fxch" => {
                        let index = what.sources[1].st_index().expect("a stack register");
                        stack.swap(0, index as usize);
                    }
                    "fstp" => stored.push(stack.remove(0)),
                    "" => continue,
                    other => {
                        let want = if top == "left" { popping.to_owned() } else { popping.replace("rp", "p") };
                        assert_eq!(other, want);
                        let index = what.sources[0].st_index().expect("a stack register") as usize;
                        let (a, b) = (stack[index], stack[0]);
                        stack[index] = match other {
                            "faddp" => a + b,
                            "fmulp" => a * b,
                            "fsubp" => a - b,
                            "fdivp" => a / b,
                            "fsubrp" => b - a,
                            "fdivrp" => b / a,
                            _ => panic!(),
                        };
                        stack.remove(0);
                    }
                }
            }
            assert!(stored == vec![expected] && stack.is_empty(), "{op} {top}");
        }
    }
}

#[test]
fn test_shared_producer_is_kept_across_two_arithmetic_consumers() {
    // FPCSE's shared sum needs to survive multiplication before division reads
    // it.
    let cell = frame_cell(-4, 4);
    let body = _body(vec![
        _load(1, &cell),
        _arithmetic("fmul", 2, fl(1), m(&cell)),
        _store(&cell, 2),
        _arithmetic("fdiv", 3, fl(1), m(&cell)),
        _store(&cell, 3),
    ]);
    let result = run(&body);
    let insns = result.insns();
    assert_eq!(insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fld", "fld", "fmul", "fstp", "fdiv", "fstp"]);
    let duplicate = &insns[1];
    assert_eq!(what(duplicate).sources, vec![st(0)]);
    assert_eq!(what(duplicate).dests, vec![st(0)]);
    assert_eq!(select::emit(what(duplicate), 0, None, false, false, None).unwrap().code, [0xd9, 0xc0]);
    assert!(duplicate.covers == Some((8, 8)) && duplicate.call.is_none());
    assert_eq!(what(&insns[4]).sources, vec![st(0), m(&cell)]);
}

fn _memory_operand_body() -> (LirBody, Vec<Mem>) {
    let cells = _cells([-4, -8, -12], 4);
    let body = _body(vec![
        _load(1, &cells[0]),
        fchs(2, 1),
        _load(3, &cells[1]),
        _arithmetic("fmul", 4, fl(2), fl(3)),
        _store(&cells[2], 4),
    ]);
    (body, cells)
}

#[test]
fn test_x87_memory_operand_follows_the_selected_cpu_cost() {
    // A costly memory multiply must materialize the home instead of folding it.
    let (body, cells) = _memory_operand_body();
    let (source, home, out) = (&cells[0], &cells[1], &cells[2]);
    let base = cpu::profile("386").unwrap();
    let mut costs: IndexMap<String, i64> = base._costs.iter().cloned().collect();
    for (key, value) in [("x87_load", 1), ("x87_mul", 1), ("x87_mul_m", 99)] {
        costs.insert(key.to_owned(), value);
    }
    let slow_memory = cpu::Profile { name: "test-x87".to_owned(), _costs: costs.into_iter().collect(), ..base.clone() };

    let result = allocated(&body, None, None, true, &slow_memory).unwrap();

    let insns = result.insns();
    let multiply = what(insns.iter().find(|one| name(one).starts_with("fmul")).unwrap());
    assert!(multiply.sources.iter().all(|arg| arg.st_index().is_some()));
    let (memory, stack) = _x87(&insns, &[(source, 7.0), (home, 3.0)]);
    assert!(memory[out] == -21.0 && stack.is_empty());
}

#[test]
fn test_float_cell_write_breaks_reload_equivalence() {
    // A changed frame temporary must be loaded again, not reused from x87.
    let cells = _cells([-4, -8], 4);
    let (cell, out) = (&cells[0], &cells[1]);
    let body = _body(vec![_load(1, cell), fchs(2, 1), _store(cell, 2), _load(3, cell), _store(out, 3)]);

    let result = run(&body);

    let insns = result.insns();
    assert_eq!(insns.iter().filter(|one| name(one) == "fld" && what(one).sources == vec![m(cell)]).count(), 2);
    let (memory, stack) = _x87(&insns, &[(cell, 7.0)]);
    assert!(memory[cell] == -7.0 && memory[out] == -7.0 && stack.is_empty());
}

#[test]
fn test_volatile_float_load_breaks_reload_equivalence() {
    // A volatile scalar read is observable and may see device state change.
    let cell = frame_cell(-4, 4);
    let body = _body(vec![_load(1, &cell), _load(2, &cell), _load(3, &cell)]);
    let mut insns = body.insns();
    insns[1] = Arc::new(Insn { volatile: true, ..(*insns[1]).clone() });

    assert!(_equivalent_loads(&insns, &Default::default()).is_empty());
}

#[test]
fn test_popping_subtraction_preserves_reused_values() {
    // Shared FP values were refused when FSUBP consumed an operand used later.
    for live in ["left", "right", "both", "same", "same_dead"] {
        let cells = _cells([-4, -8, -12, -16, -20], 4);
        let (left_cell, right_cell, out, again, other) = (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4]);
        let (left_load, right_load, left, mut right, result) = (1, 2, 3, 4, 5);
        let mut operations = vec![_load(left_load, left_cell), fchs(left, left_load)];
        if live == "same" || live == "same_dead" {
            right = left;
        } else {
            operations.extend([_load(right_load, right_cell), fchs(right, right_load)]);
        }
        operations.extend([
            sem(Operation::FloatArithPop, "fsubp", vec![fl(result)], vec![fl(left), fl(right)]),
            _store(out, result),
        ]);
        if ["left", "both", "same"].contains(&live) {
            operations.push(_store(again, left));
        }
        if ["right", "both"].contains(&live) {
            operations.push(_store(other, right));
        }
        let (memory, stack) = _x87(&run(&_body(operations)).insns(), &[(left_cell, 7.0), (right_cell, 2.0)]);
        let expected: Vec<(&Mem, f64)> = match live {
            "left" => vec![(out, -5.0), (again, -7.0)],
            "right" => vec![(out, -5.0), (other, -2.0)],
            "both" => vec![(out, -5.0), (again, -7.0), (other, -2.0)],
            "same" => vec![(out, 0.0), (again, -7.0)],
            _ => vec![(out, 0.0)],
        };
        assert_eq!(
            expected.iter().map(|(cell, _)| memory[*cell]).collect::<Vec<_>>(),
            expected.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            "{live}"
        );
        assert!(stack.is_empty());
    }
}

// tests/test_float_constants.py
#[test]
fn test_a_loop_accumulator_stays_on_the_stack_across_the_back_edge() {
    // Every block edge was a region end: the running sum went through a
    // ten-byte cell each iteration.
    let (start, step, answer) = (frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8));
    let mut body = _body(vec![
        _load(1, &start),
        _load(3, &step),
        _arithmetic("fadd", 4, fl(2), fl(3)),
        Semantics { target: Some(16), ..sem(Operation::Branch, "jne", vec![], vec![]) },
        _store(&answer, 4),
    ]);
    let insns = body.insns();
    body.blocks = vec![
        block(0, insns[..1].to_vec(), vec![16]).into(),
        LirBlock {
            phis: vec![Phi { result: 2, incoming: vec![(0, 1), (16, 4)] }].into(),
            ..block(16, insns[1..4].to_vec(), vec![16, 48])
        }
        .into(),
        block(48, insns[4..].to_vec(), vec![]).into(),
    ];
    let result = with_frame(&body, &mut Frame::new(-24));
    let insns = _along(&result, &[0, 16, 16, 16, 48]);
    let (_, stack, stores) = _x87_traced(&insns, &[(&start, 1.0), (&step, 2.0)]);
    assert_eq!((stores, stack), (vec![(answer.clone(), 7.0)], vec![]));
    let looped: Vec<Arc<Insn>> = result
        .blocks
        .iter()
        .filter(|block| block.at == 16 || (block.at != 0 && block.succ == vec![16]))
        .flat_map(|block| block.insns.to_vec())
        .collect();
    let memory: Vec<&Semantics> = looped
        .iter()
        .map(|one| what(one))
        .filter(|what| what.sources.iter().chain(&what.dests).any(|arg| matches!(arg, Loc::Mem(_))))
        .collect();
    assert_eq!(memory.len(), 1, "{memory:?}");
    assert!(memory[0].sources.contains(&m(&step)));
}

/// Blocks of `(address, what it does, where it goes)` through `_truncating`: a
/// `c` is `(int)f`, an `r` a `ret`, an `a` an `fadd`, a `.` nothing.
fn switched(shape: &[(i64, &str, Vec<i64>)]) -> Vec<(i64, Vec<String>)> {
    let insn = |at: i64, what: Semantics| Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]));
    let cell = Mem { through: RegId::BP, offset: 0, disp_width: 2, ..frame_cell(-2, 2) };
    let blocks = shape
        .iter()
        .map(|(at, what, succ)| {
            let insns = what
                .chars()
                .map(|one| {
                    insn(
                        *at,
                        match one {
                            'c' => sem(Operation::FloatStore, "fisttp", vec![m(&cell)], vec![st(0)]),
                            'r' => sem(Operation::Return, "ret", vec![], vec![]),
                            'b' => sem(Operation::Branch, "jne", vec![], vec![]),
                            'a' => sem(Operation::FloatArith, "fadd", vec![st(0)], vec![st(0), st(1)]),
                            _ => sem(Operation::Nothing, "", vec![], vec![]),
                        },
                    )
                })
                .collect();
            block(*at, insns, succ.clone())
        })
        .collect();
    let mut frame = Frame::new(-8);
    let result =
        _truncating(&LirBody::new("t", 0, blocks, IndexMap::default(), IndexMap::default()), Some(&mut frame)).unwrap();
    result
        .blocks
        .iter()
        .map(|block| {
            (block.at, block.insns.iter().map(|one| name(one).to_owned()).filter(|name| !name.is_empty()).collect())
        })
        .collect()
}

#[test]
fn test_conversions_in_blocks_in_a_row_switch_the_rounding_mode_once_not_per_block() {
    // gcc's mode switching places a mode across blocks; each block switched
    // on and off for itself, four `fldcw` for two conversions.
    let result = switched(&[(0, ".", vec![5]), (5, "c", vec![6]), (6, "cr", vec![])]);
    let words = result.iter().flat_map(|(_, names)| names).filter(|name| *name == "fldcw").count();
    assert_eq!(words, 2, "{result:?}");
    assert_eq!(result[1].1, vec!["fistp"]);
}

#[test]
fn test_a_conversion_in_one_arm_of_a_branch_switches_in_that_arm_only() {
    let result = switched(&[(0, ".b", vec![5, 6]), (5, "c", vec![7]), (6, ".", vec![7]), (7, "r", vec![])]);
    let names = |at: i64| result.iter().find(|(block, _)| *block == at).unwrap().1.clone();
    assert_eq!(names(5), vec!["fldcw", "fistp", "fldcw"], "{result:?}");
    for at in [6, 7] {
        assert!(!names(at).contains(&"fldcw".to_owned()), "{result:?}");
    }
}

/// Two critical edges into one block, one from a block that converts and one
/// from a block that does not, cannot agree on its mode; every block then
/// restores the caller's, and the restore went after the branch: B$CASP of the
/// QB runtime failed to build with "block 12 leaves for (21, 19) with no
/// instruction choosing".
#[test]
fn test_a_restore_in_a_block_that_branches_goes_before_the_branch() {
    let result = switched(&[
        (0, ".b", vec![1, 2]),
        (1, "cb", vec![3, 4]),
        (2, ".b", vec![3, 4]),
        (3, "r", vec![]),
        (4, "r", vec![]),
    ]);
    let one = result.iter().find(|(at, _)| *at == 1).unwrap();
    assert_eq!(one.1, vec!["fldcw", "fistp", "fldcw", "jne"], "{result:?}");
}

/// fpbench's loop rounds (an `fmul`), converts, rounds and converts again each
/// trip: entering it truncating changes nothing a trip pays, and cost two
/// `fldcw` more, one on its entry and one on its exit (fpbench -O2: 3,005
/// memory operands against 3,003, 487 B of code against 483).
#[test]
fn test_a_loop_that_rounds_between_conversions_is_not_entered_truncating() {
    let result = switched(&[(0, ".", vec![5]), (5, "acacb", vec![5, 9]), (9, "r", vec![])]);
    for at in [0, 9] {
        let names = &result.iter().find(|(block, _)| *block == at).unwrap().1;
        assert!(!names.contains(&"fldcw".to_owned()), "{result:?}");
    }
}

fn names_in_order(result: &LirBody) -> Vec<String> {
    result.insns().iter().map(|one| name(one).to_owned()).filter(|name| name.starts_with('f')).collect()
}

fn compare_body(
    left_lives: bool,
    right_lives: bool,
) -> LirBody {
    let (a, b, store_a, store_b) = (frame_cell(-4, 4), frame_cell(-8, 4), frame_cell(-12, 4), frame_cell(-16, 4));
    // negated loads: a plain load beside its one reader is fused into the
    // compare as a memory operand
    let mut body = _body(vec![_load(3, &a), fchs(1, 3), _load(4, &b), fchs(2, 4)]);
    let mut insns = body.blocks[0].insns.to_vec();
    insns.push(Arc::new(Insn::new(
        16,
        Some((16, 24)),
        Some(sem(Operation::Compare, "fcom", vec![], vec![fl(1), fl(2)])),
        vec![],
        vec![1, 2],
    )));
    let mut at = 24;
    for (lives, value, cell) in [(left_lives, 1, &store_a), (right_lives, 2, &store_b)] {
        if lives {
            insns.push(Arc::new(Insn::new(at, Some((at, at + 8)), Some(_store(cell, value)), vec![], vec![value])));
            at += 8;
        }
    }
    body.blocks[0].insns = insns.into();
    body
}

/// d_faces ifloor: `x < 0.0` was `fldz; fld st(1); fxch st(1); fcompp` and a
/// `fstp st(0)` after; `fcom` and `fcomp` take any slot, so the live value is
/// compared where it lies.
#[test]
fn test_a_compare_against_a_value_read_again_does_not_copy_it() {
    let one_lives = names_in_order(&with_frame(&compare_body(false, true), &mut Frame::new(-16)));
    assert_eq!(one_lives, ["fld", "fchs", "fld", "fchs", "fxch", "fcomp", "fnstsw", "fstp"], "{one_lives:?}");
    let both_live = names_in_order(&with_frame(&compare_body(true, true), &mut Frame::new(-16)));
    assert_eq!(both_live, ["fld", "fchs", "fld", "fchs", "fxch", "fcom", "fnstsw", "fstp", "fstp"], "{both_live:?}");
}

/// d_faces: both operands dying, the right not beneath the left, was three
/// exchanges around `fcompp` (and `fxch st(2); fxch st(2)` pairs): the left is
/// put on top and the compare names the right's slot, which a dead value then
/// leaves with one `fstp st(i)`.
#[test]
fn test_a_compare_of_two_dying_values_apart_needs_no_rotation() {
    let (a, b, c, out) = (frame_cell(-4, 4), frame_cell(-8, 4), frame_cell(-12, 4), frame_cell(-16, 4));
    let mut body = _body(vec![_load(5, &a), fchs(1, 5), _load(6, &b), fchs(3, 6), _load(7, &c), fchs(2, 7)]);
    let mut insns = body.blocks[0].insns.to_vec();
    // compare value 1 (deepest) with 2 (on top): 3 stays, read after the
    // compare
    insns.push(Arc::new(Insn::new(
        16,
        Some((16, 24)),
        Some(sem(Operation::Compare, "fcom", vec![], vec![fl(1), fl(2)])),
        vec![],
        vec![1, 2],
    )));
    insns.push(Arc::new(Insn::new(24, Some((24, 32)), Some(_store(&out, 3)), vec![], vec![3])));
    body.blocks[0].insns = insns.into();
    let names = names_in_order(&with_frame(&body, &mut Frame::new(-16)));
    let exchanges = names.iter().filter(|name| *name == "fxch").count();
    assert!(exchanges <= 1, "{names:?}");
    assert!(!names.iter().any(|name| name == "fcompp") || exchanges == 0, "{names:?}");
}

/// The left lives and the right dies: `fcom st(i)`, and the right leaves with
/// `fstp st(i)`, no copy of the left.
#[test]
fn test_a_compare_that_keeps_its_left_and_drops_its_right_copies_nothing() {
    let (a, b, out) = (frame_cell(-4, 4), frame_cell(-8, 4), frame_cell(-12, 4));
    let mut body = _body(vec![_load(5, &a), fchs(1, 5), _load(6, &b), fchs(2, 6)]);
    let mut insns = body.blocks[0].insns.to_vec();
    insns.push(Arc::new(Insn::new(
        16,
        Some((16, 24)),
        Some(sem(Operation::Compare, "fcom", vec![], vec![fl(1), fl(2)])),
        vec![],
        vec![1, 2],
    )));
    insns.push(Arc::new(Insn::new(24, Some((24, 32)), Some(_store(&out, 1)), vec![], vec![1])));
    body.blocks[0].insns = insns.into();
    let names = names_in_order(&with_frame(&body, &mut Frame::new(-16)));
    assert_eq!(names.iter().filter(|name| *name == "fld").count(), 2, "{names:?}");
}

/// A float copied to a cell by a plain `mov` (a phi's copy the inline spiller
/// folded to memory) is an `fstp`, and a copy back an `fld`: the stackifier had
/// no rule for a `mov` of an x87 value ("1 operands, 0 results").
#[test]
fn test_a_plain_move_of_a_float_to_and_from_a_cell_is_stored_and_loaded() {
    let (source, spill) = (frame_cell(-8, 8), frame_cell(-16, 8));
    let mut to_cell = sem(Operation::Move, "mov", vec![m(&spill)], vec![fl(1)]);
    to_cell.op = Operation::Move;
    let from_cell = sem(Operation::Move, "mov", vec![fl(2)], vec![m(&spill)]);
    let body = _body(vec![_load(1, &source), to_cell, from_cell, _store(&source, 2)]);
    let result =
        crate::backend::fpstack::converted(&body, &crate::backend::classes::RegisterClasses::of(&llrm_x86_m16::M16))
            .expect("converts");
    let insns = result.insns();
    let names: Vec<&str> = insns.iter().map(|one| name(one)).filter(|name| !name.is_empty()).collect();
    assert!(!names.contains(&"mov"), "{names:?}");
    assert!(names.contains(&"fld") && names.contains(&"fstp"), "{names:?}");
}

/// `Ahead::order` read every instruction of each block ahead, three deep, for
/// every block behind it: a chain of n blocks carrying a float was scanned
/// about 4n times (d_faces: 2 M instructions in 445 calls). Counted, not timed:
/// a block is scanned once.
#[test]
fn test_a_block_ahead_is_scanned_once_however_many_blocks_ask_for_it() {
    let source = frame_cell(-10, 10);
    let cell = frame_cell(-20, 10);
    let original = _body(vec![_load(1, &source), _store(&cell, 1)]);
    let (load, store) = (original.insns()[0].clone(), original.insns()[1].clone());
    let moved = |at: i64| Arc::new(Insn { at, covers: Some((at, at + 8)), ..(*store).clone() });
    let n = 12;
    let mut body = original.clone();
    let mut blocks = vec![block(0, vec![load], vec![16]).into()];
    for k in 1..n {
        blocks.push(block(16 * k, Vec::new(), vec![16 * (k + 1)]).into());
    }
    blocks.push(block(16 * n, vec![moved(16 * n)], vec![]).into());
    body.blocks = blocks;
    let before = crate::backend::fpstack::RENAME_SCANS.with(std::cell::Cell::get);
    crate::backend::fpstack::converted(&body, &crate::backend::classes::RegisterClasses::of(&llrm_x86_m16::M16))
        .expect("a chain of blocks carrying a float");
    let scans = crate::backend::fpstack::RENAME_SCANS.with(std::cell::Cell::get) - before;
    assert!(scans <= n as usize + 1, "{scans} scans for {} blocks", n + 1);
}

/// A double live across a call that returns a double: the allocator reloads it
/// right after the call, ahead of the pseudo-op that takes the call's result,
/// so the result arrives under it. The stackifier refused that, "a call's
/// floating result arrives on a stack that is not empty" (RFP-5 through the DAG
/// path: double_across_a_call_result.c at m16, QCport modules refused under
/// --isel=shadow). The stack was empty at the call, so the result is the bottom
/// entry.
#[test]
fn test_a_reload_before_the_pseudo_that_takes_a_calls_result_leaves_the_result_below_it() {
    let cells = _cells([-4, -8, -12], 8);
    let (spill, out) = (&cells[0], &cells[1]);
    let body = _body(vec![
        _load(1, &cells[2]),
        sem(Operation::FloatStore, "fstp", vec![m(spill)], vec![fl(1)]),
        sem(Operation::Call, "call", vec![], vec![]),
        _load(2, spill),
        sem(Operation::Binary, "add", vec![], vec![]),
        sem(Operation::FloatLoad, "", vec![fl(3)], vec![]),
        _arithmetic("fadd", 4, fl(2), fl(3)),
        _store(out, 4),
    ]);
    let converted =
        crate::backend::fpstack::converted(&body, &crate::backend::classes::RegisterClasses::of(&llrm_x86_m16::M16))
            .expect("the result arrives under the reload");
    let insns = converted.insns();
    let names: Vec<&str> = insns.iter().map(|one| name(one)).filter(|name| !name.is_empty()).collect();
    assert!(names.contains(&"faddp"), "{names:?}");
}

/// The stackifier worked liveness out a second time over the body it had split:
/// a block made on an edge holds nothing, so its rows are those of the block it
/// leads to, and no other row moves (d_faces: two calls of 0.8 M each).
/// Counted, not timed: one set-building pass for the entry rows, one for the
/// exit rows.
#[test]
fn test_a_split_edge_asks_no_second_liveness() {
    let (a, b, out_a, out_b) = (frame_cell(-10, 10), frame_cell(-20, 10), frame_cell(-30, 10), frame_cell(-40, 10));
    let insn_of = |at: i64, what: Semantics| {
        let (defines, uses) = (held_values(&what.dests), held_values(&what.sources));
        Arc::new(Insn::new(at, Some((at, at + 8)), Some(what), defines, uses))
    };
    let mut body = _body(vec![]);
    body.blocks = vec![
        block(0, vec![insn_of(0, _load(1, &a)), insn_of(8, _load(2, &b))], vec![16, 32]).into(),
        block(
            16,
            vec![insn_of(16, _store(&out_a, 1)), insn_of(24, sem(Operation::Move, "mov", vec![], vec![]))],
            vec![32],
        )
        .into(),
        block(32, vec![insn_of(32, _store(&out_b, 2))], vec![]).into(),
    ];
    let before = body.blocks.len();
    crate::backend::liverows::SETS_BUILT.with(|one| one.set(0));
    let converted =
        crate::backend::fpstack::converted(&body, &crate::backend::classes::RegisterClasses::of(&llrm_x86_m16::M16))
            .expect("a fork whose join reads fewer values than the fork holds");
    let built = crate::backend::liverows::SETS_BUILT.with(|one| one.get());
    assert!(converted.blocks.len() > before, "an edge was split");
    assert_eq!(built, 2, "sets built: one liveness pass");
}

#[test]
fn test_float_prep_asks_liveness_of_the_floating_values_only() {
    // `prepared` solved liveness of every value for two lookups of floats:
    // 7 M Ir of d_faces -O2.
    use crate::backend::floatassign::{ROWS_ASKED, prepared};
    let (source, cell) = (frame_cell(-10, 10), frame_cell(-20, 10));
    let integer = |value: u32, from: u32| sem(Operation::Move, "mov", vec![int(value)], vec![int(from)]);
    let body = _body(vec![_load(1, &source), integer(7, 8)]);
    let (load, make) = (body.insns()[0].clone(), body.insns()[1].clone());
    let (store, copy) = {
        let later = _body(vec![_store(&cell, 1), integer(9, 7)]);
        (later.insns()[0].clone(), later.insns()[1].clone())
    };
    let at = |one: &Arc<Insn>, at: i64| Arc::new(Insn { at, covers: Some((at, at + 8)), ..(**one).clone() });
    let mut body = body.clone();
    body.blocks = vec![
        block(0, vec![load, make], vec![16]).into(),
        block(16, vec![at(&store, 16), at(&copy, 24)], vec![]).into(),
    ];
    prepared(&body, None, None, false).unwrap();
    assert_eq!(ROWS_ASKED.with(std::cell::Cell::get), 1);
}

fn int(value: u32) -> Loc {
    Loc::Held(Held { value, width: 4 })
}
