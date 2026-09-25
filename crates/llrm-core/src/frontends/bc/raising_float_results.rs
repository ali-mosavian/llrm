//! Port of `qbopt/frontend/raising_float_results.py`: recognize runtime
//! floating-to-integer conversions at the machine boundary.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::abi::runtime::{self, Contract};
use crate::analysis::ssa;
use crate::model::ir::{Loc, Operation, St};
use crate::model::mir::{self, Arg, Const, Held, Kind, Op, OpCode, OrderedMap, RaisedBody, Value};
use crate::model::mir::SourceMap;
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;

pub fn raised(body: RaisedBody, found: &Module, contracts: &IndexMap<i64, Contract>, source: &mut SourceMap) -> RaisedBody {
    if !omf::externals(&found.records).iter().any(|one| one == "FIDRQQ") {
        return body;
    }
    let local = module::defines(&found.records, found.seg);
    let values: Vec<Value> = ssa::values(&body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut used: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| block.ops.iter().flat_map(|op| op.uses.iter().copied())).collect();
    used.extend(body.blocks.iter().flat_map(|block| block.phis.iter().flat_map(|phi| phi.incoming.values().copied())));
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let name = found.calls.get(&op.at);
            let width: Option<u32> = match name.map(String::as_str) {
                Some("B$FIS2") => Some(2),
                Some("B$FIST") => Some(4),
                _ => None,
            };
            let rule = contracts.get(&op.at);
            let expected = width.map(|_| runtime::contract(name.map(String::as_str)));
            let mut outputs: IndexMap<Option<Register>, Value> = IndexMap::default();
            for value in op.defines.iter().filter(|value| !value.flags) {
                outputs.insert(body.origin.get(value).copied(), *value);
            }
            let registers: &[Register] = if width == Some(2) { &[Register::EAX] } else { &[Register::EAX, Register::EDX] };
            let Some(width) = width else {
                ops.push(op.clone());
                continue;
            };
            if local.contains(name.unwrap())
                || op.kind != Kind::Call
                || rule.is_none_or(|rule| !rule.established || Some(rule) != expected.as_ref())
                || !op.merges.is_empty()
                || !op.args.is_empty()
                || op.defines.iter().any(|value| value.flags && used.contains(value))
                || outputs.keys().copied().collect::<BTreeSet<_>>()
                    != registers.iter().map(|&one| Some(one)).collect::<BTreeSet<_>>()
            {
                ops.push(op.clone());
                continue;
            }
            serial += 1;
            variable += 1;
            let result = Held { value: Value { variable, version: 1, ..Value::new(serial, op.at) }, width };
            let mut converted = op.clone();
            converted.kind = Kind::Fstore;
            converted.op = Some(OpCode::Operation(Operation::FloatStore));
            converted.name = "fistp".to_owned();
            converted.args = vec![Arg::Opaque(mir::Opaque::named(Some(Loc::St(St { index: 0 })), "st0"))];
            converted.results = vec![Arg::Held(result)];
            converted.defines = vec![result.value];
            converted.uses = Vec::new();
            converted.loads = Vec::new();
            converted.stores = Vec::new();
            converted.merges = OrderedMap::new();
            converted.raised = None;
            converted.stack = Some(-1);
            converted.symbol = Some(false);
            ops.push(mir::detached(converted));
            if let Some(id) = op.id {
                source.float_protocols.insert(id, 0x34);
            }
            for (shift, register) in registers.iter().enumerate() {
                let target = outputs[&Some(*register)];
                let mut made = Op::new(
                    op.at,
                    OpCode::Operation(Operation::Move),
                    if width == 2 { "mov" } else { "" },
                    vec![target],
                    vec![result.value],
                );
                made.kind = if width == 2 { Kind::Copy } else { Kind::Extract };
                made.args = if width == 2 {
                    vec![Arg::Held(result)]
                } else {
                    vec![Arg::Held(result), Arg::Const(Const::new(16 * shift as i64, 4))]
                };
                made.results = vec![Arg::Held(Held { value: target, width: 2 })];
                made.symbol = Some(false);
                ops.push(made);
            }
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
