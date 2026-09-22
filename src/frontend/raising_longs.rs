//! Port of `qbopt/frontend/raising_longs.py`.
//!
//! Recognize adjacent BC word pairs as whole scalar loads and arithmetic.

use std::borrow::{Borrow, Cow};
use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use crate::analysis::liveness;
use crate::backend::lower::Unlowered;
use crate::frontend::pairs;
use crate::support::hash::{HashMap, HashSet, IndexSet};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, Op, OpCode, OrderedMap, RaisedBody, Synth, Value};
use crate::objectfile::module::Space;

/// Expose the sign word as an extraction from a signed whole value.
pub fn sign_fills(body: RaisedBody) -> RaisedBody {
    let mut values: BTreeSet<Value> = body.origin.keys().copied().collect();
    for block in &body.blocks {
        values.extend(block.ops.iter().flat_map(|op| op.uses.iter().chain(&op.defines).copied()));
        values.extend(block.phis.iter().map(|phi| phi.result));
        values.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
    }
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let (Some(Arg::Held(arg)), Some(Arg::Held(result))) = (op.args.first(), op.results.first()) else {
                ops.push(op.clone());
                continue;
            };
            if op.kind != Kind::Convert
                || op.op != Some(OpCode::Operation(Operation::Extend))
                || op.name != "cwd"
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
                || op.args.len() != 1
                || op.results.len() != 1
                || arg.width != 2
                || result.width != 2
                || op.defines != [result.value]
            {
                ops.push(op.clone());
                continue;
            }
            serial += 1;
            variable += 1;
            let whole = Held { value: Value { id: serial, at: op.at, flags: false, variable, version: 1 }, width: 4 };
            let mut extend = Op::new(
                op.at,
                OpCode::Operation(Operation::Extend),
                "sign_extend",
                vec![whole.value],
                vec![arg.value],
            );
            extend.kind = Kind::SignExtend;
            extend.args = op.args.clone();
            extend.results = vec![Arg::Held(whole)];
            ops.push(extend);
            let mut extract = op.clone();
            extract.kind = Kind::Extract;
            extract.op = Some(OpCode::Synth(Synth::HalfToLow));
            extract.name = "extract".to_owned();
            extract.args = vec![Arg::Held(whole), Arg::Const(Const::new(16, 4))];
            extract.uses = vec![whole.value];
            extract.merges = OrderedMap::new();
            extract.raised = None;
            ops.push(mir::detached(extract));
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

/// Rejoin high/low argument words extracted from the same whole value.
pub fn arguments(body: RaisedBody) -> RaisedBody {
    let definitions: BTreeMap<Value, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect();
    let word = |op: &Op| {
        op.kind == Kind::Arg
            && op.defines.is_empty()
            && op.loads.is_empty()
            && !op.barrier()
            && op.merges.is_empty()
            && op.stack.is_none()
            && op.args.len() == 1
            && op.stores.len() == 1
            && matches!(&op.args[0], Arg::Held(held) if held.width == 2)
            && op.stores[0] == MemRef { space: Some(Space::Stack), ..MemRef::new(None, 2) }
    };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops: Vec<Op> = Vec::new();
        for low in &block.ops {
            if let Some(high) = ops.last() {
                if mir::raising_adjacent(high, low) && word(high) && word(low) {
                    if let Some(source) = mir::extracted_whole(&high.args[0], &low.args[0], &definitions) {
                        let mut changed = high.clone();
                        changed.args = vec![Arg::Held(source)];
                        changed.uses = vec![source.value];
                        changed.stores = vec![MemRef { width: 4, ..high.stores[0].clone() }];
                        changed.raised = None;
                        let made = mir::raising_owned(mir::detached(changed), &[high, low]);
                        *ops.last_mut().expect("high") = made;
                        continue;
                    }
                }
            }
            ops.push(low.clone());
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

/// BC negates a long with NEG low, ADC high,0, NEG high.
pub fn _negated_whole<D: Borrow<Op>>(high: &Arg, low: &Arg, definitions: &BTreeMap<Value, D>) -> Option<Held> {
    let (Arg::Held(high), Arg::Held(low)) = (high, low) else {
        return None;
    };
    if high.width != 2 || low.width != 2 {
        return None;
    }
    let get = |value: &Value| definitions.get(value).map(Borrow::<Op>::borrow);
    let (lower, upper) = (get(&low.value), get(&high.value));
    let negate = |op: Option<&Op>| {
        op.is_some_and(|op| {
            op.kind == Kind::Neg
                && op.args.len() == 1
                && op.loads.is_empty()
                && op.stores.is_empty()
                && !op.barrier()
                && !mir::partial(op)
        })
    };
    if !negate(lower) || !negate(upper) {
        return None;
    }
    let (lower, upper) = (lower?, upper?);
    if lower.results != [Arg::Held(*low)] || upper.results != [Arg::Held(*high)] {
        return None;
    }
    let Arg::Held(carried) = &upper.args[0] else {
        return None;
    };
    if carried.width != 2 {
        return None;
    }
    let carry = get(&carried.value)?;
    if carry.kind != Kind::AddCarry
        || !carry.loads.is_empty()
        || !carry.stores.is_empty()
        || carry.barrier()
        || mir::partial(carry)
        || carry.results != [Arg::Held(*carried)]
        || carry.args.len() != 2
        || carry.args[1] != Arg::Const(Const::new(0, 2))
    {
        return None;
    }
    let flags: BTreeSet<Value> = lower.defines.iter().copied().filter(|value| value.flags).collect();
    let used: BTreeSet<Value> = carry.uses.iter().copied().filter(|value| value.flags).collect();
    if flags.len() != 1 || used != flags {
        return None;
    }
    mir::extracted_whole(&carry.args[0], &lower.args[0], definitions)
}

pub fn _constant_stores(body: RaisedBody) -> RaisedBody {
    let word = |one: &Op| {
        one.kind == Kind::Store
            && !one.barrier()
            && one.defines.is_empty()
            && one.loads.is_empty()
            && one.stores.len() == 1
            && one.args.len() == 1
            && matches!(&one.args[0], Arg::Const(constant) if constant.width == 2)
            && one.stores[0].width == 2
    };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops: Vec<Op> = Vec::new();
        for op in &block.ops {
            if let Some(low) = ops.last() {
                if word(low)
                    && word(op)
                    && mir::raising_adjacent(low, op)
                    && low.stores[0].addr.is_some_and(|addr| {
                        MemRef { addr: Some(addr.plus(2)), ..low.stores[0].clone() } == op.stores[0]
                    })
                {
                    let r#ref = MemRef { width: 4, ..low.stores[0].clone() };
                    let (Arg::Const(high_word), Arg::Const(low_word)) = (&op.args[0], &low.args[0]) else {
                        unreachable!("word() checked both")
                    };
                    let mask = BigInt::from(0xFFFF);
                    let value = Const::new(((&high_word.n & &mask) << 16) | (&low_word.n & &mask), 4);
                    let mut changed = low.clone();
                    changed.args = vec![Arg::Const(value)];
                    changed.results = vec![Arg::Cell(Cell { r#ref: r#ref.clone() })];
                    changed.stores = vec![r#ref];
                    changed.uses = low.uses.iter().chain(&op.uses).copied().collect::<IndexSet<_>>().into_iter().collect();
                    changed.raised = None;
                    let made = mir::raising_owned(changed, &[low, op]);
                    *ops.last_mut().expect("low") = made;
                    continue;
                }
            }
            ops.push(op.clone());
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

/// Recognize whole NOT and NEG before their word results cross an ABI.
pub fn unary(body: RaisedBody) -> RaisedBody {
    let mut definitions: BTreeMap<Value, Cow<'_, Op>> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, Cow::Borrowed(op))))
        .collect();
    let mut values: BTreeSet<Value> = body.origin.keys().copied().collect();
    values.extend(definitions.keys().copied());
    for block in &body.blocks {
        values.extend(block.ops.iter().flat_map(|op| op.uses.iter().copied()));
        values.extend(block.phis.iter().map(|phi| phi.result));
        values.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
    }
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    // Python's `id(op)`: the operation's (block, index) position.
    let mut users: HashMap<Value, HashSet<(usize, usize)>> = HashMap::default();
    for (at, block) in body.blocks.iter().enumerate() {
        for (index, op) in block.ops.iter().enumerate() {
            for value in &op.uses {
                users.entry(*value).or_default().insert((at, index));
            }
        }
    }
    let phi_reads = liveness::phi_inputs(&body, None);
    let mut blocks = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        let (mut ops, mut index) = (Vec::new(), 0);
        while index < block.ops.len() {
            let low = &block.ops[index];
            let count = if low.kind == Kind::Not { 2 } else { 3 };
            let group = &block.ops[index..(index + count).min(block.ops.len())];
            let mut source = None;
            if matches!(low.kind, Kind::Not | Kind::Neg)
                && group.len() == count
                && group.iter().all(|op| {
                    op.loads.is_empty()
                        && op.stores.is_empty()
                        && !op.barrier()
                        && !mir::partial(op)
                        && op.results.len() == 1
                        && matches!(&op.results[0], Arg::Held(held) if held.width == 2)
                })
                && group.windows(2).all(|two| mir::raising_adjacent(&two[0], &two[1]))
            {
                let high = &group[group.len() - 1];
                if low.kind == Kind::Not && high.kind == Kind::Not && low.args.len() == 1 && high.args.len() == 1 {
                    source = mir::extracted_whole(&high.args[0], &low.args[0], &definitions);
                } else if low.kind == Kind::Neg {
                    source = _negated_whole(&high.results[0], &low.results[0], &definitions);
                }
                let internal: HashSet<(usize, usize)> = (index..index + count).map(|one| (at, one)).collect();
                let (Arg::Held(first), Arg::Held(last)) = (&low.results[0], &high.results[0]) else {
                    unreachable!("checked Held results")
                };
                let kept = [first.value, last.value];
                let observed = group
                    .iter()
                    .flat_map(|op| op.defines.iter())
                    .filter(|value| !kept.contains(value))
                    .any(|value| {
                        phi_reads.contains(value)
                            || users.get(value).is_some_and(|found| !found.is_subset(&internal))
                    });
                if observed {
                    source = None;
                }
            }
            let Some(source) = source else {
                ops.push(low.clone());
                index += 1;
                continue;
            };
            let high = &group[group.len() - 1];
            serial += 1;
            variable += 1;
            let result = Held { value: Value { id: serial, at: low.at, flags: false, variable, version: 1 }, width: 4 };
            let mut whole = Op::new(
                low.at,
                OpCode::Operation(Operation::Unary),
                low.kind.as_str(),
                vec![result.value],
                vec![source.value],
            );
            whole.kind = low.kind;
            whole.args = vec![Arg::Held(source)];
            whole.results = vec![Arg::Held(result)];
            let owners: Vec<&Op> = group.iter().collect();
            ops.push(mir::raising_owned(whole, &owners));
            for (half, offset) in [(low, 0), (high, 16)] {
                let Arg::Held(word) = &half.results[0] else { unreachable!("a checked Held result") };
                let mut extract = Op::new(
                    high.at,
                    OpCode::Synth(Synth::HalfToLow),
                    "extract",
                    vec![word.value],
                    vec![result.value],
                );
                extract.kind = Kind::Extract;
                extract.args = vec![Arg::Held(result), Arg::Const(Const::new(offset, 4))];
                extract.results = half.results.clone();
                definitions.insert(word.value, Cow::Owned(extract.clone()));
                ops.push(extract);
            }
            index += count;
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

pub fn scalar(body: RaisedBody) -> Result<RaisedBody, Unlowered> {
    let body = _constant_stores(body);
    // Python's `id(op)`: identity of an operation in this one body.
    let id = |op: &Op| std::ptr::from_ref(op);
    let found = pairs::found(&body)?;
    let mut candidates: HashMap<*const Op, pairs::Pair<'_>> = HashMap::default();
    for pair in found {
        if matches!(pair.kind, pairs::Kind::Load | pairs::Kind::Alu | pairs::Kind::AluImm | pairs::Kind::Store) {
            candidates.insert(id(pair.first()), pair);
        }
    }
    let mut definitions: BTreeMap<Value, Cow<'_, Op>> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, Cow::Borrowed(op))))
        .collect();
    let mut values: BTreeSet<Value> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.uses.iter().chain(&op.defines).copied())
        .collect();
    values.extend(body.origin.keys().copied());
    for block in &body.blocks {
        values.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
        values.extend(block.phis.iter().map(|phi| phi.result));
    }
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut readers: BTreeSet<Value> = BTreeSet::new();
    let mut users: HashMap<Value, HashSet<*const Op>> = HashMap::default();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for value in op.uses.iter().filter(|value| !op.merges.contains_key(value)) {
            readers.insert(*value);
            if value.flags {
                users.entry(*value).or_default().insert(id(op));
            }
        }
    }
    // A word-pair ALU operation leaves the high-word condition codes, while
    // the scalar form would leave the whole-value condition codes.  Direct
    // users are not the whole observation: a condition can cross a CFG or
    // machine-exit edge, so derive every operation's live flags from ordinary
    // MIR liveness before recognizing a pair.
    let live = liveness::live(&body);
    let phi_reads = liveness::phi_inputs(&body, Some(&live));
    readers.extend(phi_reads.iter().copied());
    let mut flags_after: HashMap<*const Op, BTreeSet<Value>> = HashMap::default();
    for block in &body.blocks {
        let mut alive: BTreeSet<Value> = live.live_out[&block.at].iter().copied().filter(|value| value.flags).collect();
        for op in block.ops.iter().rev() {
            flags_after.insert(id(op), alive.clone());
            for value in &op.defines {
                alive.remove(value);
            }
            alive.extend(op.uses.iter().copied().filter(|value| value.flags));
        }
    }
    let wide_reads: BTreeSet<Value> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.args)
        .filter_map(|arg| match arg {
            Arg::Held(held) if held.width > 2 => Some(held.value),
            _ => None,
        })
        .collect();

    let mut fresh = |at: i64| {
        serial += 1;
        variable += 1;
        Held { value: Value { id: serial, at: at, flags: false, variable, version: 1 }, width: 4 }
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut whole: HashMap<(Arg, Arg), Held> = HashMap::default();
        let mut dropped: HashSet<*const Op> = HashSet::default();
        let mut ops: Vec<Op> = Vec::new();
        for op in &block.ops {
            if dropped.contains(&id(op)) {
                continue;
            }
            let Some(pair) = candidates.get(&id(op)) else {
                ops.push(op.clone());
                continue;
            };
            let (low, high) = (pair.low, pair.high);
            let stores = pair.kind == pairs::Kind::Store;
            let immediate = pair.kind == pairs::Kind::AluImm;
            let refs = if stores { (&low.stores, &high.stores) } else { (&low.loads, &high.loads) };
            if low.barrier() || high.barrier() {
                ops.push(op.clone());
                continue;
            }
            if !immediate
                && (refs.0.len() != 1
                    || refs.1.len() != 1
                    || refs.0[0].addr.is_none_or(|addr| {
                        MemRef { addr: Some(addr.plus(2)), ..refs.0[0].clone() } != refs.1[0]
                    }))
            {
                ops.push(op.clone());
                continue;
            }
            let r#ref = if immediate { None } else { Some(MemRef { width: 4, ..refs.0[0].clone() }) };
            let one_each = low.args.len() == 1 && high.args.len() == 1;
            let (args, results, kind);
            if stores {
                let mut source =
                    if one_each { whole.get(&(high.args[0].clone(), low.args[0].clone())).copied() } else { None };
                if source.is_none() && one_each {
                    source = mir::extracted_whole(&high.args[0], &low.args[0], &definitions);
                }
                if source.is_none() && one_each {
                    if let Some(original) = _negated_whole(&high.args[0], &low.args[0], &definitions) {
                        let made = fresh(low.at);
                        let mut negate = Op::new(
                            low.at,
                            OpCode::Operation(Operation::Unary),
                            "neg",
                            vec![made.value],
                            vec![original.value],
                        );
                        negate.kind = Kind::Neg;
                        negate.args = vec![Arg::Held(original)];
                        negate.results = vec![Arg::Held(made)];
                        ops.push(negate);
                        source = Some(made);
                    }
                }
                if source.is_none() && one_each {
                    let (upper, lower) = (&high.args[0], &low.args[0]);
                    let extension = match upper {
                        Arg::Held(upper) => definitions.get(&upper.value).map(|op| &**op),
                        _ => None,
                    };
                    if let (Arg::Held(lower_held), Arg::Held(upper_held), Some(extension)) = (lower, upper, extension) {
                        if lower_held.width == 2
                            && extension.kind == Kind::Convert
                            && extension.op == Some(OpCode::Operation(Operation::Extend))
                            && extension.name == "cwd"
                            && extension.args == [lower.clone()]
                            && extension.results == [upper.clone()]
                            && upper_held.width == 2
                            && extension.loads.is_empty()
                            && extension.stores.is_empty()
                        {
                            let made = fresh(low.at);
                            let mut extend = Op::new(
                                low.at,
                                OpCode::Operation(Operation::Extend),
                                "sign_extend",
                                vec![made.value],
                                vec![lower_held.value],
                            );
                            extend.kind = Kind::SignExtend;
                            extend.args = vec![lower.clone()];
                            extend.results = vec![Arg::Held(made)];
                            ops.push(extend);
                            source = Some(made);
                        }
                    }
                }
                let Some(source) = source else {
                    ops.push(op.clone());
                    continue;
                };
                args = vec![Arg::Held(source)];
                results = vec![Arg::Cell(Cell { r#ref: r#ref.clone().expect("a store pair has memory") })];
                kind = Kind::Store;
            } else {
                let unfit = |result: &Arg| match result {
                    Arg::Held(held) => held.width != 2 || wide_reads.contains(&held.value),
                    _ => true,
                };
                let observed_low = |value: &Value| {
                    value.flags
                        && (phi_reads.contains(value)
                            || users.get(value).is_some_and(|found| found.iter().any(|one| *one != id(high))))
                };
                if low.results.len() != 1
                    || high.results.len() != 1
                    || low.results.iter().chain(&high.results).any(unfit)
                    || high.defines.iter().any(|value| value.flags && readers.contains(value))
                    || low.defines.iter().any(observed_low)
                    || !flags_after[&id(high)].is_empty()
                {
                    ops.push(op.clone());
                    continue;
                }
                if pair.kind == pairs::Kind::Load {
                    args = vec![Arg::Cell(Cell { r#ref: r#ref.clone().expect("a load pair has memory") })];
                    kind = Kind::Load;
                } else {
                    let source = whole
                        .get(&(high.args[0].clone(), low.args[0].clone()))
                        .copied()
                        .or_else(|| mir::extracted_whole(&high.args[0], &low.args[0], &definitions));
                    let Some(source) = source.filter(|_| {
                        matches!(low.kind, Kind::Add | Kind::Sub | Kind::And | Kind::Or | Kind::Xor)
                    }) else {
                        ops.push(op.clone());
                        continue;
                    };
                    let operand = if immediate {
                        let (upper, lower) = (&high.args[high.args.len() - 1], &low.args[low.args.len() - 1]);
                        let (Arg::Const(upper), Arg::Const(lower)) = (upper, lower) else {
                            ops.push(op.clone());
                            continue;
                        };
                        if upper.width != 2 || lower.width != 2 {
                            ops.push(op.clone());
                            continue;
                        }
                        let mask = BigInt::from(0xFFFF);
                        Arg::Const(Const::new(((&upper.n & &mask) << 16) | (&lower.n & &mask), 4))
                    } else {
                        Arg::Cell(Cell { r#ref: r#ref.clone().expect("a memory pair has memory") })
                    };
                    args = vec![Arg::Held(source), operand];
                    kind = low.kind;
                }
                results = vec![Arg::Held(fresh(low.at))];
            }
            let mut uses: Vec<Value> = args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect();
            if let Some(r#ref) = &r#ref {
                uses.extend([r#ref.base, r#ref.segment].into_iter().flatten());
            }
            let mut changed = low.clone();
            changed.kind = kind;
            changed.args = args.clone();
            changed.results = results.clone();
            changed.uses = uses.into_iter().collect::<IndexSet<_>>().into_iter().collect();
            changed.defines = match &results[0] {
                Arg::Held(result) if !stores => vec![result.value],
                _ => Vec::new(),
            };
            changed.loads = match &r#ref {
                Some(r#ref) if !stores => vec![r#ref.clone()],
                _ => Vec::new(),
            };
            changed.stores = if stores { vec![r#ref.clone().expect("a store pair has memory")] } else { Vec::new() };
            changed.merges = OrderedMap::new();
            changed.raised = None;
            let mut widened = mir::raising_owned(changed, &[low, high]);
            if pair.kind == pairs::Kind::Alu {
                let r#ref = r#ref.clone().expect("a memory pair has memory");
                let loaded = fresh(low.at);
                let mut load = widened.clone();
                load.op = Some(OpCode::Operation(Operation::Move));
                load.name = "mov".to_owned();
                load.kind = Kind::Load;
                load.args = vec![Arg::Cell(Cell { r#ref: r#ref.clone() })];
                load.results = vec![Arg::Held(loaded)];
                load.defines = vec![loaded.value];
                load.uses = [r#ref.base, r#ref.segment].into_iter().flatten().collect();
                load.symbol = Some(true);
                ops.push(load);
                let Arg::Held(source) = &args[0] else { unreachable!("an ALU pair's whole source") };
                let mut rest = widened;
                rest.args = vec![args[0].clone(), Arg::Held(loaded)];
                rest.uses = vec![source.value, loaded.value];
                rest.loads = Vec::new();
                rest.id = None;
                rest.symbol = Some(false);
                widened = mir::source_free(rest);
            }
            ops.push(widened);
            if !stores {
                let Arg::Held(result) = results[0] else { unreachable!("a fresh Held result") };
                whole.insert((high.results[0].clone(), low.results[0].clone()), result);
                for (half, offset) in [(low, 0), (high, 16)] {
                    let Arg::Held(word) = &half.results[0] else { unreachable!("a checked Held result") };
                    let mut extract = Op::new(
                        high.at,
                        OpCode::Synth(Synth::HalfToLow),
                        "extract",
                        vec![word.value],
                        vec![result.value],
                    );
                    extract.kind = Kind::Extract;
                    extract.args = vec![Arg::Held(result), Arg::Const(Const::new(offset, 4))];
                    extract.results = half.results.clone();
                    definitions.insert(word.value, Cow::Owned(extract.clone()));
                    ops.push(extract);
                }
            }
            dropped.insert(id(low));
            dropped.insert(id(high));
        }
        blocks.push(block.with_ops(ops));
    }
    Ok(body.with_blocks(blocks))
}
