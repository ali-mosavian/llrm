//! Port of `tests/test_raising_copies.py`.
//!
//! A synthetic `cld / mov si / mov di / push ds / pop es / movsw`
//! covers `scalar`; its expected text is the same body run through Python.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::frontend::declen;
use crate::model::ir::nodes::Opaque;
use crate::model::ir::Effects;
use crate::model::mir::{MirBlock, MirBody};
use std::rc::Rc;

use num_bigint::BigInt;

use crate::analysis::consts;
use crate::backend::lower;
use crate::frontend::raising_literals;
use crate::model::ir::semantics::instruction_effects;
use crate::model::mir::Phi;
use crate::objectfile::module::literal_only;
use crate::objectfile::module::tests::{fixtures, loaded};
use crate::support::pyrepr::Repr;
use crate::support::testing;

fn occurrence(at: usize, raw: &[u8], defs: &[Register], op: Op) -> Op {
    let mut code = vec![0; at];
    code.extend_from_slice(raw);
    let decoded = declen::decode(&code, at).unwrap();
    let covers = (decoded.at as i64, decoded.end() as i64);
    let effects = Effects { defs: Some(defs.iter().copied().collect::<BTreeSet<_>>()), ..Effects::no_effect() };
    let node = Arc::new(Node::Opaque(Opaque::new(decoded, effects)));
    mir::raising_occurrence(&op, covers, Vec::new(), Some(node))
}

fn body(index: i64, offset: i64, pushed: bool) -> RaisedBody {
    let (s0, d0, s1, d1) = (Value::new(1, 1), Value::new(2, 2), Value::new(3, 3), Value::new(4, 4));
    let nothing = |at: i64| Op::new(at, OpCode::Operation(Operation::Barrier), "", vec![], vec![]);
    let copy = |at: i64, value: Value, offset: i64, id: u32| {
        let mut op = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![value], vec![]);
        op.kind = Kind::Copy;
        op.args = vec![Arg::Symbol(Symbol::new(Space::Segment, index, offset, 2))];
        op.results = vec![Arg::Held(Held { value, width: 2 })];
        op.id = Some(id);
        op
    };
    let mut ops = vec![
        occurrence(0x10, b"\xfc", &[], nothing(0x10)),
        occurrence(0x11, b"\xbe\x00\x00", &[Register::ESI], copy(0x11, s0, offset, 900)),
        occurrence(0x14, b"\xbf\x00\x00", &[Register::EDI], copy(0x14, d0, offset + 8, 901)),
    ];
    if pushed {
        ops.push(occurrence(0x17, b"\x1e", &[Register::ESP], nothing(0x17)));
        ops.push(occurrence(0x18, b"\x07", &[Register::ES, Register::ESP], nothing(0x18)));
    }
    let movsw = Op::new(0x19, OpCode::Operation(Operation::Barrier), "", vec![s1, d1], vec![s0, d0]);
    ops.push(occurrence(0x19, b"\xa5", &[Register::ESI, Register::EDI], movsw));
    let mut made = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]));
    made.origin =
        [(s0, Register::ESI), (s1, Register::ESI), (d0, Register::EDI), (d1, Register::EDI)].into_iter().collect();
    made
}

fn summary(body: &RaisedBody) -> String {
    let refs = |refs: &[MemRef]| {
        let each: Vec<String> =
            refs.iter().map(|r| format!("{}:{}", r.addr.map_or("None".to_owned(), |addr| addr.repr()), r.width)).collect();
        format!("[{}]", each.join(", "))
    };
    let values = |values: &[Value]| format!("[{}]", values.iter().map(Value::repr).collect::<Vec<_>>().join(", "));
    let args = |args: &[Arg]| format!("[{}]", args.iter().map(Arg::repr).collect::<Vec<_>>().join(", "));
    let mut lines = Vec::new();
    for block in &body.blocks {
        for op in &block.ops {
            let merges: Vec<String> =
                op.merges.iter().map(|(source, result)| format!("({}, {})", source.repr(), result.repr())).collect();
            lines.push(format!(
                "{:#x} {} args={} results={} defines={} uses={} loads={} stores={} merges=[{}] ranges={:?}",
                op.at,
                op.kind.as_str(),
                args(&op.args),
                args(&op.results),
                values(&op.defines),
                values(&op.uses),
                refs(&op.loads),
                refs(&op.stores),
                merges.join(", "),
                mir::raising_ranges(op),
            ));
        }
    }
    lines.join("\n")
}

