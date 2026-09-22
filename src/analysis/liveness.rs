//! Port of `qbopt/analysis/liveness.py`: which values are live where, over
//! MIR's own SSA values.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{self, MirBlock, MirBody, Value};
use crate::support::bits::Bits;
use crate::support::hash::HashMap;

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
///
/// Run on bit sets over dense value indices; the sets, and the order they
/// are updated in, are Python's.
pub fn live(body: &MirBody) -> Liveness {
    let mut index: HashMap<Value, usize> = HashMap::default();
    let mut values: Vec<Value> = Vec::new();
    for block in &body.blocks {
        let phis = block.phis.iter().flat_map(|phi| std::iter::once(&phi.result).chain(phi.incoming.values()));
        let ops = block.ops.iter().flat_map(|op| op.defines.iter().chain(&op.uses).chain(mir::exit_values(op)));
        for one in phis.chain(ops) {
            index.entry(*one).or_insert_with(|| {
                values.push(*one);
                values.len() - 1
            });
        }
    }
    let bits = |ones: &mut dyn Iterator<Item = &Value>| {
        let mut set = Bits::new(values.len());
        for one in ones {
            set.insert(index[one]);
        }
        set
    };
    let at_index: HashMap<i64, usize> =
        body.blocks.iter().enumerate().rev().map(|(position, block)| (block.at, position)).collect();

    let mut op_defines: Vec<Bits> =
        body.blocks.iter().map(|block| bits(&mut block.ops.iter().flat_map(|op| &op.defines))).collect();
    let phi_defines: Vec<Bits> =
        body.blocks.iter().map(|block| bits(&mut block.phis.iter().map(|phi| &phi.result))).collect();
    let mut exposed: Vec<Bits> = body.blocks.iter().map(|block| bits(&mut _exposed(block).iter())).collect();
    let exits: Vec<Bits> = body
        .blocks
        .iter()
        .map(|block| bits(&mut block.ops.iter().flat_map(|op| mir::exit_values(op))))
        .collect();
    if !body.blocks.is_empty() {
        let arriving = bits(&mut entry_values(body).iter());
        if let Some(&entry) = at_index.get(&body.entry) {
            op_defines[entry].union_with(&arriving);
            exposed[entry].subtract(&arriving);
        }
    }
    // Each successor as its position and, per phi, (result, this block's arm).
    let successors: Vec<Vec<(usize, Vec<(usize, usize)>)>> = body
        .blocks
        .iter()
        .map(|block| {
            block
                .succ
                .iter()
                .filter_map(|successor| at_index.get(successor))
                .map(|&position| {
                    let arms = body.blocks[position]
                        .phis
                        .iter()
                        .filter_map(|phi| phi.incoming.get(&block.at).map(|arm| (index[&phi.result], index[arm])))
                        .collect();
                    (position, arms)
                })
                .collect()
        })
        .collect();
    let empty = Bits::new(values.len());
    let mut live_in = vec![empty.clone(); body.blocks.len()];
    let mut live_out = vec![empty.clone(); body.blocks.len()];
    let mut after_phis = vec![empty; body.blocks.len()];

    let mut changing = true;
    while changing {
        changing = false;
        for position in 0..body.blocks.len() {
            let mut out = exits[position].clone();
            for (successor, arms) in &successors[position] {
                out.union_with(&live_in[*successor]);
                for &(result, arm) in arms {
                    if after_phis[*successor].contains(result) {
                        out.insert(arm);
                    }
                }
            }
            let mut after = out.clone();
            after.subtract(&op_defines[position]);
            after.union_with(&exposed[position]);
            let mut inside = after.clone();
            inside.subtract(&phi_defines[position]);
            if out != live_out[position] || inside != live_in[position] || after != after_phis[position] {
                live_out[position] = out;
                live_in[position] = inside;
                after_phis[position] = after;
                changing = true;
            }
        }
    }
    let sets = |found: &[Bits]| -> BTreeMap<i64, BTreeSet<Value>> {
        body.blocks
            .iter()
            .zip(found)
            .map(|(block, set)| (block.at, set.iter().map(|one| values[one]).collect()))
            .collect()
    };
    Liveness {
        live_in: sets(&live_in),
        live_out: sets(&live_out),
    }
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
