//! Port of `qbopt/analysis/frameescape.py`.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{Arg, Kind, MirBody, Op, Value};

pub type Extent = (i64, i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escapes {
    pub origins: BTreeMap<Value, BTreeSet<i64>>,
    pub exposed: BTreeSet<i64>,
    pub opaque_addresses: BTreeSet<i64>,
    /// The bytes the exposed addresses reach, or None where one is not bounded to its object.
    pub reach: Option<BTreeSet<Extent>>,
}

pub fn analysed(body: &MirBody) -> Escapes {
    // Origins are not allocation bounds. Absence here says nothing about
    // runtime frame walking, callbacks, or pointers loaded from memory.
    let mut origins: BTreeMap<Value, BTreeSet<i64>> = BTreeMap::new();
    let operations: Vec<&Op> = body.blocks.iter().flat_map(|block| &block.ops).collect();
    let phis: Vec<_> = body.blocks.iter().flat_map(|block| &block.phis).collect();

    let inputs = |op: &Op, origins: &BTreeMap<Value, BTreeSet<i64>>| -> BTreeSet<i64> {
        let mut out: BTreeSet<i64> = op
            .args
            .iter()
            .filter_map(|arg| match arg {
                Arg::FrameAddress(address) => Some(address.offset),
                _ => None,
            })
            .collect();
        let mut values: BTreeSet<Value> = op.uses.iter().copied().collect();
        values.extend(op.args.iter().filter_map(|arg| match arg {
            Arg::Held(held) => Some(held.value),
            _ => None,
        }));
        values.extend(op.loads.iter().chain(&op.stores).flat_map(|one| [one.base, one.segment]).flatten());
        for value in values {
            out.extend(origins.get(&value).into_iter().flatten().copied());
        }
        out
    };
    let copies = |op: &Op| {
        matches!(op.kind, Kind::Copy | Kind::Address) && op.loads.is_empty() && op.stores.is_empty() && !op.barrier()
    };

    loop {
        let mut changed = false;
        for phi in &phis {
            let incoming: BTreeSet<i64> =
                phi.incoming.values().flat_map(|value| origins.get(value).into_iter().flatten().copied()).collect();
            let previous = origins.get(&phi.result).cloned().unwrap_or_default();
            if !incoming.is_subset(&previous) {
                origins.insert(phi.result, previous.union(&incoming).copied().collect());
                changed = true;
            }
        }
        for op in &operations {
            if !copies(op) {
                continue;
            }
            let incoming = inputs(op, &origins);
            for value in &op.defines {
                if value.flags {
                    continue;
                }
                let previous = origins.get(value).cloned().unwrap_or_default();
                if !incoming.is_subset(&previous) {
                    origins.insert(*value, previous.union(&incoming).copied().collect());
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let exposed: BTreeSet<i64> =
        operations.iter().filter(|op| !copies(op)).flat_map(|op| inputs(op, &origins)).collect();
    let opaque: BTreeSet<i64> = operations
        .iter()
        .filter(|op| op.kind == Kind::Address && op.args.iter().any(|arg| matches!(arg, Arg::Opaque(_))))
        .map(|op| op.at)
        .collect();
    let mut extents: BTreeMap<i64, BTreeSet<Option<Extent>>> = BTreeMap::new();
    for op in &operations {
        for arg in &op.args {
            if let Arg::FrameAddress(address) = arg {
                extents.entry(address.offset).or_default().insert(address.extent);
            }
        }
    }
    let mut reach = Some(BTreeSet::new());
    for offset in &exposed {
        let found = extents.get(offset).cloned().unwrap_or_else(|| BTreeSet::from([None]));
        if found.contains(&None) {
            reach = None;
            break;
        }
        if let Some(reach) = reach.as_mut() {
            reach.extend(found.into_iter().flatten());
        }
    }
    Escapes { origins, exposed, opaque_addresses: opaque, reach }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Number,
    Address,
    Unknown,
}

/// What `moved` answers: extents, None where it cannot, `...` while unknown.
enum Moved {
    Extents(BTreeSet<Extent>),
    Refuted,
    Pending,
}

/// Values that hold an address inside frame objects of known extent on every path.
pub fn framed(body: &MirBody) -> BTreeMap<Value, BTreeSet<Extent>> {
    let ops: Vec<&Op> = body.blocks.iter().flat_map(|block| &block.ops).collect();
    let phis: Vec<_> = body.blocks.iter().flat_map(|block| &block.phis).collect();
    let mut defined: BTreeSet<Value> = phis.iter().map(|phi| phi.result).collect();
    defined.extend(ops.iter().flat_map(|op| op.defines.iter().copied()));
    // Python dict: insertion order, the last definition winning.
    let mut moving: Vec<(Value, &Op)> = Vec::new();
    let mut refuted: BTreeSet<Value> = BTreeSet::new();
    for op in &ops {
        let held = op.results.len() == 1 && matches!(&op.results[0], Arg::Held(one) if one.width == 2);
        let pure = !(!op.loads.is_empty() || !op.stores.is_empty() || op.barrier());
        let kinds = matches!(op.kind, Kind::Address | Kind::Copy | Kind::Add | Kind::Sub);
        for value in &op.defines {
            let result = matches!(&op.results[..], [Arg::Held(one)] if one.value == *value);
            if held && pure && kinds && result {
                match moving.iter_mut().find(|(one, _)| one == value) {
                    Some(entry) => entry.1 = op,
                    None => moving.push((*value, op)),
                }
            } else {
                refuted.insert(*value);
            }
        }
    }
    for phi in &phis {
        refuted.extend(phi.incoming.values().filter(|one| !defined.contains(one)).copied());
    }
    let mut state: BTreeMap<Value, BTreeSet<Extent>> = BTreeMap::new();

    let side = |arg: &Arg, state: &BTreeMap<Value, BTreeSet<Extent>>, refuted: &BTreeSet<Value>| match arg {
        Arg::Const(_) => Side::Number,
        Arg::Held(held) if refuted.contains(&held.value) => Side::Number,
        Arg::Held(held) if state.contains_key(&held.value) => Side::Address,
        _ => Side::Unknown,
    };
    let moved = |op: &Op, state: &BTreeMap<Value, BTreeSet<Extent>>, refuted: &BTreeSet<Value>| -> Moved {
        let value_of = |arg: &Arg| match arg {
            Arg::Held(held) => held.value,
            _ => unreachable!("an address side is a held value"),
        };
        let unknown = |sides: [Side; 2]| if sides.contains(&Side::Unknown) { Moved::Pending } else { Moved::Refuted };
        match (op.kind, &op.args[..]) {
            (Kind::Address, [Arg::FrameAddress(address)]) if address.extent.is_some() => {
                Moved::Extents(BTreeSet::from([address.extent.unwrap()]))
            }
            (Kind::Copy, [source @ Arg::Held(_)]) => match side(source, state, refuted) {
                Side::Address => Moved::Extents(state[&value_of(source)].clone()),
                Side::Number => Moved::Refuted,
                Side::Unknown => Moved::Pending,
            },
            (Kind::Add, [left, right]) => {
                let sides = [side(left, state, refuted), side(right, state, refuted)];
                match sides {
                    [Side::Address, Side::Number] => Moved::Extents(state[&value_of(left)].clone()),
                    [Side::Number, Side::Address] => Moved::Extents(state[&value_of(right)].clone()),
                    _ => unknown(sides),
                }
            }
            (Kind::Sub, [left, right]) => {
                let sides = [side(left, state, refuted), side(right, state, refuted)];
                match sides {
                    [Side::Address, Side::Number] => Moved::Extents(state[&value_of(left)].clone()),
                    _ => unknown(sides),
                }
            }
            _ => Moved::Refuted,
        }
    };

    loop {
        let mut changed = true;
        while changed {
            changed = false;
            for phi in &phis {
                if refuted.contains(&phi.result) {
                    continue;
                }
                if phi.incoming.values().any(|one| refuted.contains(one)) {
                    refuted.insert(phi.result);
                    changed = true;
                    continue;
                }
                let union: BTreeSet<Extent> =
                    phi.incoming.values().flat_map(|one| state.get(one).into_iter().flatten().copied()).collect();
                let previous = state.get(&phi.result).cloned().unwrap_or_default();
                if !union.is_subset(&previous) {
                    state.insert(phi.result, previous.union(&union).copied().collect());
                    changed = true;
                }
            }
            for (value, op) in &moving {
                if refuted.contains(value) {
                    continue;
                }
                match moved(op, &state, &refuted) {
                    Moved::Refuted => {
                        refuted.insert(*value);
                        changed = true;
                    }
                    Moved::Extents(got) => {
                        let previous = state.get(value).cloned().unwrap_or_default();
                        if !got.is_subset(&previous) {
                            state.insert(*value, previous.union(&got).copied().collect());
                            changed = true;
                        }
                    }
                    Moved::Pending => {}
                }
            }
        }
        // Optimism settles cycles; anything still unproven is refuted and the rest looked at again.
        let candidates = moving.iter().map(|(value, _)| *value).chain(phis.iter().map(|phi| phi.result));
        let unproven: BTreeSet<Value> = candidates
            .filter(|value| {
                if refuted.contains(value) {
                    return false;
                }
                let moved_by = moving.iter().find(|(one, _)| one == value).map(|(_, op)| *op);
                !state.contains_key(value)
                    || moved_by.is_some_and(|op| !matches!(moved(op, &state, &refuted), Moved::Extents(_)))
                    || (moved_by.is_none()
                        && phis
                            .iter()
                            .filter(|phi| phi.result == *value)
                            .any(|phi| phi.incoming.values().any(|one| !state.contains_key(one))))
            })
            .collect();
        if unproven.is_empty() {
            return state.into_iter().filter(|(value, _)| !refuted.contains(value)).collect();
        }
        refuted.extend(unproven);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::Operation;
    use crate::model::mir::{FrameAddress, Held, MirBlock, OpCode, Opaque, Phi};

    fn op(at: i64, operation: Operation, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
        let mut op = Op::new(at, OpCode::Operation(operation), "", defines, uses);
        op.kind = kind;
        op
    }

    #[test]
    fn test_frame_origin_reaches_use_through_copy_and_loop_phi() {
        for sink in [Kind::Call, Kind::Store, Kind::Return, Kind::Add] {
            let [root, joined, copied] = [1, 2, 3].map(|index| Value::new(index, i64::from(index)));
            let mut address = op(0, Operation::Address, vec![root], vec![], Kind::Address);
            address.args = vec![Arg::FrameAddress(FrameAddress::new(-32, 2))];
            address.results = vec![Arg::Held(Held { value: root, width: 2 })];
            let mut copy = op(2, Operation::Move, vec![copied], vec![joined], Kind::Copy);
            copy.args = vec![Arg::Held(Held { value: joined, width: 2 })];
            copy.results = vec![Arg::Held(Held { value: copied, width: 2 })];
            let mut used = op(3, Operation::Nothing, vec![], vec![copied], sink);
            used.args = vec![Arg::Held(Held { value: copied, width: 2 })];
            let phi = Phi { result: joined, incoming: [(0, root), (1, copied)].into_iter().collect() };
            let body = MirBody::new(
                0,
                vec![
                    MirBlock::new(0, vec![], vec![address], vec![1]),
                    MirBlock::new(1, vec![phi], vec![copy, used], vec![1]),
                ],
            );
            let result = analysed(&body);
            assert_eq!(result.origins[&copied], BTreeSet::from([-32]), "{sink}");
            assert_eq!(result.exposed, BTreeSet::from([-32]), "{sink}");
        }
    }

    #[test]
    fn test_opaque_address_is_not_an_empty_escape_proof() {
        let mut address = op(8, Operation::Address, vec![], vec![], Kind::Address);
        address.args = vec![Arg::Opaque(Opaque::new(None))];
        let result = analysed(&MirBody::new(8, vec![MirBlock::new(8, vec![], vec![address], vec![])]));
        assert_eq!(result.opaque_addresses, BTreeSet::from([8]));
    }
}
