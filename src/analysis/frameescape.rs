//! Port of `qbopt/analysis/frameescape.py`.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use crate::model::mir::{Arg, Kind, MirBody, Op, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escapes {
    pub origins: IndexMap<Value, BTreeSet<i64>>,
    pub exposed: BTreeSet<i64>,
    pub opaque_addresses: BTreeSet<i64>,
    // The bytes the exposed addresses reach, or None where one is not bounded to its object.
    pub reach: Option<BTreeSet<(i64, i64)>>,
}

pub fn analysed(body: &MirBody) -> Escapes {
    // Origins are not allocation bounds. Absence here says nothing about
    // runtime frame walking, callbacks, or pointers loaded from memory.
    let mut origins: IndexMap<Value, BTreeSet<i64>> = IndexMap::default();
    let operations: Vec<&Op> = body.blocks.iter().flat_map(|block| &block.ops).collect();
    let phis: Vec<_> = body.blocks.iter().flat_map(|block| &block.phis).collect();

    let inputs = |op: &Op, origins: &IndexMap<Value, BTreeSet<i64>>| -> BTreeSet<i64> {
        let mut direct: BTreeSet<i64> = op
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
            direct.extend(origins.get(&value).into_iter().flatten().copied());
        }
        direct
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
            if !matches!(op.kind, Kind::Copy | Kind::Address)
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
            {
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

    let exposed: BTreeSet<i64> = operations
        .iter()
        .filter(|op| {
            !matches!(op.kind, Kind::Copy | Kind::Address)
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
        })
        .flat_map(|op| inputs(op, &origins))
        .collect();
    let opaque: BTreeSet<i64> = operations
        .iter()
        .filter(|op| op.kind == Kind::Address && op.args.iter().any(|arg| matches!(arg, Arg::Opaque(_))))
        .map(|op| op.at)
        .collect();
    let mut extents: IndexMap<i64, BTreeSet<Option<(i64, i64)>>> = IndexMap::default();
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

/// `side`'s three answers.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Number,
    Address,
    Unknown,
}

/// `moved`'s answers: extents, None where it cannot, `...` while unknown.
enum Moved {
    Extents(BTreeSet<(i64, i64)>),
    None,
    Ellipsis,
}

/// Values that hold an address inside frame objects of known extent on every path.
///
/// C's pointer arithmetic stays inside its object, so an address the body took
/// of a local, moved by an integer, still reaches only that local's bytes.
pub fn framed(body: &MirBody) -> IndexMap<Value, BTreeSet<(i64, i64)>> {
    let ops: Vec<&Op> = body.blocks.iter().flat_map(|block| &block.ops).collect();
    let phis: Vec<_> = body.blocks.iter().flat_map(|block| &block.phis).collect();
    let mut defined: BTreeSet<Value> = phis.iter().map(|phi| phi.result).collect();
    defined.extend(ops.iter().flat_map(|op| op.defines.iter().copied()));
    let mut moving: IndexMap<Value, &Op> = IndexMap::default();
    let mut refuted: BTreeSet<Value> = BTreeSet::new();
    for op in &ops {
        let held = op.results.len() == 1 && matches!(&op.results[0], Arg::Held(one) if one.width == 2);
        let pure = !(!op.loads.is_empty() || !op.stores.is_empty() || op.barrier());
        let kinds = [Kind::Address, Kind::Copy, Kind::Add, Kind::Sub];
        for value in &op.defines {
            if held
                && pure
                && kinds.contains(&op.kind)
                && matches!(&op.results[0], Arg::Held(one) if one.value == *value)
            {
                moving.insert(*value, op);
            } else {
                refuted.insert(*value);
            }
        }
    }
    refuted.extend(phis.iter().flat_map(|phi| phi.incoming.values()).filter(|one| !defined.contains(one)).copied());
    let mut state: IndexMap<Value, BTreeSet<(i64, i64)>> = IndexMap::default();

    let side = |arg: &Arg, state: &IndexMap<Value, BTreeSet<(i64, i64)>>, refuted: &BTreeSet<Value>| match arg {
        Arg::Const(_) => Side::Number,
        Arg::Held(held) if refuted.contains(&held.value) => Side::Number,
        Arg::Held(held) if state.contains_key(&held.value) => Side::Address,
        _ => Side::Unknown,
    };
    // The extents `op` leaves its result in, None where it cannot, `...` while unknown.
    let moved = |op: &Op,
                 _final: bool,
                 state: &IndexMap<Value, BTreeSet<(i64, i64)>>,
                 refuted: &BTreeSet<Value>|
     -> Moved {
        let value = |arg: &Arg| match arg {
            Arg::Held(held) => held.value,
            _ => unreachable!("an address side is a held value"),
        };
        let unknown = |sides: [Side; 2]| if sides.contains(&Side::Unknown) { Moved::Ellipsis } else { Moved::None };
        let (kind, source) = match (op.kind, &op.args[..]) {
            (Kind::Address, [Arg::FrameAddress(address)]) if address.extent.is_some() => {
                return Moved::Extents(BTreeSet::from([address.extent.expect("guarded")]));
            }
            (Kind::Copy, [source @ Arg::Held(_)]) => (side(source, state, refuted), source),
            (Kind::Add, [left, right]) => {
                let sides = [side(left, state, refuted), side(right, state, refuted)];
                return match sides {
                    [Side::Address, Side::Number] => Moved::Extents(state[&value(left)].clone()),
                    [Side::Number, Side::Address] => Moved::Extents(state[&value(right)].clone()),
                    _ => unknown(sides),
                };
            }
            (Kind::Sub, [left, right]) => {
                let sides = [side(left, state, refuted), side(right, state, refuted)];
                return match sides {
                    [Side::Address, Side::Number] => Moved::Extents(state[&value(left)].clone()),
                    _ => unknown(sides),
                };
            }
            _ => return Moved::None,
        };
        match kind {
            Side::Address => Moved::Extents(state[&value(source)].clone()),
            Side::Number => Moved::None,
            Side::Unknown => Moved::Ellipsis,
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
                let union: BTreeSet<(i64, i64)> =
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
                match moved(op, false, &state, &refuted) {
                    Moved::None => {
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
                    Moved::Ellipsis => {}
                }
            }
        }
        // Optimism settles cycles; anything still unproven is refuted and the rest looked at again.
        let unproven: BTreeSet<Value> = moving
            .keys()
            .copied()
            .chain(phis.iter().map(|phi| phi.result))
            .filter(|value| {
                !refuted.contains(value)
                    && (!state.contains_key(value)
                        || moving
                            .get(value)
                            .is_some_and(|op| !matches!(moved(op, true, &state, &refuted), Moved::Extents(_)))
                        || (!moving.contains_key(value)
                            && phis
                                .iter()
                                .filter(|phi| phi.result == *value)
                                .any(|phi| phi.incoming.values().any(|one| !state.contains_key(one)))))
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
    //! Port of tests/test_frame_escape.py.
    //!
    //! `test_renderer_exposes_temporary_string_not_counter_address` waits for
    //! `mir.bodies` and the corpus loader.

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
