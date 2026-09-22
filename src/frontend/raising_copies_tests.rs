//! Port of `tests/test_raising_copies.py`.
//!
//! Skipped, needing `mir.bodies` on FPDEEP (`_copy`), `lower.lowered`,
//! `raising_literals`, `consts.known`, `flow.machine` or `transform`:
//! `test_copy_has_explicit_memory_and_pointer_results`,
//! `test_only_observed_pointer_results_cross_the_raise_boundary`,
//! `test_pointer_used_on_a_successor_edge_survives`,
//! `test_forward_copy_propagates_the_double_literal`,
//! `test_copy_environment_survives_only_agreeing_predecessors`,
//! `test_copy_selects_without_clobbering_arithmetic_flags`,
//! `test_copy_does_not_advance_a_symbol_beyond_its_segment`,
//! `test_proven_copy_unlocks_strict_floating_cse`,
//! `test_unproved_copy_environment_is_not_assumed`.
//!
//! Meanwhile a synthetic `cld / mov si / mov di / push ds / pop es / movsw`
//! covers `scalar`; its expected text is the same body run through Python.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::frontend::declen;
use crate::model::ir::nodes::Opaque;
use crate::model::ir::Effects;
use crate::model::mir::{MirBlock, MirBody};
use crate::objectfile::module::tests::{fixtures, loaded};
use crate::support::pyrepr::Repr;

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
