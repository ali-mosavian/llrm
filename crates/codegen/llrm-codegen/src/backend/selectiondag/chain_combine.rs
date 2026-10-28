//! The walk that lets a load or store skip the memory nodes it cannot alias
//! (`mayAlias`, `GatherAllAliases`, `FindBetterChain`,
//! `parallelizeChainedStores`, `findBetterNeighborChains`; the pin's
//! [DAGCombiner.cpp
//! L24398](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24398)).
//!
//! # How it works
//! A load or store reads from the nearest chain it may alias: the walk goes up
//! the chain past the loads and stores whose address the address analysis
//! proves disjoint, and the aliases it gathers are joined by a token factor.
//! `visitTokenFactor` is `combine_token_factor.rs`.
//!
//! DEVIATE CC2: `mayAlias` has no `MachineMemOperand` offset, base alignment
//! or IR value: the relatively-aligned-offsets test and the alias analysis
//! (`useAA`, false for x86) are not run; and there are no atomics or
//! lifetime nodes. A pair the address analysis cannot decide is a may-alias.
//! Exit: `MemInfo` carrying the pointer info. Owner: w12.
//!
//! DEVIATE CC1: the pin skips all of this when optimising nothing
//! (`OptLevel == None`); no opt level reaches the combiner, so it runs at
//! every level. Exit: the level in `DagConfig`. Owner: w12.
//!
//! # Used by
//! `DagCombiner::visit`.

use llrm_support::hash::HashSet;

