//! Port of `qbopt/analysis/liveness.py`: which values are live where, over
//! MIR's own SSA values.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{self, MirBlock, MirBody, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Liveness {
    pub live_in: BTreeMap<i64, BTreeSet<Value>>,
    pub live_out: BTreeMap<i64, BTreeSet<Value>>,
}

fn _defines(block: &MirBlock) -> BTreeSet<Value> {
    let mut out: BTreeSet<Value> = block.phis.iter().map(|phi| phi.result).collect();
    out.extend(block.ops.iter().flat_map(|op| op.defines.iter().copied()));
    out
}

/// Values the ordinary operations read before writing.
fn _exposed(block: &MirBlock) -> BTreeSet<Value> {
    let mut live = BTreeSet::new();
    for op in block.ops.iter().rev() {
        for one in &op.defines {
            live.remove(one);
        }
        live.extend(op.uses.iter().copied());
    }
    live
}

/// Values the caller supplied: used somewhere, defined by nothing here.
pub fn entry_values(body: &MirBody) -> BTreeSet<Value> {
    let defined: BTreeSet<Value> = body.blocks.iter().flat_map(_defines).collect();
    let mut used = BTreeSet::new();
    for block in &body.blocks {
        for op in &block.ops {
            used.extend(op.uses.iter().copied());
            used.extend(op.exits.iter().copied());
        }
        for phi in &block.phis {
            used.extend(phi.incoming.values().copied());
        }
    }
    used.difference(&defined).copied().collect()
}

/// The most values live at once, flags aside -- anywhere, or in `inside`.
pub fn pressure(body: &MirBody, found: Option<&Liveness>, inside: Option<&BTreeSet<i64>>) -> usize {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(body);
            &owned
        }
    };
    let count = |alive: &BTreeSet<Value>| alive.iter().filter(|one| !one.flags).count();
    let mut peak = 0;
    for block in &body.blocks {
        if inside.is_some_and(|inside| !inside.contains(&block.at)) {
            continue;
        }
        let mut alive = found.live_out[&block.at].clone();
        peak = peak.max(count(&alive));
        for op in block.ops.iter().rev() {
            for one in &op.defines {
                alive.remove(one);
            }
            alive.extend(op.uses.iter().copied());
            peak = peak.max(count(&alive));
        }
    }
    peak
}

/// The edge operands of phis whose results are actually live.
pub fn phi_inputs(body: &MirBody, found: Option<&Liveness>) -> BTreeSet<Value> {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(body);
            &owned
        }
    };
    let mut inputs = BTreeSet::new();
    for block in &body.blocks {
        let mut alive = found.live_out[&block.at].clone();
        for op in block.ops.iter().rev() {
            for one in &op.defines {
                alive.remove(one);
            }
            alive.extend(op.uses.iter().copied());
        }
        for phi in &block.phis {
            if alive.contains(&phi.result) {
                inputs.extend(phi.incoming.values().copied());
            }
        }
    }
    inputs
}

/// What is live at each block's entry and exit, to a fixed point.
pub fn live(body: &MirBody) -> Liveness {
    let mut op_defines: BTreeMap<i64, BTreeSet<Value>> = body
        .blocks
        .iter()
        .map(|block| (block.at, block.ops.iter().flat_map(|op| op.defines.iter().copied()).collect()))
        .collect();
    let phi_defines: BTreeMap<i64, BTreeSet<Value>> =
        body.blocks.iter().map(|block| (block.at, block.phis.iter().map(|phi| phi.result).collect())).collect();
    let mut exposed: BTreeMap<i64, BTreeSet<Value>> =
        body.blocks.iter().map(|block| (block.at, _exposed(block))).collect();
    if !body.blocks.is_empty() {
        let arriving = entry_values(body);
        op_defines.entry(body.entry).or_default().extend(arriving.iter().copied());
        if let Some(exposed) = exposed.get_mut(&body.entry) {
            exposed.retain(|one| !arriving.contains(one));
        }
    }
    let empty = || body.blocks.iter().map(|block| (block.at, BTreeSet::new())).collect::<BTreeMap<_, _>>();
    let mut live_in = empty();
    let mut live_out = empty();
    let mut after_phis = empty();

    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            let mut out: BTreeSet<Value> =
                block.ops.iter().flat_map(|op| mir::exit_values(op).iter().copied()).collect();
            for successor in &block.succ {
                let Some(found) = body.block(*successor) else {
                    continue;
                };
                out.extend(live_in[successor].iter().copied());
                for phi in &found.phis {
                    if after_phis[successor].contains(&phi.result) {
                        out.extend(phi.incoming.get(&block.at).copied());
                    }
                }
            }
            let mut after: BTreeSet<Value> = out.difference(&op_defines[&block.at]).copied().collect();
            after.extend(exposed[&block.at].iter().copied());
            let inside: BTreeSet<Value> = after.difference(&phi_defines[&block.at]).copied().collect();
            if out != live_out[&block.at] || inside != live_in[&block.at] || after != after_phis[&block.at] {
                live_out.insert(block.at, out);
                live_in.insert(block.at, inside);
                after_phis.insert(block.at, after);
                changing = true;
            }
        }
    }
    Liveness { live_in, live_out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::mir::Phi;

    /// PARITYCONTROL's dead join-flag phis kept ADD/ADC live as word operations.
    #[test]
    fn test_a_dead_phi_does_not_keep_its_edge_operand_live() {
        let incoming = Value { flags: true, ..Value::new(1, 0x10) };
        let merged = Value { flags: true, ..Value::new(2, 0x20) };
        let phi = Phi { result: merged, incoming: [(0x10, incoming)].into_iter().collect() };
        let body = MirBody::new(
            0x10,
            vec![MirBlock::new(0x10, vec![], vec![], vec![0x20]), MirBlock::new(0x20, vec![phi], vec![], vec![])],
        );
        let found = live(&body);
        assert!(!found.live_out[&0x10].contains(&incoming));
    }
}