fn memref(addr: &str) -> String {
    format!(
        "MemRef(addr={addr}, width=2, base=None, segment=None, space=None, beyond=None, symbolic=None, \
         allocation=None, base_width=4, pointer=False, excludes=(), typed=None, within=None, provenance=None, \
         volatile=False, inbounds=False)"
    )
}

#[test]
fn scalar_copies_a_proven_movsw_as_python_does() {
    let found = loaded(fixtures().join("fpdeep-p-g2.obj")).unwrap();
    let (source, dest) = (memref("[seg:5+0x10]"), memref("[seg:5+0x18]"));
    let expected = [
        "0x10 opaque args=[] results=[] defines=[] uses=[] loads=[] stores=[] merges=[] ranges=[(16, 17)]".to_owned(),
        "0x11 nothing args=[] results=[] defines=[] uses=[] loads=[] stores=[] merges=[] ranges=[(17, 20)]".to_owned(),
        "0x14 nothing args=[] results=[] defines=[] uses=[] loads=[] stores=[] merges=[] ranges=[(20, 23)]".to_owned(),
        "0x17 opaque args=[] results=[] defines=[] uses=[] loads=[] stores=[] merges=[] ranges=[(23, 24)]".to_owned(),
        "0x18 opaque args=[] results=[] defines=[] uses=[] loads=[] stores=[] merges=[] ranges=[(24, 25)]".to_owned(),
        format!("0x19 load args=[Cell(ref={source})] results=[Held(value=v1_1, width=2)] defines=[v1_1] uses=[] loads=[[seg:5+0x10]:2] stores=[] merges=[] ranges=[(25, 26)]"),
        format!("0x19 store args=[Held(value=v1_1, width=2)] results=[Cell(ref={dest})] defines=[] uses=[v1_1] loads=[] stores=[[seg:5+0x18]:2] merges=[] ranges=[]"),
    ]
    .join("\n");
    assert_eq!(summary(&scalar(body(5, 0x10, true), &found)), expected);
    // Without `push ds / pop es` the selector is unproved and nothing changes.
    assert_eq!(scalar(body(5, 0x10, false), &found), body(5, 0x10, false));
}

fn _instruction(raw: &[u8]) -> Op {
    let mut code = vec![0; 0x14b];
    code.extend_from_slice(raw);
    let decoded = declen::decode(&code, 0x14b).unwrap();
    let effects = instruction_effects(&decoded, &literal_only);
    let covers = (decoded.at as i64, decoded.end() as i64);
    let mut op = Op::new(decoded.at as i64, OpCode::Operation(Operation::Barrier), "", vec![], vec![]);
    op.memory_complete = effects.memory_complete;
    op.loads = effects.loads.iter().map(|cell| MemRef::new(cell.addr, cell.width)).collect();
    op.stores = effects.stores.iter().map(|cell| MemRef::new(cell.addr, cell.width)).collect();
    mir::raising_occurrence(&op, covers, Vec::new(), Some(Arc::new(Node::Opaque(Opaque::new(decoded, effects)))))
}

