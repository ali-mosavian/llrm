//! Port of `qbopt/frontend/raising_float_values.py`: resolve self-contained
//! floating stacks to ordinary MIR value operands.

use std::collections::BTreeSet;

use crate::analysis::ssa;
use crate::frontends::bc::fpstack::{self, Float, Reading};
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, FloatingOrigin, Held, Kind, MirBlock, Op, OpCode, OrderedMap, RaisedBody, Value};
use crate::support::hash::{IndexMap, IndexSet};
use crate::support::pyset::PySet;

/// Balanced typed sequences, separated by calls or unknown stack effects,
/// as indices into `block.ops`.
fn _regions(block: &MirBlock) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let mut depth = 0;
    for (index, op) in block.ops.iter().enumerate() {
        if op.barrier()
            || op.kind == Kind::Call
            || (op.stack.is_some() && op.floating.is_none() && op.kind != Kind::Fcompare)
        {
            (run, depth) = (Vec::new(), 0);
            continue;
        }
        run.push(index);
        if let Some(stack) = op.stack {
            depth += stack;
            if depth < 0 {
                (run, depth) = (Vec::new(), 0);
                continue;
            }
        }
        if depth == 0 {
            if run.iter().any(|&one| block.ops[one].stack.is_some()) {
                out.push(run.clone());
            }
            run = Vec::new();
        }
    }
    out
}

const _ARITHMETIC: [Kind; 4] = [Kind::Fadd, Kind::Fsub, Kind::Fmul, Kind::Fdiv];

/// Arithmetic over values: a memory operand becomes its own load, which
/// carries the format.
pub fn loaded(body: RaisedBody) -> RaisedBody {
    let values: Vec<Value> = ssa::values(&body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let (Some(floating), [Arg::Held(kept), Arg::Cell(cell)]) = (&op.floating, op.args.as_slice()) else {
                ops.push(op.clone());
                continue;
            };
            if !_ARITHMETIC.contains(&op.kind) || !op.stores.is_empty() || op.loads.len() != 1 || kept.width != 10 {
                ops.push(op.clone());
                continue;
            }
            serial += 1;
            variable += 1;
            let read = Held { value: Value { variable, version: 1, ..Value::new(serial, op.at) }, width: 10 };
            let integer = op.name.starts_with("fi");
            let mut load = op.clone();
            load.kind = Kind::Fload;
            load.op = Some(OpCode::Operation(Operation::FloatLoad));
            load.name = if integer { "fild" } else { "fld" }.to_owned();
            load.args = vec![Arg::Cell(cell.clone())];
            load.results = vec![Arg::Held(read)];
            load.defines = vec![read.value];
            load.uses = op.uses.iter().copied().filter(|value| *value != kept.value).collect();
            load.merges = OrderedMap::new();
            load.floating = Some(Semantics::with_exceptions(
                [floating.inputs[1]],
                Format::Extended80,
                Precision::Exact,
                Rounding::None,
                floating.exceptions,
            ));
            load.floating_origin = None;
            load.stack = None;
            load.raised = None;
            load.symbol = None;
            load.source = Some(mir::next_id());
            ops.push(mir::source_free(load));
            let mut arithmetic = op.clone();
            if integer {
                arithmetic.name = format!("f{}", &op.name[2..]);
            }
            arithmetic.args = vec![Arg::Held(*kept), Arg::Held(read)];
            arithmetic.uses = vec![kept.value, read.value];
            arithmetic.loads = Vec::new();
            arithmetic.raised = None;
            arithmetic.floating = Some(Semantics {
                inputs: [Format::Extended80, Format::Extended80].into(),
                ..floating.clone()
            });
            ops.push(arithmetic);
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

pub fn raised(body: RaisedBody) -> RaisedBody {
    let values: Vec<Value> = ssa::values(&body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let regions = _regions(block);
        let operations: Vec<usize> =
            regions.iter().flatten().copied().filter(|&index| block.ops[index].stack.is_some()).collect();
        let mut readings: IndexMap<i64, Reading> = IndexMap::default();
        for region in &regions {
            let alone = block.with_ops(region.iter().map(|&index| block.ops[index].clone()).collect());
            readings.extend(fpstack::readings(&body.body.with_blocks(vec![alone])));
        }
        let ops_of = || operations.iter().map(|&index| &block.ops[index]);
        if operations.is_empty()
            || ops_of().any(|op| op.floating.is_none() && op.kind != Kind::Fcompare)
            || ops_of().map(|op| op.at).collect::<BTreeSet<_>>().len() != operations.len()
            || ops_of().map(|op| op.stack.unwrap()).sum::<i64>() != 0
        {
            blocks.push(block.clone());
            continue;
        }
        let mut known: PySet<Float> = PySet::new();
        let mut valid = true;
        for op in ops_of() {
            let reading = readings.get(&op.at);
            let Some(reading) = reading.filter(|reading| reading.uses.values().all(|value| known.contains(value)))
            else {
                valid = false;
                break;
            };
            if op.results.iter().any(|arg| matches!(arg, Arg::Opaque(_))) && reading.defines.is_none() {
                valid = false;
                break;
            }
            if let Some(defines) = reading.defines {
                known.add(defines);
            }
        }
        if !valid {
            blocks.push(block.clone());
            continue;
        }
        let mut held: IndexMap<Float, Held> = IndexMap::default();
        let mut sorted: Vec<Float> = known.iter().copied().collect();
        sorted.sort_by_key(|one| one.id);
        for value in sorted {
            serial += 1;
            variable += 1;
            let made = Value { variable, version: 1, ..Value::new(serial, value.at.unwrap()) };
            held.insert(value, Held { value: made, width: 10 });
        }
        let sequence: Vec<i64> = ops_of().map(|op| op.at).collect();
        let selected: BTreeSet<usize> = operations.iter().copied().collect();
        let mut ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            if !selected.contains(&index) {
                ops.push(op.clone());
                continue;
            }
            let reading = &readings[&op.at];

            let source = |arg: &Arg| -> Arg {
                if let Arg::Opaque(one) = arg {
                    if let Some(digits) = one.name.strip_prefix("st") {
                        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                            return Arg::Held(held[&reading.uses[&digits.parse::<i64>().unwrap()]]);
                        }
                    }
                }
                arg.clone()
            };

            let args: Vec<Arg> = op.args.iter().map(source).collect();
            let results: Vec<Arg> = op
                .results
                .iter()
                .map(|arg| match arg {
                    Arg::Opaque(_) => Arg::Held(held[&reading.defines.unwrap()]),
                    _ => arg.clone(),
                })
                .collect();
            let baseline = if op.kind == Kind::Fcompare {
                None
            } else {
                Some(FloatingOrigin {
                    block: block.at,
                    sequence: sequence.clone(),
                    at: op.at,
                    kind: op.kind,
                    semantics: op.floating.clone().unwrap(),
                    inputs: args.clone(),
                    outputs: results.clone(),
                    machine_inputs: op.args.clone(),
                    machine_outputs: op.results.clone(),
                })
            };
            let held_values = |operands: &[Arg]| -> Vec<Value> {
                operands
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Held(one) => Some(one.value),
                        _ => None,
                    })
                    .collect()
            };
            let mut made: Op = op.clone();
            made.uses = op.uses.iter().copied().chain(held_values(&args)).collect::<IndexSet<_>>().into_iter().collect();
            made.defines =
                op.defines.iter().copied().chain(held_values(&results)).collect::<IndexSet<_>>().into_iter().collect();
            made.args = args;
            made.results = results;
            made.floating_origin = baseline;
            ops.push(made);
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