use super::address_analysis::{BaseIndexOffset, compute_aliasing};
use super::dag::{NodeData, Rauw, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;

/// `GatherAllAliasesMaxDepth`: 18, `TargetLoweringBase`; x86 does not set its
/// own.
const GATHER_ALL_ALIASES_MAX_DEPTH: usize = 18;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// The bytes a load or store moves, and its memory info.
    fn access(
        &self,
        node: SDValue,
    ) -> Option<(SDValue, u64, super::dag::MemInfo)> {
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let ptr = match self.dag.opcode(node) {
            ISD::Load => self.dag.operand(node, 1),
            ISD::Store => self.dag.operand(node, 2),
            _ => return None,
        };
        Some((ptr, u64::from(info.mem_vt.size_in_bits().div_ceil(8)), info))
    }

    /// `mayAlias` for two loads or stores.
    fn may_alias(
        &self,
        op0: SDValue,
        op1: SDValue,
    ) -> bool {
        let (Some((ptr0, size0, info0)), Some((ptr1, size1, info1))) = (self.access(op0), self.access(op1)) else {
            return true;
        };
        // If they are to the same address, they alias.
        if ptr0 == ptr1 {
            return true;
        }
        // Two volatile accesses cannot be reordered.
        if info0.volatile && info1.volatile {
            return true;
        }
        // A load of invariant memory and a store do not alias.
        let invariant = |op: SDValue| self.dag.invariant_loads.contains(&op.node);
        let is_store = |op: SDValue| self.dag.opcode(op) == ISD::Store;
        if (invariant(op0) && is_store(op1)) || (invariant(op1) && is_store(op0)) {
            return false;
        }
        compute_aliasing(self.dag, op0, Some(size0), op1, Some(size1)).unwrap_or(true)
    }

    /// `GatherAllAliases`: walk up the chain from `original` past the nodes
    /// `node` does not alias, collecting those it does.
    pub(super) fn gather_all_aliases(
        &self,
        node: SDValue,
        original: SDValue,
    ) -> Vec<SDValue> {
        let mut aliases = Vec::new();
        let mut visited: HashSet<u32> = HashSet::default();
        let is_load =
            self.dag.opcode(node) == ISD::Load && self.access(node).is_some_and(|(_, _, info)| !info.volatile);
        let mut chains = vec![original];
        let mut depth = 0;
        while let Some(chain) = chains.pop() {
            // Don't bother if we've seen the chain before.
            if !visited.insert(chain.node) {
                continue;
            }
            // Past the depth limit the original chain is all that is kept.
            if depth > GATHER_ALL_ALIASES_MAX_DEPTH {
                return vec![original];
            }
            if self.dag.opcode(chain) == ISD::TokenFactor {
                // A big token factor is an alias in itself.
                let ops = &self.dag.node(chain).ops;
                if ops.len() > 16 {
                    aliases.push(chain);
                    continue;
                }
                chains.extend(ops.iter().rev().copied());
                depth += 1;
                continue;
            }
            // Attempt to improve the chain by a single step.
            let improved = match self.dag.opcode(chain) {
                // Nothing to mark.
                ISD::EntryToken => Some(None),
                ISD::Load | ISD::Store => {
                    let op_is_load = self.dag.opcode(chain) == ISD::Load
                        && self.access(chain).is_some_and(|(_, _, info)| !info.volatile);
                    (is_load && op_is_load || !self.may_alias(node, chain)).then(|| Some(self.dag.operand(chain, 0)))
                }
                // Always forward past a CopyFromReg.
                ISD::CopyFromReg => Some(Some(self.dag.operand(chain, 0))),
                _ => None,
            };
            match improved {
                Some(next) => {
                    chains.extend(next);
                    depth += 1;
                }
                // No improved chain is possible: treat it as an alias.
                None => aliases.push(chain),
            }
        }
        aliases
    }

    /// `FindBetterChain`: the chain `node` may use instead of `old`.
    fn find_better_chain(
        &mut self,
        node: SDValue,
        old: SDValue,
    ) -> SDValue {
        let aliases = self.gather_all_aliases(node, old);
        match aliases[..] {
            [] => self.dag.entry_node(),
            [only] => only,
            _ => self.dag.get_token_factor(&aliases),
        }
    }

    /// `visitLOAD`'s chain step: a load reads from the nearest chain it may
    /// alias, and a token factor keeps the old chain for its users.
    pub(super) fn improve_load_chain(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let chain = self.dag.operand(node, 0);
        let better = self.find_better_chain(node, chain);
        if better == chain {
            return None;
        }
        let (NodeData::Mem(info), ptr) = (self.dag.node(node).data, self.dag.operand(node, 1)) else { return None };
        let replacement = self.dag.get_load(self.dag.value_type(node), better, ptr, info);
        let token = self.dag.get_token_factor(&[chain, SDValue { node: replacement.node, res: 1 }]);
        // `CombineTo(N, ReplLoad.getValue(0), Token)`: the value and the chain
        // are replaced separately.
        let results = [replacement, token];
        for (res, new) in results.into_iter().enumerate() {
            for event in self.dag.replace_all_uses_with(SDValue { node: node.node, res: res as u8 }, new) {
                match event {
                    Rauw::Updated(user) => self.add_to_worklist(user),
                    Rauw::Merged { into, .. } => self.add_to_worklist_with_users(into),
                }
            }
        }
        self.add_to_worklist(token.node);
        Some(node)
    }

    /// `parallelizeChainedStores`: the stores chained below `store` that cannot
    /// overlap it are improved together and joined by one token factor.
    fn parallelize_chained_stores(
        &mut self,
        store: SDValue,
    ) -> Option<SDValue> {
        let base_ptr = BaseIndexOffset::matching(self.dag, store);
        base_ptr.base?;
        if self.dag.is_undef(base_ptr.base?) {
            return None;
        }
        let (_, size, info) = self.access(store)?;
        if size == 0 {
            return None;
        }
        // Which offsets from the base are covered, as sorted half-open
        // intervals that merge when they touch (`IntervalMap`).
        let mut intervals: Vec<(i64, i64)> = vec![(0, size as i64)];
        let mut chained: Vec<SDValue> = Vec::new();
        let mut last = store;
        while self.dag.opcode(self.dag.operand(last, 0)) == ISD::Store {
            let chain = self.dag.operand(last, 0);
            // More than one use: the memory operations cannot be reordered.
            if self.dag.use_count(chain.node) != 1 {
                break;
            }
            let (_, length, chain_info) = self.access(chain)?;
            if chain_info.volatile {
                break;
            }
            let ptr = BaseIndexOffset::matching(self.dag, chain);
            let Some(offset) = base_ptr.equal_base_index(&ptr, self.dag) else { break };
            let length = length as i64;
            // `find`: the first interval ending after the offset.
            let at = intervals.iter().position(|&(_, stop)| stop > offset);
            if let Some(at) = at {
                if intervals[at].0 < offset + length {
                    break;
                }
            }
            if at != Some(0) && !intervals.is_empty() {
                // There is an interval before: `(--I).stop() <= Offset`.
                let before = at.map_or(intervals.len(), |at| at) - 1;
                if intervals[before].1 <= offset {
                    break;
                }
            }
            intervals.push((offset, offset + length));
            intervals.sort_unstable();
            let mut merged: Vec<(i64, i64)> = Vec::new();
            for (start, stop) in intervals.drain(..) {
                match merged.last_mut() {
                    Some(last) if last.1 == start => last.1 = stop,
                    _ => merged.push((start, stop)),
                }
            }
            intervals = merged;
            chained.push(chain);
            last = chain;
        }
        // If we didn't find a chained store, exit.
        if chained.is_empty() {
            return None;
        }
        // Improve all the chained stores, from where the store chain ended.
        let new_chain = self.dag.operand(last, 0);
        let mut token_ops: Vec<SDValue> = Vec::new();
        for &stored in chained.iter().rev() {
            let better = self.find_better_chain(stored, new_chain);
            let mut ops = self.dag.node(stored).ops.clone();
            ops[0] = better;
            let updated = self.dag.update_node_operands(stored.node, &ops);
            token_ops.push(SDValue { node: updated, res: 0 });
        }
        // Improve the store's chain with a new node: no loop through the
        // replacement.
        let better = self.find_better_chain(store, new_chain);
        let (value, ptr) = (self.dag.operand(store, 1), self.dag.operand(store, 2));
        let new_store = self.dag.get_store(better, value, ptr, info);
        token_ops.push(new_store);
        // If every chain improved, the dependence on the old chain is lost
        // for the users: add it back, first.
        if token_ops.iter().all(|op| self.dag.operand(*op, 0) != new_chain) {
            token_ops.insert(0, new_chain);
        }
        let token = self.dag.get_token_factor(&token_ops);
        self.add_to_worklist(token.node);
        for op in self.dag.node(token).ops.clone() {
            self.add_to_worklist(op.node);
        }
        self.add_to_worklist(last.node);
        Some(token)
    }

    /// `findBetterNeighborChains`: what replaces `store` when its chain, or the
    /// stores chained below it, can skip nodes it does not alias.
    pub(super) fn find_better_neighbor_chains(
        &mut self,
        store: SDValue,
    ) -> Option<SDValue> {
        let base_ptr = BaseIndexOffset::matching(self.dag, store);
        let base = base_ptr.base?;
        if self.dag.is_undef(base) {
            return None;
        }
        if let Some(replacement) = self.parallelize_chained_stores(store) {
            return Some(replacement);
        }
        let chain = self.dag.operand(store, 0);
        let better = self.find_better_chain(store, chain);
        if better == chain {
            return None;
        }
        // `replaceStoreChain`: the store on the better chain, and a token
        // factor that keeps both around.
        let (value, ptr) = (self.dag.operand(store, 1), self.dag.operand(store, 2));
        let NodeData::Mem(info) = self.dag.node(store).data else { return None };
        let replacement = self.dag.get_store(better, value, ptr, info);
        let token = self.dag.get_token_factor(&[chain, replacement]);
        self.add_to_worklist(token.node);
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::super::mvt::EVT;
    use super::*;
    use crate::backend::selectiondag::dag::{DagConfig, MemBase, MemInfo, SelectionDAG};
    use crate::backend::selectiondag::dag_combiner::{CombineLevel, NoCombines, combine};

    fn info() -> MemInfo {
        MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Unknown, ext: None, space: 0 }
    }

    /// The root after one combine, from a token factor made of `build`'s
    /// chains.
    fn combined(build: impl FnOnce(&mut SelectionDAG) -> Vec<SDValue>) -> (SelectionDAG, SDValue) {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let chains = build(&mut dag);
        let root = dag.get_node(ISD::TokenFactor, EVT::OTHER, &chains);
        dag.set_root(root);
        combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
        let root = dag.get_root();
        (dag, root)
    }

    /// The nodes reachable from `root`, by opcode.
    fn reachable(
        dag: &SelectionDAG,
        root: SDValue,
        opcode: ISD,
    ) -> Vec<SDValue> {
        let (mut seen, mut stack, mut found) = (std::collections::BTreeSet::new(), vec![root], Vec::new());
        while let Some(at) = stack.pop() {
            if !seen.insert(at.node) {
                continue;
            }
            if dag.opcode(at) == opcode {
                found.push(at);
            }
            stack.extend(dag.node(at).ops.iter().copied());
        }
        found
    }

    /// A load of one stack slot read through the chain of a store to another:
    /// the load goes to the entry, the order kept by a token factor.
    #[test]
    fn a_load_skips_a_store_to_another_slot() {
        let (dag, root) = combined(|dag| {
            let entry = dag.entry_node();
            let (slot0, slot1) = (dag.get_frame_index(EVT::I32, 0), dag.get_frame_index(EVT::I32, 1));
            let v = dag.get_constant(1, EVT::I32);
            let store = dag.get_store(entry, v, slot0, info());
            let load = dag.get_load(EVT::I32, store, slot1, info());
            // The loaded value is read, so the load is not a dead one.
            let sink = dag.get_register(EVT::I32, 7);
            vec![dag.get_store(SDValue { node: load.node, res: 1 }, load, sink, info())]
        });
        let loads = reachable(&dag, root, ISD::Load);
        assert_eq!(loads.len(), 1, "{}", dag.dump());
        assert_eq!(dag.opcode(dag.operand(loads[0], 0)), ISD::EntryToken, "{}", dag.dump());
    }

    /// The same slot is the same memory: the load stays after the store.
    #[test]
    fn a_load_keeps_the_store_to_its_own_slot() {
        let (dag, root) = combined(|dag| {
            let entry = dag.entry_node();
            let slot = dag.get_frame_index(EVT::I32, 0);
            let v = dag.get_constant(1, EVT::I32);
            let store = dag.get_store(entry, v, slot, info());
            let load = dag.get_load(EVT::I32, store, slot, info());
            vec![SDValue { node: load.node, res: 1 }]
        });
        let loads = reachable(&dag, root, ISD::Load);
        assert_eq!(dag.opcode(dag.operand(loads[0], 0)), ISD::Store, "{}", dag.dump());
    }

    /// Two stores to different slots are chained: the second no longer waits
    /// for the first, and a token factor joins them.
    #[test]
    fn stores_to_different_slots_are_unchained() {
        let (dag, root) = combined(|dag| {
            let entry = dag.entry_node();
            let (slot0, slot1) = (dag.get_frame_index(EVT::I32, 0), dag.get_frame_index(EVT::I32, 1));
            let v = dag.get_constant(1, EVT::I32);
            let first = dag.get_store(entry, v, slot0, info());
            let second = dag.get_store(first, v, slot1, info());
            vec![second]
        });
        let stores = reachable(&dag, root, ISD::Store);
        assert_eq!(stores.len(), 2, "{}", dag.dump());
        assert!(stores.iter().all(|s| dag.opcode(dag.operand(*s, 0)) == ISD::EntryToken), "{}", dag.dump());
    }

    /// An unknown pointer may be anywhere: the order of the memory accesses is
    /// kept.
    #[test]
    fn an_unknown_pointer_keeps_the_order() {
        let (dag, root) = combined(|dag| {
            let entry = dag.entry_node();
            let (p, slot) = (dag.get_register(EVT::I32, 100), dag.get_frame_index(EVT::I32, 0));
            let v = dag.get_constant(1, EVT::I32);
            let store = dag.get_store(entry, v, p, info());
            let load = dag.get_load(EVT::I32, store, slot, info());
            vec![SDValue { node: load.node, res: 1 }]
        });
        let loads = reachable(&dag, root, ISD::Load);
        assert_eq!(dag.opcode(dag.operand(loads[0], 0)), ISD::Store, "{}", dag.dump());
    }

    /// histo's `counts[i]++`: the load read through an older chain and the
    /// store on a better one left the old store a reader of the sum, so the
    /// sum looked shared and the 16-bit read-modify-write was promoted to
    /// 32 bits (`inc word ptr` became load, `inc`, store).
    #[test]
    fn a_replaced_store_is_no_user_of_its_value() {
        let (dag, root) = combined(|dag| {
            let entry = dag.entry_node();
            let (slot0, slot1) = (dag.get_frame_index(EVT::I32, 0), dag.get_frame_index(EVT::I32, 1));
            let other = dag.get_load(EVT::I32, entry, slot1, info());
            let load = dag.get_load(EVT::I32, SDValue { node: other.node, res: 1 }, slot0, info());
            let one = dag.get_constant(1, EVT::I32);
            let sum = dag.get_node(ISD::Add, EVT::I32, &[load, one]);
            vec![dag.get_store(SDValue { node: load.node, res: 1 }, sum, slot0, info())]
        });
        for sum in reachable(&dag, root, ISD::Add) {
            assert_eq!(dag.users(sum).count(), 1, "{}", dag.dump());
        }
    }
}
