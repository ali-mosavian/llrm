//! Port of `qbopt/frontend/raising_carried.py`: registers a caller relies on
//! a call leaving alone.
//!
//! A clobber is a may-clobber. The QB45 table lists SI or DI for some routines
//! whose documented convention preserves them, because a header elsewhere says
//! otherwise (runtime.py's B$FOUTBX note). BC's own code reads SI after such
//! calls without reloading it -- BENCHMARK's `mov si,[bp+6]` survives ten
//! runtime calls to a `push word [si]`. Read as "the callee defines SI", the
//! value before the call is dead, its load is deleted, and the read sees
//! whatever the rebuild left in SI.
//!
//! So where a caller reads, after a call, a convention-preserved register the
//! contract lists as clobbered, the value it held before the call is an input:
//! whatever the callee does with it, it starts from the same state BC's code
//! did.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::abi::runtime::{self, Contract, Reg};
use crate::frontend::blocks::Block;
use crate::model::ir::nodes::Node;
use crate::model::mir::{self, Registers};
use crate::support::hash::IndexMap;

/// The registers cmacros' convention keeps and the raise tracks as values.
fn _candidates() -> BTreeSet<Reg> {
    BTreeSet::from([Reg::Si, Reg::Di]).difference(&runtime::PER_CONVENTION).copied().collect()
}

/// Each call site whose contract needs a carried register as an input.
pub fn carried(
    blocks: &[Block],
    nodes: &IndexMap<i64, Arc<Node>>,
    calls: &IndexMap<i64, String>,
    contracts: &IndexMap<i64, Contract>,
) -> IndexMap<i64, Contract> {
    let candidates = _candidates();
    let mut chosen = contracts.clone();
    let mut changed: IndexMap<i64, Contract> = IndexMap::default();
    let by_at: BTreeSet<usize> = blocks.iter().map(|block| block.at).collect();
    loop {
        let mut live_in: IndexMap<usize, Registers> = blocks.iter().map(|block| (block.at, Registers::new())).collect();
        let mut after: IndexMap<i64, Registers> = IndexMap::default();
        let mut moving = true;
        while moving {
            moving = false;
            for block in blocks.iter().rev() {
                let mut live: Registers = block
                    .succ
                    .iter()
                    .filter(|one| by_at.contains(one))
                    .flat_map(|one| live_in[one].iter().copied())
                    .collect();
                for insn in block.insns.iter().rev() {
                    let Some(node) = nodes.get(&(insn.at as i64)) else {
                        continue;
                    };
                    after.insert(insn.at as i64, live.clone());
                    let (defines, uses) = mir::touched(node, Some(calls), Some(&chosen));
                    live = live.difference(&defines).copied().chain(uses).collect();
                }
                if live != live_in[&block.at] {
                    live_in.insert(block.at, live);
                    moving = true;
                }
            }
        }
        let mut grown = false;
        for index in 0..chosen.len() {
            let (&at, routine) = chosen.get_index(index).expect("in range");
            if !after.contains_key(&at) || !runtime::established_inputs(routine) {
                continue;
            }
            let inputs = routine.inputs.clone().unwrap_or_default();
            let extra: BTreeSet<Reg> = runtime::disturbs(routine)
                .difference(&inputs)
                .filter(|one| candidates.contains(one))
                .filter(|&&one| mir::from_contract(one).is_some_and(|root| after[&at].contains(&root)))
                .copied()
                .collect();
            if !extra.is_empty() {
                let mut made = routine.clone();
                made.inputs = Some(inputs.union(&extra).copied().collect());
                made.evidence = format!(
                    "{} Carried: the caller reads {} after this call without redefining it.",
                    routine.evidence,
                    extra.iter().map(|one| one.value()).collect::<Vec<_>>().join(", ")
                );
                chosen.insert(at, made.clone());
                changed.insert(at, made);
                grown = true;
            }
        }
        if !grown {
            return changed;
        }
    }
}
