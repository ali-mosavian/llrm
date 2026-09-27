//! Direct port of `qbopt/optimize/ivshare.py`.
//!
//! Tests: every test in `tests/test_ivshare.py` is skipped; its `culling`
//! fixture spies on `strength.reduced`, and errors in Python at this commit.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::consts::masked;
use crate::analysis::induction::{self, Affine, AffineOperand};
use crate::analysis::loops;
use crate::model::mir::{Arg, Const, Held, Kind, MirBody, MirBlock, Op, OrderedMap, Value};
use crate::optimize::strength::_made;
use crate::optimize::transform;

/// Direct port of `qbopt/optimize/ivshare.py:shared`.
pub fn shared(body: &Rc<MirBody>) -> Rc<MirBody> {
    let mut definitions: BTreeMap<u32, &Op> = BTreeMap::new();
    let mut made: BTreeMap<Value, &Op> = BTreeMap::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.defines {
                definitions.insert(value.id, op);
                made.insert(*value, op);
            }
        }
    }
    let required = transform::halves(body);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let counters = induction::basics(body, &loop_);
        let header_index = body
            .blocks
            .iter()
            .position(|block| block.at == loop_.header)
            .expect("loop header is a block");
        let header = &body.blocks[header_index];
        for derived in counters.values() {
            let twin = _twin(&counters, derived, header, &required);
            if let Some(twin) = twin {
                return Rc::new(_replacing(body, header_index, derived, twin, derived.start.width()));
            }
            let Some((base, offset, seed)) = _offset(&counters, derived, &definitions, &made) else {
                continue;
            };
            let width = derived.start.width();
            let phi_index = header
                .phis
                .iter()
                .position(|one| one.result.id == derived.value)
                .expect("counter has a header phi");
            let phi = &header.phis[phi_index];
            let mut upper: Option<Value> = None;
            if width < 4 && required.contains(&(phi.result, transform::HIGH)) {
                let mut carried = Vec::new();
                let mut broke = false;
                for incoming in phi.incoming.values() {
                    let producer = definitions.get(&incoming.id).copied();
                    let Some(producer) = producer.filter(|producer| {
                        producer.results
                            == vec![Arg::Held(Held {
                                value: *incoming,
                                width,
                            })]
                            && producer.merges.len() == 1
                    }) else {
                        broke = true;
                        break;
                    };
                    carried.push(*producer.merges.keys().next().expect("one merge"));
                }
                if !broke && carried.iter().collect::<BTreeSet<_>>().len() == 1 {
                    upper = Some(carried[0]);
                }
                match upper {
                    Some(value) if induction::invariant(body, &loop_.body).contains(&value.id) => {}
                    _ => continue,
                }
            }
            let root = header
                .phis
                .iter()
                .find(|one| one.result.id == base.value)
                .expect("base counter has a header phi")
                .result;
            let mut operation = _made(
                Kind::Add,
                "add",
                phi.result,
                vec![Arg::Held(Held { value: root, width }), Arg::Const(offset)],
                header.at,
                seed.unwrap_or(&header.ops[0]),
            );
            if let Some(upper) = upper {
                let mut merges = OrderedMap::new();
                merges.insert(upper, phi.result);
                operation.merges = merges;
                operation.uses.push(upper);
            }
            let mut changed = header.clone();
            changed.phis.remove(phi_index);
            changed.ops.insert(0, operation);
            let mut result = MirBody::clone(body);
            result.blocks[header_index] = changed;
            return Rc::new(result);
        }
    }
    body.clone()
}

/// Another counter stepping as `derived` does a constant distance behind it, the distance, and a seed.
///
/// Two starts are that far apart when `induction::distance` says so:
/// `add source,c` over the other's start, or strength reduction's `a[i].x`
/// and `a[i].y`, starting 4 apart. Otherwise the canonical counter is the
/// lower id, so the pair converges.
fn _offset<'a, 'b>(
    counters: &'a OrderedMap<u32, Affine>,
    derived: &Affine,
    definitions: &BTreeMap<u32, &'b Op>,
    made: &BTreeMap<Value, &'b Op>,
) -> Option<(&'a Affine, Const, Option<&'b Op>)> {
    if !matches!(derived.step, AffineOperand::Const(_)) {
        return None;
    }
    let width = derived.start.width();
    let seed = match &derived.start {
        AffineOperand::Held(start) => definitions.get(&start.value.id).copied(),
        AffineOperand::Const(_) => None,
    };
    let direct: &[Arg] = match seed {
        Some(seed) if seed.kind == Kind::Add => &seed.args,
        _ => &[],
    };
    let mut ordered: Vec<&Affine> = counters.values().collect();
    ordered.sort_by_key(|one| (!direct.contains(&one.start.as_arg()), one.value));
    for one in ordered {
        if one.value == derived.value || one.step != derived.step {
            continue;
        }
        if !direct.contains(&one.start.as_arg()) && one.value > derived.value {
            continue;
        }
        if one.start.width() != width {
            continue;
        }
        let Some(distance) = induction::distance(&derived.start.as_arg(), &one.start.as_arg(), made, width) else {
            continue;
        };
        return Some((one, Const::new(distance, width), seed));
    }
    None
}

/// Another counter of this loop that advances identically, or None.
///
/// Direct port of `qbopt/optimize/ivshare.py:_twin`.
fn _twin<'a>(
    counters: &'a OrderedMap<u32, Affine>,
    derived: &Affine,
    header: &MirBlock,
    required: &BTreeSet<(Value, u8)>,
) -> Option<&'a Affine> {
    if header
        .phis
        .iter()
        .any(|one| one.result.id == derived.value && required.contains(&(one.result, transform::HIGH)))
    {
        return None; // a long pair's high half is tied to this phi; the offset path owns that
    }
    counters
        .values()
        .find(|one| one.value < derived.value && one.start == derived.start && one.step == derived.step)
}

/// `body` with `derived`'s phi replaced by a copy of `twin`'s.
///
/// Direct port of `qbopt/optimize/ivshare.py:_replacing`.  `header` is the
/// block's index: Python compares the block by identity.
fn _replacing(body: &MirBody, header: usize, derived: &Affine, twin: &Affine, width: u32) -> MirBody {
    let block = &body.blocks[header];
    let phi_index = block
        .phis
        .iter()
        .position(|one| one.result.id == derived.value)
        .expect("counter has a header phi");
    let phi = &block.phis[phi_index];
    let root = block
        .phis
        .iter()
        .find(|one| one.result.id == twin.value)
        .expect("twin counter has a header phi")
        .result;
    let operation = _made(
        Kind::Copy,
        "mov",
        phi.result,
        vec![Arg::Held(Held { value: root, width })],
        block.at,
        &block.ops[0],
    );
    let mut changed = block.clone();
    changed.phis.remove(phi_index);
    changed.ops.insert(0, operation);
    let mut result = body.clone();
    result.blocks[header] = changed;
    result
}
