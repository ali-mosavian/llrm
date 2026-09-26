//! Port of `tests/test_floatalloc.py` and the floatalloc-only test in
//! `tests/test_float_constants.py`.

use std::collections::BTreeSet;
use crate::support::hash::HashMap;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use super::{_equivalent_loads, _truncating, allocated};
use crate::backend::cpu;
use crate::backend::floatregions::Raised;
use crate::backend::frame::Frame;
use crate::backend::phielim;
use crate::backend::select;
use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space, St};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::mir::Op;

fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn fl(value: u32) -> Loc {
    Loc::Held(Held { value, width: 10 })
}

fn m(cell: &Mem) -> Loc {
    Loc::Mem(cell.clone())
}

fn st(index: u32) -> Loc {
    Loc::St(St { index })
}

fn frame_cell(disp: i64, width: u32) -> Mem {
    Mem::new(Some(Addr::new(Space::Frame, disp)), width)
}

fn emits(what: &Semantics) -> bool {
    select::emit(what, 0, None, false, false, None).is_some()
}

fn name(one: &Insn) -> &str {
    one.what.as_ref().unwrap().name.as_deref().unwrap_or("")
}

fn what(one: &Insn) -> &Semantics {
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

fn _body(operations: Vec<Semantics>) -> LirBody {
    let insns = operations
        .into_iter()
        .enumerate()
        .map(|(index, what)| {
            let at = index as i64 * 8;
            let (defines, uses) = (held_values(&what.dests), held_values(&what.sources));
            Arc::new(Insn::new(at, Some((at, at + 8)), Some(what), defines, uses))
        })
        .collect();
    LirBody::new("floating", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default())
}

fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
    LirBlock { succ, ..LirBlock::new(at, insns) }
}

/// As the pipeline runs it: phis are copies by now.
fn run(body: &LirBody) -> LirBody {
    allocated(&phielim::eliminated(body).unwrap(), None, None, true, "386").unwrap()
}

fn with_frame(body: &LirBody, frame: &mut Frame) -> LirBody {
    allocated(&phielim::eliminated(body).unwrap(), Some(frame), None, true, "386").unwrap()
}

