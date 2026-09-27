//! Which values are live where, adapted from llrm-core's
//! `analysis/liveness.rs` (a port of `qbopt/analysis/liveness.py`).
//!
//! That module read the old MIR's `MirBody`, so it stays here rather than in
//! `llrm-graph`; the machine phases have their own, `backend/liveness.rs`,
//! over physical register lanes.  Blocks are keyed by [`cfg::id`], as the
//! loops that `pressure`'s `inside` comes from name them.
//!
//! The old MIR's exit values (observable without being operands) and flag
//! values have no counterpart: a return's value is its operand, and every
//! value counts toward pressure.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{BlockId, Function, Instruction, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::bits::Bits;
use llrm_support::hash::HashMap;

use crate::cfg::id;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Liveness {
    pub live_in: BTreeMap<i64, BTreeSet<ValueId>>,
    pub live_out: BTreeMap<i64, BTreeSet<ValueId>>,
}

/// The local values an instruction reads.
fn reads(instruction: &Instruction) -> impl Iterator<Item = ValueId> + '_ {
    instruction.operands.iter().filter_map(|operand| match operand {
        Operand::Value(value) => Some(*value),
        _ => None,
    })
}

fn is_phi(instruction: &Instruction) -> bool {
    instruction.opcode == Opcode::Phi
}

/// A block's instructions: its phis, then its ordinary operations.
fn split(function: &Function, block: BlockId) -> (Vec<&Instruction>, Vec<&Instruction>) {
    function.block(block).instructions().iter().map(|&one| function.instruction(one)).partition(|one| is_phi(one))
}

/// A phi's value on the edge from `predecessor`, when it is a local value.
fn arm(phi: &Instruction, predecessor: BlockId) -> Option<ValueId> {
    phi.operands.chunks(2).find(|pair| pair[1] == Operand::Block(predecessor)).and_then(|pair| match pair[0] {
        Operand::Value(value) => Some(value),
        _ => None,
    })
}

fn _defines(function: &Function, block: BlockId) -> BTreeSet<ValueId> {
    function.block(block).instructions().iter().filter_map(|&one| function.instruction(one).result).collect()
}

/// Values the ordinary operations read before writing.
fn _exposed(function: &Function, block: BlockId) -> BTreeSet<ValueId> {
    let mut live = BTreeSet::new();
    for op in split(function, block).1.into_iter().rev() {
        if let Some(one) = op.result {
            live.remove(&one);
        }
        live.extend(reads(op));
    }
    live
}

/// Values the caller supplied: used somewhere, defined by nothing here.
pub fn entry_values(function: &Function) -> BTreeSet<ValueId> {
    let defined: BTreeSet<ValueId> = function.layout().iter().flat_map(|&block| _defines(function, block)).collect();
    let used: BTreeSet<ValueId> = function.walk().flat_map(|(_, one)| reads(function.instruction(one))).collect();
    used.difference(&defined).copied().collect()
}

/// The most values live at once -- anywhere, or in `inside`.
pub fn pressure(function: &Function, found: Option<&Liveness>, inside: Option<&BTreeSet<i64>>) -> usize {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut peak = 0;
    for &block in function.layout() {
        if inside.is_some_and(|inside| !inside.contains(&id(block))) {
            continue;
        }
        let mut alive = found.live_out[&id(block)].clone();
        peak = peak.max(alive.len());
        for op in split(function, block).1.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
            peak = peak.max(alive.len());
        }
    }
    peak
}

/// The edge operands of phis whose results are actually live.
pub fn phi_inputs(function: &Function, found: Option<&Liveness>) -> BTreeSet<ValueId> {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut inputs = BTreeSet::new();
    for &block in function.layout() {
        let (phis, ops) = split(function, block);
        let mut alive = found.live_out[&id(block)].clone();
        for op in ops.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
        }
        for phi in phis {
            if phi.result.is_some_and(|result| alive.contains(&result)) {
                inputs.extend(reads(phi));
            }
        }
    }
    inputs
}

/// What is live at each block's entry and exit, to a fixed point.
///
/// Run on bit sets over dense value indices; the sets, and the order they
/// are updated in, are Python's.
pub fn live(function: &Function) -> Liveness {
    let layout = function.layout();
    let mut index: HashMap<ValueId, usize> = HashMap::default();
    let mut values: Vec<ValueId> = Vec::new();
    for (_, one) in function.walk() {
        let instruction = function.instruction(one);
        for value in instruction.result.into_iter().chain(reads(instruction)) {
            index.entry(value).or_insert_with(|| {
                values.push(value);
                values.len() - 1
            });
        }
    }
    let bits = |ones: &mut dyn Iterator<Item = ValueId>| {
        let mut set = Bits::new(values.len());
        for one in ones {
            set.insert(index[&one]);
        }
        set
    };
    let at_index: HashMap<BlockId, usize> = layout.iter().enumerate().map(|(position, &block)| (block, position)).collect();
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> = layout.iter().map(|&block| split(function, block)).collect();

    let mut op_defines: Vec<Bits> = blocks.iter().map(|(_, ops)| bits(&mut ops.iter().filter_map(|op| op.result))).collect();
    let phi_defines: Vec<Bits> = blocks.iter().map(|(phis, _)| bits(&mut phis.iter().filter_map(|phi| phi.result))).collect();
    let mut exposed: Vec<Bits> = layout.iter().map(|&block| bits(&mut _exposed(function, block).into_iter())).collect();
    if !layout.is_empty() {
        let arriving = bits(&mut entry_values(function).into_iter());
        op_defines[0].union_with(&arriving);
        exposed[0].subtract(&arriving);
    }
    // Each successor as its position and, per phi, (result, this block's arm).
    let successors: Vec<Vec<(usize, Vec<(usize, usize)>)>> = layout
        .iter()
        .map(|&block| {
            function
                .successors(block)
                .into_iter()
                .filter_map(|successor| at_index.get(&successor))
                .map(|&position| {
                    let arms = blocks[position]
                        .0
                        .iter()
                        .filter_map(|phi| Some((index[&phi.result?], index[&arm(phi, block)?])))
                        .collect();
                    (position, arms)
                })
                .collect()
        })
        .collect();
    let empty = Bits::new(values.len());
    let mut live_in = vec![empty.clone(); layout.len()];
    let mut live_out = vec![empty.clone(); layout.len()];
    let mut after_phis = vec![empty.clone(); layout.len()];

    let mut changing = true;
    while changing {
        changing = false;
        for position in 0..layout.len() {
            let mut out = empty.clone();
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
    let sets = |found: &[Bits]| -> BTreeMap<i64, BTreeSet<ValueId>> {
        layout.iter().zip(found).map(|(&block, set)| (id(block), set.iter().map(|one| values[one]).collect())).collect()
    };
    Liveness {
        live_in: sets(&live_in),
        live_out: sets(&live_out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{block, function, parsed, value};

    /// PARITYCONTROL's dead join-flag phis kept ADD/ADC live as word operations.
    #[test]
    fn test_a_dead_phi_does_not_keep_its_edge_operand_live() {
        let module = parsed("define void @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret void
}
");
        let function = function(&module, "f");
        let found = live(function);
        assert!(!found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")));

        let module = parsed("define i16 @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret i16 %merged
}
");
        let function = crate::testing::function(&module, "f");
        let found = live(function);
        assert!(found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")), "a live phi keeps it");
    }
}
