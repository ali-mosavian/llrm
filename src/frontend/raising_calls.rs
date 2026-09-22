//! Port of `qbopt/frontend/raising_calls.py`: recover arithmetic arguments
//! from the values present at each stack push.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::flags;
use crate::frontend::blocks::Block;
use crate::frontend::stack::PUSH_BYTES;
use crate::legacy::calls::{self, CallSite};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, Op, OpCode, OrderedMap, RaisedBody, Synth, Value};
use crate::objectfile::module::Module;
use crate::support::hash::IndexMap;

pub fn _discarded(push: &Op) -> Op {
    let mut made = Op::new(push.at, OpCode::Operation(Operation::Nothing), "", Vec::new(), Vec::new());
    made.kind = Kind::Nothing;
    made.id = push.id;
    mir::raising_owned(made, &[push])
}

pub fn _capture(push: &Op, arg: &Arg, held: &Held) -> Op {
    let memory = matches!(arg, Arg::Cell(_));
    let uses = match arg {
        Arg::Held(one) => vec![one.value],
        Arg::Cell(cell) => [cell.r#ref.base, cell.r#ref.segment].into_iter().flatten().collect(),
        _ => Vec::new(),
    };
    let mut made = push.clone();
    made.kind = if memory { Kind::Load } else { Kind::Copy };
    made.op = Some(OpCode::Operation(Operation::Move));
    made.name = "mov".to_owned();
    made.defines = vec![held.value];
    made.uses = uses;
    made.args = vec![arg.clone()];
    made.results = vec![Arg::Held(*held)];
    made.loads = match arg {
        Arg::Cell(cell) => vec![cell.r#ref.clone()],
        _ => Vec::new(),
    };
    made.stores = Vec::new();
    made.raised = None;
    made.merges = OrderedMap::new();
    made.stack = None;
    made
}

pub fn _whole_memory(group: &[&Op]) -> Option<MemRef> {
    let [high, low] = group else {
        return None;
    };
    let (Some(Arg::Cell(upper)), Some(Arg::Cell(lower))) = (high.args.first(), low.args.first()) else {
        return None;
    };
    if !mir::raising_adjacent(high, low) {
        return None;
    }
    let (upper, lower) = (&upper.r#ref, &lower.r#ref);
    let addr = lower.addr?;
    if lower.width != 2 || upper.width != 2 || (MemRef { addr: Some(addr.plus(2)), ..lower.clone() }) != *upper {
        return None;
    }
    Some(MemRef { width: 4, ..lower.clone() })
}

/// Python's `arithmetic(body, found, blocks, *, basic_semantics=False)`.
pub fn arithmetic(body: RaisedBody, found: &Module, blocks: &[Block], basic_semantics: bool) -> RaisedBody {
    let arithmetic_names = [calls::DIVIDE, calls::REMAINDER, calls::MULTIPLY, calls::COMPARE];
    let reached: Vec<_> = blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect();
    let sites: Vec<CallSite> = calls::sites(found, &reached, blocks)
        .into_iter()
        .map(|site| {
            if arithmetic_names.contains(&site.name.as_str()) && site.consume.is_empty() {
                let consume = reached
                    .iter()
                    .filter(|insn| site.start <= insn.at && insn.at < site.at && PUSH_BYTES.contains_key(&insn.code()))
                    .cloned()
                    .collect();
                CallSite { consume, ..site }
            } else {
                site
            }
        })
        .collect();
    let live_flags = flags::live_in(blocks);
    let mut candidates: IndexMap<i64, CallSite> = IndexMap::default();
    for site in sites {
        if !site.consume.is_empty()
            && arithmetic_names.contains(&site.name.as_str())
            && !(basic_semantics && calls::DIVIDES.contains(&site.name.as_str()))
            && calls::absorb(&site, mir::_flags_after(blocks, &live_flags, site.start, site.end), true).is_ok()
        {
            candidates.insert(site.at as i64, site);
        }
    }
    if candidates.is_empty() {
        return body;
    }
    let mut values: Vec<Value> = Vec::new();
    for block in &body.blocks {
        for op in &block.ops {
            values.extend(op.defines.iter().chain(&op.uses).copied());
        }
        for phi in &block.phis {
            values.push(phi.result);
            values.extend(phi.incoming.values().copied());
        }
    }
    values.extend(body.origin.keys().copied());
    let mut readers: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| block.ops.iter()).flat_map(mir::consumed).collect();
    let phis: Vec<&mir::Phi> = body.blocks.iter().flat_map(|block| block.phis.iter()).collect();
    loop {
        let incoming: BTreeSet<Value> = phis
            .iter()
            .filter(|phi| readers.contains(&phi.result))
            .flat_map(|phi| phi.incoming.values().copied())
            .collect();
        if incoming.is_subset(&readers) {
            break;
        }
        readers.extend(incoming);
    }
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);

    let mut fresh = |at: i64| -> Value {
        serial += 1;
        variable += 1;
        Value { id: serial, at, flags: false, variable, version: 1 }
    };

    let mut changed: Vec<MirBlock> = Vec::new();
    let mut removed_flags: BTreeSet<Value> = BTreeSet::new();
    for block in &body.blocks {
        // Python's `id(op)` keys are positions in `block.ops`.
        let mut pushes: IndexMap<i64, usize> = IndexMap::default();
        for (index, op) in block.ops.iter().enumerate() {
            if op.kind == Kind::Arg && op.args.len() == 1 {
                pushes.insert(op.at, index);
            }
        }
        let mut replacements: IndexMap<usize, Vec<Op>> = IndexMap::default();
        for (position, call) in block.ops.iter().enumerate() {
            let Some(site) = candidates.get(&call.at) else { continue };
            if call.kind != Kind::Call {
                continue;
            }
            let Some(groups) = calls::grouped(&site.consume) else { continue };
            if groups.len() != 2 {
                continue;
            }
            let incoming: Vec<Vec<Option<usize>>> = groups
                .iter()
                .map(|group| group.iter().map(|insn| pushes.get(&(insn.at as i64)).copied()).collect())
                .collect();
            if incoming.iter().flatten().any(|op| match op {
                None => true,
                Some(index) => {
                    replacements.contains_key(index)
                        || !matches!(block.ops[*index].args[0], Arg::Held(_) | Arg::Const(_) | Arg::Cell(_))
                }
            }) {
                continue;
            }
            let incoming: Vec<Vec<usize>> =
                incoming.into_iter().map(|group| group.into_iter().map(|op| op.expect("checked")).collect()).collect();
            let mut returns: IndexMap<Option<Register>, Value> = IndexMap::default();
            for value in call.defines.iter().filter(|value| !value.flags) {
                returns.insert(body.origin.get(value).copied(), *value);
            }
            let compare = site.name == calls::COMPARE;
            if compare && (!returns.is_empty() || call.defines.len() != 1) {
                continue;
            }
            if !compare && (!returns.contains_key(&Some(Register::EAX)) || !returns.contains_key(&Some(Register::EDX))) {
                continue;
            }
            if returns.iter().any(|(register, value)| {
                *register != Some(Register::EAX) && *register != Some(Register::EDX) && readers.contains(value)
            }) {
                continue;
            }
            let mut pending: IndexMap<usize, Vec<Op>> = IndexMap::default();
            let mut arguments: Vec<Held> = Vec::new();
            let mut setup: Vec<Op> = Vec::new();
            for (index, group) in incoming.iter().enumerate() {
                let literal = if site.pushed.len() == incoming.len() { Some(&site.pushed[index]) } else { None };
                if let Some(literal) = literal.filter(|literal| literal.kind == calls::Kind::Constant) {
                    let last = *group.last().expect("a group is never empty");
                    let held = Held { value: fresh(block.ops[last].at), width: 4 };
                    for &push in &group[..group.len() - 1] {
                        pending.insert(push, vec![_discarded(&block.ops[push])]);
                    }
                    pending.insert(last, vec![_capture(&block.ops[last], &Arg::Const(Const::new(literal.value, 4)), &held)]);
                    arguments.push(held);
                    continue;
                }
                let ops: Vec<&Op> = group.iter().map(|&index| &block.ops[index]).collect();
                if let Some(memory) = _whole_memory(&ops) {
                    let (high, low) = (group[0], group[1]);
                    let held = Held { value: fresh(block.ops[low].at), width: 4 };
                    pending.insert(high, vec![_discarded(&block.ops[high])]);
                    pending.insert(low, vec![_capture(&block.ops[low], &Arg::Cell(Cell { r#ref: memory }), &held)]);
                    arguments.push(held);
                    continue;
                }
                let mut words: Vec<Held> = Vec::new();
                for &push in group {
                    let arg = &block.ops[push].args[0];
                    let width = match arg {
                        Arg::Cell(cell) => cell.r#ref.width,
                        Arg::Held(held) => held.width,
                        Arg::Const(constant) => constant.width,
                        _ => unreachable!("checked above"),
                    };
                    let value = fresh(block.ops[push].at);
                    let held = Held { value, width };
                    pending.insert(push, vec![_capture(&block.ops[push], arg, &held)]);
                    words.push(held);
                }
                if words.len() == 1 && words[0].width == 4 {
                    arguments.push(words[0]);
                } else if words.len() == 2 && words.iter().all(|word| word.width == 2) {
                    let value = fresh(call.at);
                    let mut concat = Op::new(
                        call.at,
                        OpCode::Synth(Synth::ConcatLow),
                        "concat",
                        vec![value],
                        words.iter().map(|word| word.value).collect(),
                    );
                    concat.kind = Kind::Concat;
                    concat.args = words.iter().map(|word| Arg::Held(*word)).collect();
                    concat.results = vec![Arg::Held(Held { value, width: 4 })];
                    setup.push(concat);
                    arguments.push(Held { value, width: 4 });
                } else {
                    break;
                }
            }
            if arguments.len() != 2 {
                continue;
            }
            if compare {
                // CPI4 pushes left first, unlike multiply and divide.
                let mut comparison = Op::new(
                    call.at,
                    OpCode::Operation(Operation::Compare),
                    "cmp",
                    call.defines.clone(),
                    arguments.iter().map(|arg| arg.value).collect(),
                );
                comparison.kind = Kind::Sub;
                comparison.args = arguments.iter().map(|arg| Arg::Held(*arg)).collect();
                comparison.id = call.id;
                let comparison = mir::raising_owned(comparison, &[call]);
                replacements.extend(pending);
                setup.push(comparison);
                replacements.insert(position, setup);
                continue;
            }
            let (quotient, remainder) = (fresh(call.at), fresh(call.at));
            let multiply = site.name == calls::MULTIPLY;
            let answers = if multiply { vec![quotient] } else { vec![quotient, remainder] };
            // These runtime routines push their right operand first.
            let mut arithmetic = Op::new(
                call.at,
                OpCode::Operation(if multiply { Operation::Multiply } else { Operation::Divide }),
                if multiply { "imul" } else { "idiv" },
                answers.clone(),
                arguments.iter().rev().map(|arg| arg.value).collect(),
            );
            arithmetic.kind = if multiply { Kind::Mul } else { Kind::Divmod };
            arithmetic.args = arguments.iter().rev().map(|arg| Arg::Held(*arg)).collect();
            arithmetic.results = answers.iter().map(|value| Arg::Held(Held { value: *value, width: 4 })).collect();
            arithmetic.id = call.id;
            let arithmetic = mir::raising_owned(arithmetic, &[call]);
            let answer = if site.name == calls::REMAINDER { remainder } else { quotient };
            let extracts = [(Register::EAX, 0), (Register::EDX, 16)].map(|(register, offset)| {
                let returned = returns[&Some(register)];
                let mut extract =
                    Op::new(call.at, OpCode::Synth(Synth::HalfToLow), "extract", vec![returned], vec![answer]);
                extract.kind = Kind::Extract;
                extract.args = vec![Arg::Held(Held { value: answer, width: 4 }), Arg::Const(Const::new(offset, 4))];
                extract.results = vec![Arg::Held(Held { value: returned, width: 2 })];
                extract
            });
            replacements.extend(pending);
            setup.push(arithmetic);
            setup.extend(extracts);
            replacements.insert(position, setup);
            removed_flags.extend(call.defines.iter().copied().filter(|value| value.flags));
        }
        let ops = block
            .ops
            .iter()
            .enumerate()
            .flat_map(|(index, op)| replacements.get(&index).cloned().unwrap_or_else(|| vec![op.clone()]))
            .collect();
        changed.push(MirBlock { ops, ..block.clone() });
    }
    // These runtime arithmetic helpers do not consume incoming flags. The
    // original call nodes conservatively carried a flag dependency anyway.
    let blocks: Vec<MirBlock> = changed
        .into_iter()
        .map(|block| MirBlock {
            phis: block.phis.iter().filter(|phi| readers.contains(&phi.result)).cloned().collect(),
            ops: block
                .ops
                .iter()
                .map(|op| {
                    let helper = found.calls.get(&op.at).map(String::as_str);
                    if op.kind == Kind::Call
                        && helper.is_some_and(|name| [calls::MULTIPLY, calls::DIVIDE, calls::REMAINDER].contains(&name))
                    {
                        let mut made = op.clone();
                        made.uses = op.uses.iter().copied().filter(|value| !removed_flags.contains(value)).collect();
                        made
                    } else {
                        op.clone()
                    }
                })
                .collect(),
            ..block
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "raising_calls_tests.rs"]
mod tests;