/// FPDEEP's `d = 12` copy, alone in one block, after `byte` (a direction flag setter) if any.
fn _copy(byte: Option<u8>) -> (Module, RaisedBody) {
    let path = "fixtures/omf/fpdeep-p-g2.obj";
    let found = testing::loaded(path).unwrap();
    let raised = mir::bodies(&found, &testing::partitioned(path), None, false, false).unwrap();
    let public = &raised.values[0].1;
    let body = mir::_with_hints(public, &raised.hints[&public.entry]);
    let with_node = |op: &Op| {
        let mut op = op.clone();
        let node = op.id.and_then(|id| raised.source.nodes.get(&id)).cloned();
        op.raising = Some(Box::new(mir::Raising { node, covers: None, extra_covers: Vec::new() }));
        op
    };
    let mut ops: Vec<Op> =
        body.blocks.iter().flat_map(|block| &block.ops).filter(|op| (0x14c..=0x157).contains(&op.at)).map(with_node).collect();
    if let Some(byte) = byte {
        ops.insert(0, _instruction(&[byte]));
    }
    let mut body = body.with_blocks(vec![MirBlock::new(body.entry, vec![], ops, vec![])]);
    body.body.initial = vec![];
    (found, body)
}

fn kinds(block: &MirBlock, kind: Kind) -> Vec<Op> {
    block.ops.iter().filter(|op| op.kind == kind).cloned().collect()
}

/// FPDEEP's d=12 needs memory effects, not eight unused pointer definitions.
#[test]
#[ignore = "fails in Python too: lowered refuses the cld, 'no instruction for opaque'"]
fn test_copy_has_explicit_memory_and_pointer_results() {
    for (byte, step) in [(0xfc, 2), (0xfd, -2)] {
        let (found, body) = _copy(Some(byte));
        let raised = scalar(body, &found);
        let (reads, writes) = (kinds(&raised.blocks[0], Kind::Load), kinds(&raised.blocks[0], Kind::Store));
        let disps = |ops: &[Op], stores: bool| -> Vec<i64> {
            ops.iter().map(|op| if stores { &op.stores[0] } else { &op.loads[0] }.addr.unwrap().disp).collect()
        };
        assert_eq!(disps(&reads, false), (0..4).map(|index| 0x22 + step * index).collect::<Vec<_>>());
        assert_eq!(disps(&writes, true), (0..4).map(|index| 0x1a + step * index).collect::<Vec<_>>());
        assert!(reads.iter().zip(&writes).all(|(load, store)| store.args == load.results));
        assert!(!raised.blocks[0].ops.iter().any(|op| !op.merges.is_empty() && op.at >= 0x154));
        let lowered = lower::lowered("copy", &raised, Some(&IndexMap::default()), BTreeSet::new(), Some(&IndexMap::default()), "386", Default::default())
            .unwrap();
        let moves = lowered.insns().into_iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move));
        assert_eq!(moves.count(), 8, "{byte:#x}");
    }
}

