//! Port of `qbopt/frontend/raising_float_calls.py`: recognize
//! integer-to-floating helpers as conversions of ordinary MIR values.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::abi::runtime::{self, Contract, Control, Memory};
use crate::analysis::ssa;
use crate::model::ir::{Loc, Operation, St};
use crate::model::mir::{self, Arg, Const, Held, Kind, OpCode, Opaque, OrderedMap, RaisedBody, Synth, Value};
use crate::model::mir::SourceMap;
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;

const _WIDTHS: [(&str, u32); 2] = [("B$FILD", 4), ("B$FIL2", 2)];

/// `definitions` maps a value to its op's index in the block and its
/// position among the rewritten ops.
fn _source(
    op: &mir::Op,
    width: u32,
    origin: &OrderedMap<Value, Register>,
    definitions: &IndexMap<Value, (usize, usize)>,
    ops: &[mir::Op],
) -> Option<Arg> {
    let mut inputs: IndexMap<Option<Register>, &Held> = IndexMap::default();
    for arg in &op.args {
        if let Arg::Held(held) = arg {
            inputs.insert(origin.get(&held.value).copied(), held);
        }
    }
    let argument = inputs.get(&Some(Register::EAX)).copied();
    if width == 2 {
        return argument.map(|one| Arg::Held(*one));
    }
    let producer = |arg: Option<&Held>| arg.and_then(|arg| definitions.get(&arg.value)).map(|&(_, at)| &ops[at]);
    let (low, high) = (producer(argument), producer(inputs.get(&Some(Register::EDX)).copied()));
    if let (Some(low), Some(high)) = (low, high) {
        if low.kind == Kind::Extract
            && high.kind == Kind::Extract
            && low.args.len() == 2
            && high.args.len() == 2
            && low.args[1] == Arg::Const(Const::new(0, 4))
            && high.args[1] == Arg::Const(Const::new(16, 4))
            && low.args[0] == high.args[0]
            && matches!(low.args[0], Arg::Held(_))
        {
            return Some(low.args[0].clone());
        }
    }
    None
}

/// Whether every read definition is the sign half this can name instead.
fn _signs(served: &[Value], width: u32, origin: &OrderedMap<Value, Register>) -> bool {
    width == 2 && served.iter().all(|one| !one.flags && origin.get(one) == Some(&Register::EDX))
}