fn arith(name: &str, d: f64, s: f64) -> f64 {
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

/// Run allocated instructions over `memory`, cells to numbers: the memory after and the stack left.
fn _x87(insns: &[Arc<Insn>], memory: &[(&Mem, f64)]) -> (HashMap<Mem, f64>, Vec<f64>) {
    let (memory, stack, _) = _x87_traced(insns, memory);
    (memory, stack)
}

/// `_x87`, and every store to memory in order.
fn _x87_traced(insns: &[Arc<Insn>], memory: &[(&Mem, f64)]) -> (HashMap<Mem, f64>, Vec<f64>, Vec<(Mem, f64)>) {
    let mut memory: HashMap<Mem, f64> = memory.iter().map(|(cell, value)| ((*cell).clone(), *value)).collect();
    let (mut stack, mut stores): (Vec<f64>, Vec<(Mem, f64)>) = (Vec::new(), Vec::new());
    for one in insns {
        let what = what(one);
        if matches!(what.op, Operation::Nothing | Operation::Jump | Operation::Branch) {
            continue;
        }
        if what.op == Operation::Barrier {
            assert!(stack.is_empty());
            continue;
        }
        assert!(emits(what), "{what:?}");
        let read = |arg: &Loc, stack: &Vec<f64>, memory: &HashMap<Mem, f64>| match arg {
            Loc::St(index) => stack[index.index as usize],
            Loc::Mem(cell) => memory[cell],
            _ => panic!("{arg:?}"),
        };
        let index_of = |arg: &Loc| match arg {
            Loc::St(index) => index.index as usize,
            _ => panic!("{arg:?}"),
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
                Loc::St(slot) => {
                    stack[slot.index as usize] = stack[0];
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

/// The instructions run along `path`, through any block allocation put on an edge of it.
fn _along(result: &LirBody, path: &[i64]) -> Vec<Arc<Insn>> {
    let by_at: HashMap<i64, &LirBlock> = result.blocks.iter().map(|block| (block.at, block)).collect();
    let mut insns = Vec::new();
    for (index, at) in path.iter().enumerate() {
        insns.extend(by_at[at].insns.iter().cloned());
        if let Some(next) = path.get(index + 1).filter(|next| !by_at[at].succ.contains(next)) {
            let edge = by_at[at].succ.iter().find(|edge| by_at[*edge].succ == vec![*next]).expect("a block on the edge");
            insns.extend(by_at[edge].insns.iter().cloned());
        }
    }
    insns
}

fn _cells(displacements: impl IntoIterator<Item = i64>, width: u32) -> Vec<Mem> {
    displacements.into_iter().map(|disp| frame_cell(disp, width)).collect()
}

fn _load(value: u32, cell: &Mem) -> Semantics {
    sem(Operation::FloatLoad, "fld", vec![fl(value)], vec![m(cell)])
}

fn _store(cell: &Mem, value: u32) -> Semantics {
    sem(Operation::FloatStore, "fstp", vec![m(cell)], vec![fl(value)])
}

fn _arithmetic(name: &str, result: u32, left: Loc, right: Loc) -> Semantics {
    sem(Operation::FloatArith, name, vec![fl(result)], vec![left, right])
}

fn fchs(result: u32, source: u32) -> Semantics {
    sem(Operation::FloatUnary, "fchs", vec![fl(result)], vec![fl(source)])
}

fn reads(result: &LirBody, cell: &Mem) -> usize {
    result.insns().iter().filter(|one| what(one).sources.contains(&m(cell))).count()
}

#[test]
fn test_truncation_saves_the_control_word_once_per_body() {
    // Every `(int)f` saved the control word and built the truncating one again.
    let insn = |at: i64, what: Semantics| Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]));
    let store = |at: i64, disp: i64| {
        let cell = Mem { through: Register::BP, offset: 0, disp_width: 2, ..frame_cell(disp, 2) };
        insn(at, sem(Operation::FloatStore, "fisttp", vec![m(&cell)], vec![st(0)]))
    };
    let blocks = vec![
        block(0, vec![insn(0, sem(Operation::Nothing, "", vec![], vec![]))], vec![5]),
        block(5, vec![store(5, -2), store(6, -4)], vec![]),
    ];
    let mut frame = Frame::new(-8);
    let result =
        _truncating(&LirBody::new("t", 0, blocks, IndexMap::default(), IndexMap::default()), Some(&mut frame)).unwrap();
    let names: Vec<(i64, Vec<&str>)> = result
        .blocks
        .iter()
        .map(|block| (block.at, block.insns.iter().map(|one| name(one)).filter(|name| !name.is_empty()).collect()))
        .collect();
    assert_eq!(
        names,
        vec![
            (0, vec!["fnstcw", "mov", "or", "mov"]),
            (5, vec!["fldcw", "fistp", "fldcw", "fldcw", "fistp", "fldcw"]),
        ]
    );
}

#[test]
fn test_a_load_read_by_several_arithmetics_is_each_ones_memory_operand() {
    // NBODYS held `falloff` for two multiplies where BC wrote `fmul [m]` twice.
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
    let insns = result.insns();
    let using: Vec<(&str, Vec<Loc>)> = insns
        .iter()
        .filter(|one| what(one).sources.contains(&m(falloff)))
        .map(|one| (name(one), what(one).sources.clone()))
        .collect();
    assert_eq!(using, vec![("fmul", vec![st(0), m(falloff)]); 2]);
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
                through: Register::SI,
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
fn test_a_first_read_moves_past_a_trapping_instruction_only_for_a_quiet_cell() {
    // `fld m32` raises on a signalling NaN; read after a division, that exception would come second.
    for proven in [false, true] {
        let cells = _cells([-4, -8, -12, -16, -20, -24, -28], 4);
        let (w, x, y, c, z, out, other) = (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4], &cells[5], &cells[6]);
        let (h, a, b, f, q, g, r) = (1, 2, 3, 4, 5, 6, 7);
        let mut operations = if proven { vec![_load(h, w), _store(c, h)] } else { vec![] };
        operations.extend([
            _load(a, x),
            _load(b, y),
            _load(f, c),
            _arithmetic("fdiv", q, fl(a), fl(b)),
            _store(out, q),
            _load(g, z),
            _arithmetic("fmul", r, fl(g), fl(f)),
            _store(other, r),
        ]);
        let result = run(&_body(operations));
        let insns = result.insns();
        let readers: Vec<usize> =
            insns.iter().enumerate().filter(|(_, one)| what(one).sources.contains(&m(c))).map(|(index, _)| index).collect();
        let division = insns.iter().position(|one| name(one).starts_with("fdiv")).unwrap();
        assert!(readers.len() == 1 && (readers[0] > division) == proven, "{proven}");
        let (memory, stack) = _x87(&insns, &[(w, 0.25), (x, 6.0), (y, 3.0), (c, 0.5), (z, 4.0)]);
        assert_eq!((memory[out], memory[other], stack), (2.0, if proven { 1.0 } else { 2.0 }, vec![]));
    }
}

#[test]
fn test_arithmetic_overwrites_the_operand_that_dies() {
    // Neither operand on top, the second dying: the first was exchanged up and duplicated where one exchange does.
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
    assert!(!insns.iter().any(|one| name(one) == "fld" && matches!(what(one).sources[0], Loc::St(_))));
    let (memory, stack) = _x87(&insns, &[(x, 8.0), (y, 2.0), (z, 1.0)]);
    assert_eq!((memory[first], memory[second], memory[third], stack), (-6.0, -1.0, -8.0, vec![]));
}

#[test]
fn test_a_float_live_across_a_call_is_read_again_from_its_cell() {
    // A value loaded before a call and stored after it refused the whole object.
    for boundary in [Operation::Call, Operation::Barrier] {
        let (source, target) = (frame_cell(-4, 4), frame_cell(-8, 4));
        let body = _body(vec![
            _load(1, &source),
            sem(boundary, if boundary == Operation::Call { "call" } else { "" }, vec![], vec![]),
            _store(&target, 1),
        ]);
        let result = with_frame(&body, &mut Frame::new(-8));
        let shape: Vec<(String, Vec<Loc>, Vec<Loc>)> = result
            .insns()
            .iter()
            .filter(|one| what(one).op != Operation::Nothing)
            .map(|one| (name(one).to_owned(), what(one).dests.clone(), what(one).sources.clone()))
            .collect();
        // The source cell is unchanged across it: read again, with nothing spilled.
        assert_eq!(
            shape,
            vec![
                ((if boundary == Operation::Call { "call" } else { "" }).to_owned(), vec![], vec![]),
                ("fld".to_owned(), vec![st(0)], vec![m(&source)]),
                ("fstp".to_owned(), vec![m(&target)], vec![st(0)]),
            ]
        );
    }
}

#[test]
fn test_region_value_reuses_one_reload_until_an_unknown_effect() {
    // A shared floating result crossing a fork reloaded its owned slot for every store.
    // It now stays on the stack across the edge; only a call sends it to memory.
    for boundary in [None, Some(Operation::Call), Some(Operation::Barrier)] {
        let cell = frame_cell(-4, 4);
        let mut operations = vec![_load(1, &cell), _store(&cell, 1)];
        if let Some(boundary) = boundary {
            operations.push(sem(boundary, if boundary == Operation::Call { "call" } else { "" }, vec![], vec![]));
        }
        operations.push(_store(&cell, 1));
        let mut body = _body(operations);
        let insns = body.insns();
        body.blocks =
            vec![block(0, insns[..1].to_vec(), vec![8, 80]), block(8, insns[1..].to_vec(), vec![]), block(80, vec![], vec![])];
        let result = with_frame(&body, &mut Frame::new(-8));
        let consumer = result.blocks.iter().find(|block| block.at == 8).unwrap();
        let reloads = consumer
            .insns
            .iter()
            .filter(|one| {
                what(one).op == Operation::FloatLoad
                    && what(one).sources.iter().any(|arg| matches!(arg, Loc::Mem(arg) if arg.width == 8))
            })
            .count();
        assert_eq!(reloads, usize::from(boundary.is_some()));
        let stores: Vec<&Semantics> = consumer
            .insns
            .iter()
            .map(|one| what(one))
            .filter(|what| what.op == Operation::FloatStore && what.dests == vec![m(&cell)])
            .collect();
        assert_eq!(stores.iter().map(|one| one.name.as_deref().unwrap()).collect::<Vec<_>>(), vec!["fst", "fstp"]);
        assert!(stores.iter().all(|one| one.sources == vec![st(0)]));
        if boundary.is_none() {
            assert!(consumer.insns.iter().all(|one| emits(what(one))));
        }
    }
}

#[test]
fn test_a_load_read_once_by_the_next_arithmetic_is_its_memory_operand() {
    // `fld [x]` then a popping multiply spent an instruction and a stack slot `fmul [x]` does not.
    for (loaded, op, loaded_first, fused) in
        [("fld", "fmul", false, "fmul"), ("fld", "fsub", true, "fsubr"), ("fild", "fdiv", false, "fidiv")]
    {
        let (home, cell) = (frame_cell(-4, 4), frame_cell(-8, 4));
        let body = _body(vec![
            _load(1, &home),
            sem(Operation::FloatLoad, loaded, vec![fl(2)], vec![m(&cell)]),
            sem(Operation::FloatArith, op, vec![fl(3)], if loaded_first { vec![fl(2), fl(1)] } else { vec![fl(1), fl(2)] }),
            _store(&home, 3),
        ]);
        let result = run(&body);
        let emitted: Vec<(String, Vec<Loc>)> = result
            .insns()
            .iter()
            .filter(|one| what(one).op != Operation::Nothing)
            .map(|one| (name(one).to_owned(), what(one).sources.clone()))
            .collect();
        assert_eq!(
            emitted,
            vec![
                ("fld".to_owned(), vec![m(&home)]),
                (fused.to_owned(), vec![st(0), m(&cell)]),
                ("fstp".to_owned(), vec![st(0)]),
            ]
        );
        assert!(result.insns().iter().all(|one| emits(what(one))));
    }
}

#[test]
fn test_a_load_folded_into_x87_memory_arithmetic_retains_its_source_bytes() {
    // Qmove and qbsp fresh emission refused 32 and 9 unowned bytes.
    let cells = _cells([-4, -8], 4);
    let (home, cell) = (&cells[0], &cells[1]);
    let body = _body(vec![_load(1, home), _load(2, cell), _arithmetic("fmul", 3, fl(1), fl(2)), _store(home, 3)]);
    let result = run(&body);
    let covered: BTreeSet<i64> =
        result.insns().iter().flat_map(|one| { let (start, end) = one.covers.unwrap(); start..end }).collect();
    assert_eq!(covered, (0..32).collect());
    assert_eq!(
        result.insns().iter().filter(|one| what(one).op != Operation::Nothing).map(|one| name(one)).collect::<Vec<_>>(),
        ["fld", "fmul", "fstp"]
    );
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
fn test_runtime_stack_integer_result_uses_memory_without_named_float_values() {
    // SYS_INIT_TABLES at 08fa refused FISTP EBX after POW4 returned in physical ST0.
    for width in [2, 4] {
        let result = Loc::Held(Held { value: 2, width });
        let cell = frame_cell(-8, width);
        let body = _body(vec![
            sem(Operation::FloatStore, "fistp", vec![result.clone()], vec![st(0)]),
            sem(Operation::Move, "mov", vec![m(&cell)], vec![result.clone()]),
        ]);
        let allocated = with_frame(&body, &mut Frame::new(-8));
        let insns = allocated.insns();
        assert_eq!(insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["wait", "fistp", "wait", "mov", "mov"]);
        let (store, load) = (what(&insns[1]), what(&insns[3]));
        assert!(matches!(store.dests[0], Loc::Mem(_)));
        assert_eq!(store.dests, load.sources);
        assert!(matches!(&store.dests[0], Loc::Mem(cell) if cell.width == width));
        assert_eq!(load.dests, vec![result]);
        assert!(emits(store));
    }
}

#[test]
fn test_integer_result_waits_before_reading_owned_conversion_storage() {
    // B$FIST/B$FIS2 wait on both sides of conversion before returning the integer.
    for width in [2, 4] {
        let result = Loc::Held(Held { value: 2, width });
        let cell = frame_cell(-8, 8);
        let body = _body(vec![
            _load(1, &cell),
            sem(Operation::FloatStore, "fistp", vec![result.clone()], vec![fl(1)]),
            sem(Operation::Move, "mov", vec![m(&cell)], vec![result.clone()]),
        ]);
        let allocated = with_frame(&body, &mut Frame::new(-8));
        let insns = allocated.insns();
        assert_eq!(insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fld", "wait", "fistp", "wait", "mov", "mov"]);
        let (store, load) = (what(&insns[2]), what(&insns[4]));
        assert_eq!(store.dests, load.sources);
        let Loc::Mem(stored) = &store.dests[0] else { panic!() };
        assert_eq!(stored.width, width);
        assert!(stored.addr.unwrap().disp < -8);
        assert_eq!(load.dests, vec![result]);
    }
}

#[test]
fn test_unused_integer_conversion_keeps_checkpoints_without_materializing_result() {
    // Constant print arguments can leave an unused conversion result; both waits must survive.
    for width in [2, 4] {
        let cell = frame_cell(-8, 8);
        let body = _body(vec![
            _load(1, &cell),
            sem(Operation::FloatStore, "fistp", vec![Loc::Held(Held { value: 2, width })], vec![fl(1)]),
        ]);
        let allocated = with_frame(&body, &mut Frame::new(-8));
        let insns = allocated.insns();
        assert_eq!(insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fld", "wait", "fistp", "wait"]);
        assert!(matches!(&what(&insns[2]).dests[0], Loc::Mem(cell) if cell.width == width));
    }
}

#[test]
fn test_conversion_result_is_kept_for_non_operand_readers() {
    // An invisible reader must not lose the integer returned by B$FIST.
    // A phi is a copy by the time floats are allocated.
    for reader in ["opaque", "barrier", "pinned", "uses"] {
        let result = Loc::Held(Held { value: 2, width: 4 });
        let cell = frame_cell(-8, 8);
        let mut body = _body(vec![_load(1, &cell), sem(Operation::FloatStore, "fistp", vec![result.clone()], vec![fl(1)])]);
        let first = body.blocks[0].clone();
        match reader {
            "pinned" => body.pins = IndexMap::from_iter([(2, Register::None)]),
            _ => {
                let what = match reader {
                    "opaque" => None,
                    "barrier" => Some(sem(Operation::Barrier, "", vec![], vec![])),
                    _ => Some(sem(Operation::Nothing, "", vec![], vec![])),
                };
                let extra = Insn::new(24, Some((24, 24)), what, vec![], if reader == "uses" { vec![2] } else { vec![] });
                let mut extended = first;
                extended.insns.push(Arc::new(extra));
                body.blocks = vec![extended];
            }
        }
        let allocated = with_frame(&body, &mut Frame::new(-8));
        assert!(
            allocated.insns().iter().any(|one| one
                .what
                .as_ref()
                .is_some_and(|what| what.name.as_deref() == Some("mov") && what.dests == vec![result.clone()])),
            "{reader}"
        );
    }
}

#[test]
fn test_a_pinned_conversion_result_survives_lowering_into_floatalloc() {
    // Lower keyed `pins` by `mir.Value` and floatalloc read ids, so a result
    // pinned to EAX with no other reader was left in the frame, never loaded.
    use crate::backend::lower;
    use crate::model::floating::{Format, Precision, Rounding, Semantics as Floating};
    use crate::model::mir::{AllocationHints, Arg, Cell, Held as Named, Kind, MemRef, MirBlock, MirBody, OpCode, Value};

    let cell = MemRef { space: Some(Space::Frame), ..MemRef::new(Some(Addr::new(Space::Frame, -10)), 10) };
    let (real, integer) = (Value::new(1, 0x10), Value::new(2, 0x20));
    let mut load = Op::new(0x10, OpCode::Operation(Operation::FloatLoad), "fld", vec![real], vec![]);
    load.kind = Kind::Fload;
    load.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    load.results = vec![Arg::Held(Named { value: real, width: 10 })];
    load.loads = vec![cell];
    load.floating = Some(Floating::new([Format::Extended80], Format::Extended80, Precision::Exact, Rounding::None));
    let mut store = Op::new(0x20, OpCode::Operation(Operation::FloatStore), "fistp", vec![integer], vec![real]);
    store.kind = Kind::Fstore;
    store.args = vec![Arg::Held(Named { value: real, width: 10 })];
    store.results = vec![Arg::Held(Named { value: integer, width: 4 })];
    store.floating = Some(Floating::new([Format::Extended80], Format::Signed32, Precision::Destination, Rounding::Dynamic));
    store.source = Some(200);
    let body = MirBody::new(0x10, vec![MirBlock::new(0x10, vec![], vec![load, store], vec![])]);
    let mut hints = AllocationHints::new();
    hints.pins.insert((200, 0), Register::EAX);
    let options = lower::Lowered { hints: Some(&hints), ..Default::default() };
    let low = lower::lowered("pinned", &body, Some(&IndexMap::default()), BTreeSet::new(), Some(&IndexMap::default()), "386", options)
        .unwrap();
    let allocated = allocated(&low, Some(&mut Frame::new(-10)), None, false, "386").unwrap();
    let result = Loc::Held(Held { value: integer.id, width: 4 });
    assert!(allocated.insns().iter().any(|one| one
        .what
        .as_ref()
        .is_some_and(|what| what.name.as_deref() == Some("mov") && what.dests == vec![result.clone()])));
}

#[test]
fn test_ninth_float_uses_an_owned_spill() {
    // Nine live FP values previously refused allocation. Machine semantics spill as a double.
    let sources = _cells((-40..-4).step_by(4), 4);
    let answers = _cells((-80..-44).step_by(4), 4);
    let mut operations = Vec::new();
    for (index, cell) in sources.iter().enumerate() {
        let (load, value) = (index as u32 + 1, index as u32 + 11);
        operations.extend([_load(load, cell), fchs(value, load)]);
    }
    for (index, cell) in answers.iter().enumerate() {
        operations.push(_store(cell, index as u32 + 11));
    }
    let body = _body(operations);
    let mut slots = Frame::new(-80);
    let integer_scratch = slots.cell(1_i64, 2).unwrap();
    let allocated = with_frame(&body, &mut slots);
    let insns = allocated.insns();
    let spills: Vec<&Arc<Insn>> = insns
        .iter()
        .filter(|one| name(one) == "fstp" && matches!(&what(one).dests[0], Loc::Mem(cell) if cell.width == 8))
        .collect();
    assert!(spills.len() == 1 && slots.size() >= 8);
    assert!(spills.iter().all(|one| {
        matches!(&what(one).dests[0], Loc::Mem(cell) if cell.addr != integer_scratch.addr)
            && one.covers.unwrap().0 == one.covers.unwrap().1
    }));
    let memory: Vec<(&Mem, f64)> = sources.iter().enumerate().map(|(index, cell)| (cell, index as f64 + 1.0)).collect();
    let (memory, stack) = _x87(&insns, &memory);
    assert_eq!(answers.iter().map(|cell| memory[cell]).collect::<Vec<_>>(), (1..10).map(|index| -f64::from(index)).collect::<Vec<_>>());
    assert!(stack.is_empty());
}

#[test]
fn test_float_survives_fork_join_and_loop_without_rereading_source() {
    // A dominating extended value was refused at forks, joins and loop boundaries.
    // It now stays on the stack through all of them, with no memory between.
    for path in [vec![0, 16, 48], vec![0, 32, 48], vec![0, 16, 16, 48]] {
        let cell = frame_cell(-20, 10);
        let source = frame_cell(-10, 10);
        let original = _body(vec![_load(1, &source), _store(&cell, 1)]);
        let (load, store) = (original.insns()[0].clone(), original.insns()[1].clone());
        let moved = |at: i64| Arc::new(Insn { at, covers: Some((at, at + 8)), ..(*store).clone() });
        let mut body = original.clone();
        body.blocks = vec![
            block(48, vec![moved(48)], vec![]),
            block(0, vec![load], vec![16, 32]),
            block(16, vec![moved(16)], vec![16, 48]),
            block(32, vec![moved(32)], vec![48]),
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
        // Python used Fraction(1) + 2**-63: any value only moved, never computed.
        let precise = 1.25;
        let (_, stack, stores) = _x87_traced(&insns, &[(&source, precise)]);
        assert_eq!(stores, vec![(cell.clone(), precise); path.len() - 1]);
        assert!(stack.is_empty());
        assert_eq!(slots.size(), before);
    }
}

#[test]
fn test_floating_bridge_never_reads_an_unestablished_slot() {
    // Cross-block allocation must not turn a missing definition into a frame read.
    for defect in ["entry", "bypass"] {
        let cell = frame_cell(-10, 10);
        let body = _body(vec![_load(1, &cell), _store(&cell, 1)]);
        let (load, store) = (body.insns()[0].clone(), body.insns()[1].clone());
        let mut blocks = vec![block(0, vec![load], vec![16, 32]), block(16, vec![store], vec![]), block(32, vec![], vec![16])];
        let mut body = body;
        if defect == "entry" {
            body.entry = 16;
        } else {
            body.entry = 48;
            blocks.push(block(48, vec![], vec![0, 16]));
        }
        body.blocks = blocks;
        let result = allocated(&body, Some(&mut Frame::new(-10)), None, true, "386");
        assert!(matches!(result, Err(Raised::Unlowered(_))), "{defect}: {result:?}");
    }
}

#[test]
fn test_floating_loop_phis_swap_in_parallel_on_the_critical_backedge() {
    // Floating loop phis were refused; serial slot copies would turn (1,2) into (2,2).
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
            block(0, insns[..2].to_vec(), vec![16]),
            LirBlock {
                phis: vec![
                    Phi { result: 3, incoming: vec![(0, 1), (16, 4)] },
                    Phi { result: 4, incoming: vec![(0, 2), (16, 3)] },
                ],
                ..block(16, insns[2..].to_vec(), vec![16, 48])
            },
            block(48, vec![], vec![]),
        ];
        let result = with_frame(&body, &mut Frame::new(-30));
        let insns = _along(&result, &[0, 16, 16, 48]);
        assert!(insns.iter().all(|one| matches!(what(one).op, Operation::Branch | Operation::Jump) || emits(what(one))));
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
fn test_shared_float_crosses_only_a_unique_straight_line_edge() {
    // A shared sum was refused at a block edge despite one unchanged stack path.
    // Only a value no path defines is refused.
    for boundary in ["linear", "reversed", "separated", "fork", "join", "entry"] {
        let cell = frame_cell(-4, 4);
        let mut body = _body(vec![
            _load(1, &cell),
            _arithmetic("fmul", 2, fl(1), m(&cell)),
            _store(&cell, 2),
            _arithmetic("fdiv", 3, fl(1), m(&cell)),
            _store(&cell, 3),
        ]);
        let insns = body.insns();
        let first = block(0, insns[..3].to_vec(), if boundary == "fork" { vec![24, 80] } else { vec![24] });
        let second = block(24, insns[3..].to_vec(), vec![]);
        let mut blocks = vec![first.clone(), second.clone()];
        if boundary == "fork" || boundary == "join" {
            blocks.push(block(80, vec![], if boundary == "join" { vec![24] } else { vec![] }));
        }
        if boundary == "reversed" {
            blocks.reverse();
        }
        if boundary == "separated" {
            blocks = vec![first, block(80, vec![], vec![]), second];
        }
        body.entry = if boundary == "entry" { 24 } else { 0 };
        body.blocks = blocks;
        if boundary == "entry" {
            assert!(matches!(allocated(&body, None, None, true, "386"), Err(Raised::Unlowered(_))), "{boundary}");
            continue;
        }
        let allocated = run(&body);
        assert_eq!(
            allocated.blocks.iter().map(|block| block.at).collect::<Vec<_>>(),
            body.blocks.iter().map(|block| block.at).collect::<Vec<_>>()
        );
        let by_at: HashMap<i64, &LirBlock> = allocated.blocks.iter().map(|block| (block.at, block)).collect();
        assert_eq!(by_at[&0].insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fld", "fld", "fmul", "fstp"]);
        assert_eq!(by_at[&24].insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fdiv", "fstp"]);
        assert_eq!(what(&by_at[&24].insns[0]).sources, vec![st(0), m(&cell)]);
        assert!(allocated.insns().iter().all(|one| emits(what(one))));
        if boundary == "fork" {
            // The other way out does not read it, and pops it.
            assert_eq!(by_at[&80].insns.iter().map(|one| name(one)).collect::<Vec<_>>(), ["fstp"]);
        }
    }
}

#[test]
fn test_integer_conversion_materializes_a_frame_operand() {
    // FPCALC's computed integer must reach FILD through an owned, correctly sized slot.
    for width in [2, 4] {
        let integer = Loc::Held(Held { value: 1, width });
        let output = frame_cell(-8, 8);
        let body = _body(vec![sem(Operation::FloatLoad, "fild", vec![fl(2)], vec![integer.clone()]), _store(&output, 2)]);
        let mut slots = Frame::new(-8);
        let result = with_frame(&body, &mut slots);
        let emitted: Vec<Arc<Insn>> = result.insns().into_iter().filter(|one| what(one).op != Operation::Nothing).collect();
        let [store, load, _] = emitted.as_slice() else { panic!("{emitted:?}") };
        assert_eq!(slots.size(), i64::from(width));
        assert_eq!(what(store).sources, vec![integer]);
        assert_eq!(what(store).dests, what(load).sources);
        assert_eq!(what(store).dests, vec![Loc::Mem(slots.cell(2, i64::from(width)).unwrap())]);
        assert!(store.uses == vec![1] && load.uses.is_empty());
        assert_eq!(store.covers, Some((0, 0)));
    }
}

#[test]
fn test_integer_conversion_into_physical_x87_stack_materializes_memory() {
    // Qrender camera 0335 emitted impossible FILD BX after integer promotion.
    for width in [2, 4] {
        let integer = Loc::Held(Held { value: 1, width });
        let body = _body(vec![sem(Operation::FloatLoad, "fild", vec![st(0)], vec![integer.clone()])]);
        let result = with_frame(&body, &mut Frame::new(-8));
        let insns = result.insns();
        assert_eq!(insns.len(), 2);
        let (store, load) = (what(&insns[0]), what(&insns[1]));
        assert_eq!(store.sources, vec![integer]);
        assert!(matches!(load.sources[0], Loc::Mem(_)));
        assert_eq!(store.dests, load.sources);
        assert_eq!(load.dests, vec![st(0)]);
        assert!(emits(load));
    }
}

#[test]
fn test_buried_float_operand_is_exchanged_not_duplicated() {
    // A second live value must not prevent using the first, or reverse subtraction.
    for operation in ["fsub", "fchs", "fstp", "fsubp"] {
        let cells = _cells([-4, -8, -12, -16], 4);
        let (first_cell, second_cell, answer, kept) = (&cells[0], &cells[1], &cells[2], &cells[3]);
        let (first_load, second_load, first, second, result) = (1, 2, 3, 4, 5);
        let mut operations = vec![_load(first_load, first_cell), fchs(first, first_load), _load(second_load, second_cell), fchs(second, second_load)];
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
                    || !what.sources.is_empty() && matches!(what.sources[0], Loc::St(_)) && what.op == Operation::FloatLoad
            })
            .collect();
        // Both operands of the popping subtraction die, so the result overwrites one in place.
        assert_eq!(
            moves.iter().map(|one| name(one)).collect::<Vec<_>>(),
            if operation == "fsubp" { vec![] } else { vec!["fxch"] }
        );
        assert!(moves.iter().all(|one| one.covers.unwrap().0 == one.covers.unwrap().1 && one.uses.is_empty() && one.defines.is_empty()));
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
    // Forwarded floating memory operands must die without leaking a stack slot or reversing division.
    for (op, popping, expected) in [("fadd", "faddp", 9.0), ("fmul", "fmulp", 14.0), ("fsub", "fsubrp", 5.0), ("fdiv", "fdivrp", 3.5)] {
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
                        let Loc::St(index) = what.sources[1] else { panic!() };
                        stack.swap(0, index.index as usize);
                    }
                    "fstp" => stored.push(stack.remove(0)),
                    "" => continue,
                    other => {
                        let want = if top == "left" { popping.to_owned() } else { popping.replace("rp", "p") };
                        assert_eq!(other, want);
                        let Loc::St(index) = what.sources[0] else { panic!() };
                        let index = index.index as usize;
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
fn test_missing_float_is_not_created_by_an_exchange() {
    let body = _body(vec![fchs(2, 1)]);
    let Err(Raised::Unlowered(error)) = allocated(&body, None, None, true, "386") else { panic!() };
    assert!(error.0.contains("unavailable"));
}

#[test]
fn test_shared_producer_is_kept_across_two_arithmetic_consumers() {
    // FPCSE's shared sum needs to survive multiplication before division reads it.
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
    assert!(duplicate.covers == Some((8, 8)) && duplicate.op.is_none());
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
    assert!(multiply.sources.iter().all(|arg| matches!(arg, Loc::St(_))));
    let (memory, stack) = _x87(&insns, &[(source, 7.0), (home, 3.0)]);
    assert!(memory[out] == -21.0 && stack.is_empty());
}

#[test]
fn test_x87_memory_operand_matches_each_public_cpu_cost() {
    // Each public CPU's emitted form agrees with its own audited cost table.
    for profile in cpu::names() {
        let (body, _) = _memory_operand_body();
        let target = cpu::profile(profile).unwrap();
        let result = allocated(&body, None, None, true, target).unwrap();
        let insns = result.insns();
        let multiply = what(insns.iter().find(|one| name(one).starts_with("fmul")).unwrap());
        let folded = multiply.sources.iter().any(|arg| matches!(arg, Loc::Mem(_)));
        let cost = |form: &str| target.cost(form).unwrap();
        assert_eq!(folded, cost("x87_mul_m") <= cost("x87_load") + cost("x87_mul"), "{profile}");
    }
}

#[test]
fn test_profitable_multiuse_float_home_is_retained_for_the_selected_cpu() {
    // C nbody reread each rounded dx/dy temporary for every force term.
    for profile in cpu::names() {
        let cells = _cells([-4, -8, -12, -16, -20, -24], 4);
        let (home, left_cell, right_cell, square_out, left_out, right_out) =
            (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4], &cells[5]);
        let (shared, left, right, square, left_product, right_product) = (1, 2, 3, 4, 5, 6);
        let body = _body(vec![
            _load(shared, home),
            _arithmetic("fmul", square, fl(shared), fl(shared)),
            _store(square_out, square),
            _load(left, left_cell),
            _arithmetic("fmul", left_product, fl(shared), fl(left)),
            _store(left_out, left_product),
            _load(right, right_cell),
            _arithmetic("fmul", right_product, fl(shared), fl(right)),
            _store(right_out, right_product),
        ]);
        let target = cpu::profile(profile).unwrap();
        let result = allocated(&body, None, None, true, target).unwrap();
        let cost = |form: &str| target.cost(form).unwrap();
        let (load, multiply, memory_multiply) = (cost("x87_load"), cost("x87_mul"), cost("x87_mul_m"));
        let retain_cost = 2 * load + 3 * multiply;
        let home_cost = load + multiply + 2 * memory_multiply.min(load + multiply);
        let expected_reads = if retain_cost < home_cost { 1 } else { 3 };

        assert_eq!(reads(&result, home), expected_reads, "{profile}");
        let (memory, stack) = _x87(&result.insns(), &[(home, 7.0), (left_cell, 2.0), (right_cell, 3.0)]);
        assert_eq!((memory[square_out], memory[left_out], memory[right_out], stack), (49.0, 14.0, 21.0, vec![]));
    }
}

#[test]
fn test_independent_x87_regions_select_their_own_complete_candidate() {
    // One bad stack region made the old whole-body choice discard a cheaper sibling.
    let cells = _cells([-4, -8, -12, -16, -20, -24], 4);
    let (home, left_cell, right_cell, square_out, left_out, right_out) =
        (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4], &cells[5]);
    let (shared, left, right, square, left_product, right_product) = (1, 2, 3, 4, 5, 6);
    let mut operations = vec![
        _load(shared, home),
        _arithmetic("fmul", square, fl(shared), fl(shared)),
        _store(square_out, square),
        _load(left, left_cell),
        _arithmetic("fmul", left_product, fl(shared), fl(left)),
        _store(left_out, left_product),
        _load(right, right_cell),
        _arithmetic("fmul", right_product, fl(shared), fl(right)),
        _store(right_out, right_product),
        sem(Operation::Barrier, "wait", vec![], vec![]),
    ];
    let others = _cells([-28, -32, -36, -40, -44], 4);
    let (other, scale_cell, first_acc, second_acc, other_square) = (&others[0], &others[1], &others[2], &others[3], &others[4]);
    let (value, squared, scale, product, old, added, scale_again, product_again, old_again, subtracted) =
        (10, 11, 12, 13, 14, 15, 16, 17, 18, 19);
    operations.extend([
        _load(value, other),
        _arithmetic("fmul", squared, fl(value), fl(value)),
        _store(other_square, squared),
        _load(old, first_acc),
        _load(scale, scale_cell),
        _arithmetic("fmul", product, fl(value), fl(scale)),
        _arithmetic("fadd", added, fl(old), fl(product)),
        _store(first_acc, added),
        _load(old_again, second_acc),
        _load(scale_again, scale_cell),
        _arithmetic("fmul", product_again, fl(value), fl(scale_again)),
        _arithmetic("fsub", subtracted, fl(old_again), fl(product_again)),
        _store(second_acc, subtracted),
    ]);

    let result = run(&_body(operations));

    assert_eq!(reads(&result, home), 1);
    assert_eq!(reads(&result, other), 3);
    assert!(!result.insns().iter().any(|one| name(one) == "fxch"));
    let (memory, stack) = _x87(
        &result.insns(),
        &[(home, 7.0), (left_cell, 2.0), (right_cell, 3.0), (other, 5.0), (scale_cell, 2.0), (first_acc, 10.0), (second_acc, 20.0)],
    );
    assert_eq!(
        (memory[square_out], memory[left_out], memory[right_out], memory[other_square], memory[first_acc], memory[second_acc]),
        (49.0, 14.0, 21.0, 25.0, 20.0, 10.0)
    );
    assert!(stack.is_empty());
}

#[test]
fn test_repeated_stable_float_cell_load_is_kept_across_consumers() {
    // C nbody reloaded one rounded frame temporary for every force term.
    let cells = _cells([-4, -8, -12, -16], 4);
    let (cell, factor, first_out, second_out) = (&cells[0], &cells[1], &cells[2], &cells[3]);
    let body = _body(vec![
        _load(1, cell),
        _arithmetic("fmul", 5, fl(1), m(factor)),
        _store(first_out, 5),
        _load(3, cell),
        _arithmetic("fmul", 6, fl(3), m(factor)),
        _store(second_out, 6),
    ]);

    let result = run(&body);

    let insns = result.insns();
    assert_eq!(insns.iter().filter(|one| name(one) == "fld" && what(one).sources == vec![m(cell)]).count(), 1);
    let (memory, stack) = _x87(&insns, &[(cell, 7.0), (factor, 3.0)]);
    assert!(memory[first_out] == 21.0 && memory[second_out] == 21.0 && stack.is_empty());
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
    let mut volatile = Op::new(8, None, "", vec![], vec![]);
    volatile.volatile = true;
    insns[1] = Arc::new(Insn { op: Some(Arc::new(volatile)), ..(*insns[1]).clone() });

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
        assert_eq!(expected.iter().map(|(cell, _)| memory[*cell]).collect::<Vec<_>>(), expected.iter().map(|(_, value)| *value).collect::<Vec<_>>(), "{live}");
        assert!(stack.is_empty());
    }
}

// tests/test_float_constants.py
#[test]
fn test_exact_float_constants_need_no_frame() {
    for (value, encoded) in [(0, [0xd9, 0xee]), (1, [0xd9, 0xe8])] {
        for width in [2, 4] {
            let constant = sem(Operation::FloatLoad, "fild", vec![st(0)], vec![Loc::Imm(Imm { value, width, address: None })]);
            let instruction = Arc::new(Insn::new(0, Some((0, 2)), Some(constant), vec![], vec![]));
            let body = LirBody::new("constant", 0, vec![LirBlock::new(0, vec![instruction])], IndexMap::default(), IndexMap::default());
            let allocated = run(&body);
            let insns = allocated.insns();
            assert_eq!(insns.len(), 1);
            let emitted = select::emit(what(&insns[0]), 0, None, false, false, None).unwrap();
            assert_eq!(emitted.code, encoded);
        }
    }
}

/// Any other integer constant is a readonly datum in its narrowest exact
/// format, as GCC's constant pool: QB's `x * 320` stored 320 to a frame
/// temporary at every use and read it back with a 16-cycle `fild`.
#[test]
fn test_an_integer_constant_loads_from_the_pool() {
    use crate::backend::constpool::Pool;
    // A 16-bit immediate is its bits: 0xffff is -1.
    for (value, width, bytes) in [
        (320, 2, 320f32.to_le_bytes().to_vec()),
        (0xffff, 2, (-1f32).to_le_bytes().to_vec()),
        (16_777_217, 4, 16_777_217f64.to_le_bytes().to_vec()),
    ] {
        let constant = sem(Operation::FloatLoad, "fild", vec![st(0)], vec![Loc::Imm(Imm { value, width, address: None })]);
        let instruction = Arc::new(Insn::new(0, Some((0, 2)), Some(constant), vec![], vec![]));
        let body = LirBody::new("constant", 0, vec![LirBlock::new(0, vec![instruction])], IndexMap::default(), IndexMap::default());
        let mut pool = Pool::new(7);
        let allocated = allocated(&body, None, Some(&mut pool), true, "386").unwrap();
        let insns = allocated.insns();
        assert_eq!(insns.len(), 1, "{insns:?}");
        assert_eq!(name(&insns[0]), "fld");
        let Loc::Mem(cell) = &what(&insns[0]).sources[0] else { panic!("{insns:?}") };
        assert_eq!((cell.addr.map(|addr| (addr.space, addr.index)), cell.width), (Some((Space::Segment, 7)), bytes.len() as u32));
        assert_eq!(pool.entries().collect::<Vec<_>>(), [(bytes.as_slice(), 7)]);
    }
}

#[test]
fn test_a_loop_accumulator_stays_on_the_stack_across_the_back_edge() {
    // Every block edge was a region end: the running sum went through a ten-byte cell each iteration.
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
        block(0, insns[..1].to_vec(), vec![16]),
        LirBlock { phis: vec![Phi { result: 2, incoming: vec![(0, 1), (16, 4)] }], ..block(16, insns[1..4].to_vec(), vec![16, 48]) },
        block(48, insns[4..].to_vec(), vec![]),
    ];
    let result = with_frame(&body, &mut Frame::new(-24));
    let insns = _along(&result, &[0, 16, 16, 16, 48]);
    let (_, stack, stores) = _x87_traced(&insns, &[(&start, 1.0), (&step, 2.0)]);
    assert_eq!((stores, stack), (vec![(answer.clone(), 7.0)], vec![]));
    let looped: Vec<Arc<Insn>> = result
        .blocks
        .iter()
        .filter(|block| block.at == 16 || (block.at != 0 && block.succ == vec![16]))
        .flat_map(|block| block.insns.clone())
        .collect();
    let memory: Vec<&Semantics> = looped.iter().map(|one| what(one)).filter(|what| what.sources.iter().chain(&what.dests).any(|arg| matches!(arg, Loc::Mem(_)))).collect();
    assert_eq!(memory.len(), 1, "{memory:?}");
    assert!(memory[0].sources.contains(&m(&step)));
}

/// Loads and stores of the allocator's own 8-byte cells.
fn spill_traffic(result: &LirBody) -> usize {
    result
        .insns()
        .iter()
        .filter(|one| what(one).sources.iter().chain(&what(one).dests).any(|arg| matches!(arg, Loc::Mem(cell) if cell.width == 8 && cell.through == Register::BP)))
        .count()
}

#[test]
fn test_a_copy_between_spilled_values_is_no_instruction() {
    // Each value had its own cell, so a phi's copy across calls loaded one cell and stored another.
    let (source, target) = (frame_cell(-4, 4), frame_cell(-8, 4));
    let call = || sem(Operation::Call, "call", vec![], vec![]);
    let body = _body(vec![
        _load(1, &source),
        fchs(2, 1),
        call(),
        sem(Operation::Move, "mov", vec![fl(3)], vec![fl(2)]),
        call(),
        _store(&target, 3),
    ]);
    let result = with_frame(&body, &mut Frame::new(-8));
    assert_eq!(spill_traffic(&result), 2);
    let (memory, stack) = _x87(&result.insns(), &[(&source, 3.0)]);
    assert_eq!((memory[&target], stack), (-3.0, vec![]));
}

#[test]
fn test_a_value_every_successor_spills_leaves_in_memory() {
    // The first exit's stack fixed the bundle, so another exit reloaded a value only to store it again.
    let (source, target) = (frame_cell(-4, 4), frame_cell(-8, 4));
    let call = || sem(Operation::Call, "call", vec![], vec![]);
    let mut body = _body(vec![
        _load(1, &source),
        fchs(2, 1),
        Semantics { target: Some(16), ..sem(Operation::Branch, "jne", vec![], vec![]) },
        call(),
        call(),
        _store(&target, 2),
    ]);
    let insns = body.insns();
    body.blocks = vec![
        block(0, insns[..3].to_vec(), vec![8, 16]),
        block(8, insns[3..4].to_vec(), vec![16]),
        block(16, insns[4..].to_vec(), vec![]),
    ];
    let result = with_frame(&body, &mut Frame::new(-8));
    assert_eq!(spill_traffic(&result), 2);
    for path in [vec![0, 16], vec![0, 8, 16]] {
        let (memory, stack) = _x87(&_along(&result, &path), &[(&source, 3.0)]);
        assert_eq!((memory[&target], stack), (-3.0, vec![]));
    }
}