/// Reading the last copy's pointer must not resurrect its three intermediate updates.
#[test]
fn test_only_observed_pointer_results_cross_the_raise_boundary() {
    let (found, body) = _copy(Some(0xfc));
    let original = body.blocks[0].ops.clone();
    let last = original.last().unwrap().defines[0];
    let cell = MemRef::new(None, 4);
    let mut observe = Op::new(0x158, OpCode::Operation(Operation::Move), "mov", vec![], vec![last]);
    observe.kind = Kind::Store;
    observe.args = vec![Arg::Held(Held { value: last, width: 4 })];
    observe.stores = vec![cell.clone()];
    observe.results = vec![Arg::Cell(Cell { r#ref: cell })];
    let mut ops = original.clone();
    ops.push(observe);
    let body = body.with_blocks(vec![body.blocks[0].with_ops(ops)]);
    let result = scalar(body.clone(), &found);
    let updates: Vec<Op> = kinds(&result.blocks[0], Kind::Copy).into_iter().filter(|op| op.at >= 0x154).collect();
    assert!(updates.len() == 1 && updates[0].defines == [last]);
    let first = original.iter().find(|op| op.at == 0x154).unwrap();
    let before = *first.uses.iter().find(|value| body.origin.get(value) == body.origin.get(&last)).unwrap();
    assert_eq!(updates[0].merges, [(before, last)].into_iter().collect());
    assert_eq!(updates[0].results, [Arg::Held(Held { value: last, width: 2 })]);
}

/// Copy outputs consumed by a phi are uses even without a local reader.
#[test]
fn test_pointer_used_on_a_successor_edge_survives() {
    let (found, body) = _copy(Some(0xfc));
    let entry = body.blocks[0].clone();
    let last = entry.ops.last().unwrap().defines[0];
    let joined = Value { id: last.id + 1000, at: 0x200, version: last.version + 1, ..last };
    let mut phi = Phi::new(joined);
    phi.incoming.insert(entry.at, last);
    let successor = MirBlock::new(0x200, vec![phi], vec![], vec![]);
    let mut head = entry.clone();
    head.succ = vec![successor.at];
    let result = scalar(body.with_blocks(vec![head, successor]), &found);
    let updates: Vec<Op> = kinds(&result.blocks[0], Kind::Copy).into_iter().filter(|op| op.at >= 0x154).collect();
    assert!(updates.len() == 1 && updates[0].defines == [last]);
}

/// The FPDEEP initializer is binary64 12, not an unknown write or an entry value of d.
#[test]
fn test_forward_copy_propagates_the_double_literal() {
    let (found, body) = _copy(Some(0xfc));
    let body = raising_literals::initialized(scalar(body, &found), &found, None).unwrap();
    let known = consts::known(&Rc::new(body.body.clone()), Some(&found.dgroup.members), Some(&IndexMap::default()), None, None);
    let values: Vec<BigInt> = kinds(&body.blocks[0], Kind::Store)
        .iter()
        .map(|op| {
            let Arg::Held(held) = &op.args[0] else { panic!("{:?}", op.args) };
            known[&held.value].n.clone()
        })
        .collect();
    assert_eq!(values, [0, 0, 0, 0x4028].map(BigInt::from));
}

/// FPDEEP's d=12 copy must not become unknown just because setup crosses an
/// edge; a backwards incoming path must still prevent folding it.
#[test]
fn test_copy_environment_survives_only_agreeing_predecessors() {
    for conflict in [&b""[..], b"\xfd", b"\x1f", b"\x9a\x00\x00\x00\x00"] {
        let (found, body) = _copy(Some(0xfc));
        let entry = body.blocks[0].clone();
        let (setup, copying) = entry.ops.split_at(5);
        let left = MirBlock::new(0x180, vec![], vec![], vec![0x200]);
        let right = MirBlock::new(
            0x190,
            vec![],
            if conflict.is_empty() { vec![] } else { vec![_instruction(conflict)] },
            vec![0x200],
        );
        let join = MirBlock::new(0x200, vec![], copying.to_vec(), vec![]);
        let mut head = entry.with_ops(setup.to_vec());
        head.succ = vec![left.at, right.at];
        let body = body.with_blocks(vec![head.clone(), left.clone(), right.clone(), join.clone()]);
        let raised = scalar(body.clone(), &found);
        let stores = kinds(raised.blocks.last().unwrap(), Kind::Store);
        assert_eq!(stores.len(), if conflict.is_empty() { 4 } else { 0 }, "{conflict:x?}");
        let reordered = body.with_blocks(vec![join, right, left, head]);
        let raised = scalar(reordered, &found);
        assert_eq!(kinds(&raised.blocks[0], Kind::Store).len(), stores.len(), "{conflict:x?}");
    }
}

/// A near-pointer wrap is not an address in the next relocated segment.
#[test]
fn test_copy_does_not_advance_a_symbol_beyond_its_segment() {
    let (found, body) = _copy(Some(0xfc));
    let limits = omf::segments(&found.records);
    let ops = body.blocks[0]
        .ops
        .iter()
        .map(|op| {
            let mut op = op.clone();
            if op.at == 0x14f {
                let Arg::Symbol(symbol) = &op.args[0] else { panic!("{:?}", op.args) };
                let size = limits[symbol.index as usize].as_ref().unwrap().1;
                op.args = vec![Arg::Symbol(Symbol { offset: size - 2, ..symbol.clone() })];
            }
            op
        })
        .collect();
    let body = body.with_blocks(vec![body.blocks[0].with_ops(ops)]);
    assert_eq!(scalar(body.clone(), &found), body);
}

#[test]
fn test_unproved_copy_environment_is_not_assumed() {
    for change in ["unknown_direction", "unknown_selector", "changed_data_segment", "call"] {
        let (found, body) = _copy(if change == "unknown_direction" { None } else { Some(0xfc) });
        let mut ops = body.blocks[0].ops.clone();
        match change {
            "unknown_selector" => ops.retain(|op| op.at != 0x152),
            "changed_data_segment" => ops.insert(1, _instruction(b"\x1f")),
            "call" => ops.insert(1, _instruction(b"\x9a\x00\x00\x00\x00")),
            _ => {}
        }
        let body = body.with_blocks(vec![body.blocks[0].with_ops(ops)]);
        assert_eq!(scalar(body.clone(), &found), body, "{change}");
    }
}

fn lowered(body: &MirBody) -> crate::model::lir::LirBody {
    lower::lowered("copy", body, Some(&IndexMap::default()), BTreeSet::new(), Some(&IndexMap::default()), "386", Default::default())
        .unwrap()
}

/// MOVSW preserves arithmetic flags even though its pointer offsets change.
#[test]
#[ignore = "fails in Python too: 0x014b: no instruction for opaque"]
fn test_copy_selects_without_clobbering_arithmetic_flags() {
    let (found, body) = _copy(Some(0xfc));
    let mut low = lowered(&scalar(body, &found));
    let frame = crate::backend::frame::of(&low, Some(&IndexMap::default()), "", None).unwrap();
    let frame = Rc::new(std::cell::RefCell::new(frame));
    for mut stage in crate::flow::machine(&IndexMap::default(), Some(frame), Some(&IndexMap::default()), false, "386").unwrap() {
        low = stage.transform(low).unwrap();
    }
    for op in low.insns() {
        if let Some(what) = op.what.as_ref().filter(|what| what.op == Operation::Move) {
            let encoded = crate::backend::select::emit(what, 0, None, false, false, None).unwrap();
            assert_eq!(declen::decode(&encoded.code, 0).unwrap().writes(), 0);
        }
    }
}

/// FPDEEP's repeated DOUBLE load becomes one value once d=12 is represented.
#[test]
#[ignore = "fails in Python too: 0x014b: no instruction for opaque"]
fn test_proven_copy_unlocks_strict_floating_cse() {
    let (found, body) = _copy(Some(0xfc));
    let original = testing::nth(&testing::raised("fixtures/omf/fpdeep-p-g2.obj"), 0);
    let floating: Vec<Op> = testing::ops(&original).into_iter().filter(|op| (0x158..=0x174).contains(&op.at)).collect();
    let sequence: Vec<i64> = floating.iter().filter_map(|op| op.floating_origin.as_ref().map(|origin| origin.at)).collect();
    let floating = floating.into_iter().map(|mut op| {
        if let Some(origin) = op.floating_origin.as_mut() {
            origin.block = body.entry;
            origin.sequence = sequence.clone();
        }
        op
    });
    let mut ops = body.blocks[0].ops.clone();
    ops.extend(floating);
    let body = body.with_blocks(vec![body.blocks[0].with_ops(ops)]);
    let body = raising_literals::initialized(scalar(body, &found), &found, None).unwrap();
    let result = crate::optimize::transform::subexpressions(&Rc::new(body.body), &found.dgroup.members, false).unwrap();
    assert_eq!(testing::ops(&result).iter().filter(|op| op.kind == Kind::Fload).count(), 1);
    let low = crate::backend::floatalloc::allocated(&lowered(&result), None, false, "386").unwrap();
    let memory_arithmetic: Vec<String> = low
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| what.op == Operation::FloatArith && what.sources.iter().any(|source| matches!(source, crate::model::ir::Loc::Mem(_))))
        .map(|what| what.name.clone().unwrap_or_default())
        .collect();
    assert_eq!(memory_arithmetic, ["fdivr", "fmul"]);
}