pub fn raised(
    body: RaisedBody,
    found: &Module,
    contracts: &IndexMap<i64, Contract>,
    source_map: &mut SourceMap,
) -> RaisedBody {
    let emulated = omf::externals(&found.records).iter().any(|one| one == "FIDRQQ");
    let mut serial = ssa::values(&body).map(|one| one.id).max().unwrap_or(0);
    let mut variable = ssa::values(&body).map(|one| one.variable).max().unwrap_or(0);
    let local = module::defines(&found.records, found.seg);
    let mut read: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| block.ops.iter().flat_map(|op| op.uses.iter().copied())).collect();
    read.extend(body.blocks.iter().flat_map(|block| block.phis.iter().flat_map(|phi| phi.incoming.values().copied())));
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut definitions: IndexMap<Value, (usize, usize)> = IndexMap::default();
        let mut ops: Vec<mir::Op> = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            let name = found.calls.get(&op.at).map(String::as_str);
            let width = if emulated { _WIDTHS.iter().find(|one| Some(one.0) == name).map(|one| one.1) } else { None };
            let contract = contracts.get(&op.at);
            let expected = width.map(|_| runtime::contract(name));
            let served: Vec<Value> = op.defines.iter().copied().filter(|value| read.contains(value)).collect();
            let candidate = op.kind == Kind::Call
                && width.is_some()
                && !local.contains(name.unwrap())
                && contract.is_some_and(|contract| {
                    let expected = expected.as_ref().unwrap();
                    contract.established
                        && contract.writes == Memory::None
                        && contract.reads == Memory::None
                        && contract.cleanup == Some(0)
                        && contract.control == Control::Returns
                        && !(contract.enters_user_code || contract.raises_error || contract.error_handling)
                        && contract.inputs == expected.inputs
                        && contract.clobbers == expected.clobbers
                })
                && (served.is_empty() || _signs(&served, width.unwrap(), &body.origin));
            // `B$FIL2` is one instruction before `B$FILD`: a `cwd`, then it
            // falls through. So the `dx` its contract calls a clobber is the
            // sign of the word it was handed, and a read of it is served by
            // saying that rather than by keeping the call. Same rule as
            // `B$DSEG`, whose one clobber is the word it was handed unchanged.
            let signing = candidate && !served.is_empty();
            let mut argument =
                if candidate { _source(op, width.unwrap(), &body.origin, &definitions, &ops) } else { None };
            let held = match &argument {
                Some(Arg::Held(one)) => Some(*one),
                _ => None,
            };
            let source = held.and_then(|held| definitions.get(&held.value).copied());
            // Folding the load leaves nothing in ax for the sign to extend.
            let source = if signing { None } else { source };
            let mut load: Option<mir::Op> = None;
            if let Some((place, at)) = source {
                let producer = &ops[at];
                if producer.kind == Kind::Load
                    && producer.args.len() == 1
                    && matches!(&producer.args[0], Arg::Cell(cell) if Some(cell.r#ref.width) == width)
                    && block.ops[place + 1..index]
                        .iter()
                        .all(|one| one.stores.is_empty() && !one.barrier() && one.kind != Kind::Call)
                {
                    argument = Some(producer.args[0].clone());
                    load = Some(producer.clone());
                }
            }
            let mut op = op.clone();
            if let (Some(_), true, Some(held)) = (&argument, signing, held) {
                serial += 1;
                variable += 1;
                let wide = Value { variable, version: 1, ..Value::new(serial, op.at) };
                let mut extend = op.clone();
                extend.kind = Kind::SignExtend;
                extend.op = Some(OpCode::Operation(Operation::Extend));
                extend.name = "sign_extend".to_owned();
                extend.args = vec![Arg::Held(held)];
                extend.results = vec![Arg::Held(Held { value: wide, width: 4 })];
                extend.defines = vec![wide];
                extend.uses = vec![held.value];
                extend.loads = Vec::new();
                extend.stores = Vec::new();
                extend.merges = OrderedMap::new();
                extend.raised = None;
                extend.id = None;
                extend.stack = None;
                extend.symbol = None;
                ops.push(mir::source_free(extend));
                let mut extract = op.clone();
                extract.kind = Kind::Extract;
                extract.op = Some(OpCode::Synth(Synth::HalfToLow));
                extract.name = "extract".to_owned();
                extract.args = vec![Arg::Held(Held { value: wide, width: 4 }), Arg::Const(Const::new(16, 4))];
                extract.results = vec![Arg::Held(Held { value: served[0], width: 2 })];
                extract.defines = vec![served[0]];
                extract.uses = vec![wide];
                extract.loads = Vec::new();
                extract.stores = Vec::new();
                extract.merges = OrderedMap::new();
                extract.raised = None;
                extract.id = None;
                extract.stack = None;
                extract.symbol = None;
                ops.push(mir::source_free(extract));
            }
            if let Some(argument) = argument {
                let reference = match &argument {
                    Arg::Cell(cell) => Some(cell.r#ref.clone()),
                    _ => None,
                };
                let uses = match (&argument, &reference) {
                    (Arg::Held(one), _) => vec![one.value],
                    (_, Some(reference)) => [reference.base, reference.segment].into_iter().flatten().collect(),
                    _ => Vec::new(),
                };
                op.kind = Kind::Fload;
                op.op = Some(OpCode::Operation(Operation::FloatLoad));
                op.name = "fild".to_owned();
                op.args = vec![argument];
                op.results = vec![Arg::Opaque(Opaque::named(Some(Loc::St(St { index: 0 })), "st0"))];
                op.uses = uses;
                op.defines = Vec::new();
                op.symbol = Some(reference.is_some());
                op.loads = reference.into_iter().collect();
                op.stores = Vec::new();
                op.merges = OrderedMap::new();
                op.raised = None;
                op.stack = Some(1);
                op = mir::detached(op);
                if let Some(load) = &load {
                    let alone = body.body.with_blocks(vec![block.with_ops(vec![load.clone()])]);
                    for (id, refs) in mir::_referenced(&alone, found) {
                        source_map.refs.insert(id, refs);
                    }
                    if let (Some(load_id), Some(id)) = (load.id, op.id) {
                        if let Some(refs) = source_map.refs.get(&load_id).cloned() {
                            source_map.refs.insert(id, refs);
                        }
                    }
                }
                // The helper supplied its own emulator dispatch. Its replacement
                // must retain that protocol, independent of the call's opcode.
                if let Some(id) = op.id {
                    source_map.float_protocols.insert(id, 0x34);
                }
            }
            for value in &op.defines {
                definitions.insert(*value, (index, ops.len()));
            }
            ops.push(op);
        }
        blocks.push(block.with_ops(ops));
    }
    _comparisons(body.with_blocks(blocks), found, contracts)
}

/// Raise B$FCMP as the floating comparison it implements.
///
/// The two operands already stand on the x87 stack. Giving the helper its
/// semantic stack effect lets `raising_float_values` resolve them to the
/// same ordinary value operands emitted by source frontends.
fn _comparisons(body: RaisedBody, found: &Module, contracts: &IndexMap<i64, Contract>) -> RaisedBody {
    let mut used: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| block.ops.iter().flat_map(|op| op.uses.iter().copied())).collect();
    used.extend(body.blocks.iter().flat_map(|block| block.phis.iter().flat_map(|phi| phi.incoming.values().copied())));
    let local = module::defines(&found.records, found.seg);
    let expected = runtime::contract(Some("B$FCMP"));
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let name = found.calls.get(&op.at).map(String::as_str);
            let rule = contracts.get(&op.at);
            let live_flags: Vec<Value> =
                op.defines.iter().copied().filter(|value| value.flags && used.contains(value)).collect();
            let live_data: Vec<Value> =
                op.defines.iter().copied().filter(|value| !value.flags && used.contains(value)).collect();
            let candidate = op.kind == Kind::Call
                && name == Some("B$FCMP")
                && !local.contains("B$FCMP")
                && rule.is_some_and(|rule| {
                    rule.established
                        && rule.writes == Memory::None
                        && rule.reads == Memory::None
                        && rule.cleanup == Some(0)
                        && rule.control == Control::Returns
                        && !(rule.enters_user_code || rule.raises_error || rule.error_handling)
                        && rule.inputs == expected.inputs
                        && rule.clobbers == expected.clobbers
                })
                && live_flags.len() == 1
                && live_data.len() <= 1;
            if !candidate {
                ops.push(op.clone());
                continue;
            }
            let mut made = op.clone();
            made.op = Some(OpCode::Operation(Operation::Nothing));
            made.name = String::new();
            made.kind = Kind::Fcompare;
            made.defines = live_flags.into_iter().chain(live_data).collect();
            made.uses = Vec::new();
            made.args = vec![
                Arg::Opaque(Opaque::named(Some(Loc::St(St { index: 0 })), "st0")),
                Arg::Opaque(Opaque::named(Some(Loc::St(St { index: 1 })), "st1")),
            ];
            made.results = Vec::new();
            made.loads = Vec::new();
            made.stores = Vec::new();
            made.merges = OrderedMap::new();
            made.raised = None;
            made.stack = Some(-2);
            made.symbol = Some(false);
            made.args_known = true;
            made.memory_complete = true;
            made.reads_complete = true;
            made.opaque_defs = Some(BTreeSet::new());
            made.opaque_uses = Some(BTreeSet::new());
            ops.push(mir::detached(made));
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
