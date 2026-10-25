//! The combiner's work on chains: `visitTokenFactor`, and the walk that lets a
//! load or store skip the memory nodes it cannot alias (`mayAlias`,
//! `GatherAllAliases`, `FindBetterChain`, `parallelizeChainedStores`,
//! `findBetterNeighborChains`; the pin's
//! [DAGCombiner.cpp
//! L2257](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2257)).
//!
//! # How it works
//! A token factor drops entry tokens and repeated operands, takes in the
//! operands that are token factors only this one reads, and drops each operand
//! some other operand already depends on (found by walking the chains up from
//! all operands at once, breadth first, until one operand's search is all
//! that is left).
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
use super::mvt::EVT;

/// `TokenFactorInlineLimit`.
const TOKEN_FACTOR_INLINE_LIMIT: usize = 2048;

/// `getInputChainForNode`: the chain operand of `node`.
fn input_chain(
    dag: &super::dag::SelectionDAG,
    node: u32,
) -> Option<SDValue> {
    let ops = &dag.node(SDValue { node, res: 0 }).ops;
    let is_chain = |op: &SDValue| dag.value_type(*op) == EVT::OTHER;
    let last = ops.len().checked_sub(1)?;
    if is_chain(&ops[0]) {
        return Some(ops[0]);
    }
    if is_chain(&ops[last]) {
        return Some(ops[last]);
    }
    ops.iter().take(last).skip(1).find(|op| is_chain(op)).copied()
}

/// The search that finds the operands another operand already depends on.
struct Prune<'a> {
    /// Nodes to climb from, and the original operand each started at.
    worklist: Vec<(u32, usize)>,
    /// Pending work per operand.
    op_work_count: Vec<i64>,
    seen_chains: HashSet<u32>,
    seen_ops: &'a HashSet<u32>,
    ops: &'a [SDValue],
    changed: bool,
    did_prune_ops: bool,
    num_left_to_consider: i64,
}

impl Prune<'_> {
    fn add(
        &mut self,
        current: usize,
        node: u32,
        op_number: usize,
    ) {
        // An operand reached from another: the first is redundant. Its search
        // continues as the other's.
        if self.seen_ops.contains(&node) {
            self.changed = true;
            self.did_prune_ops = true;
            let original = self.ops.iter().position(|op| op.node == node).expect("a token factor operand");
            for entry in self.worklist.iter_mut().skip(current + 1) {
                if entry.1 == original {
                    entry.1 = op_number;
                }
            }
            self.op_work_count[op_number] += self.op_work_count[original];
            self.op_work_count[original] = 0;
            self.num_left_to_consider -= 1;
        }
        // Add if it's a new chain.
        if self.seen_chains.insert(node) {
            self.op_work_count[op_number] += 1;
            self.worklist.push((node, op_number));
        }
    }
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `visitTokenFactor`.
    pub(super) fn visit_token_factor(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let operands = self.dag.node(node).ops.clone();
        // Two operands, one with the other as its input chain: the other is
        // redundant.
        if let [first, second] = operands[..] {
            if input_chain(self.dag, first.node) == Some(second) {
                return Some(first);
            }
            if input_chain(self.dag, second.node) == Some(first) {
                return Some(second);
            }
        }
        if operands.len() > TOKEN_FACTOR_INLINE_LIMIT {
            return None;
        }
        // The sole user a token factor: give it a chance to merge with this.
        if self.dag.use_count(node.node) == 1 {
            let user = self.dag.users(node).next();
            if let Some(user) = user.filter(|&user| self.dag.opcode(SDValue { node: user, res: 0 }) == ISD::TokenFactor)
            {
                self.add_to_worklist(user);
            }
        }
        let mut token_factors = vec![node.node];
        let mut ops: Vec<SDValue> = Vec::new();
        let mut seen_ops: HashSet<u32> = HashSet::default();
        let mut changed = false;
        let mut i = 0;
        while i < token_factors.len() {
            // Limit the nodes to inline: the outstanding token factors join
            // the operands, or they would be dropped.
            if ops.len() > TOKEN_FACTOR_INLINE_LIMIT {
                ops.extend(token_factors[i..].iter().map(|&tf| SDValue { node: tf, res: 0 }));
                token_factors.truncate(i);
                break;
            }
            let current = token_factors[i];
            let current_ops = self.dag.node(SDValue { node: current, res: 0 }).ops.clone();
            for op in current_ops {
                match self.dag.opcode(op) {
                    // Entry tokens are redundant.
                    ISD::EntryToken => changed = true,
                    ISD::TokenFactor if self.dag.use_count(op.node) == 1 && !token_factors.contains(&op.node) => {
                        token_factors.push(op.node);
                        changed = true;
                    }
                    _ => {
                        if seen_ops.insert(op.node) {
                            ops.push(op);
                        } else {
                            changed = true;
                        }
                    }
                }
            }
            i += 1;
        }
        // Re-visit the inlined token factors, but not this one.
        for &tf in token_factors.iter().skip(1) {
            self.add_to_worklist(tf);
        }
        // Remove the nodes chained to another node in the list, by walking up
        // chains breadth first until another operand is seen.
        let mut prune = Prune {
            worklist: ops.iter().enumerate().map(|(number, op)| (op.node, number)).collect(),
            op_work_count: vec![1; ops.len()],
            seen_chains: HashSet::default(),
            seen_ops: &seen_ops,
            ops: &ops,
            changed,
            did_prune_ops: false,
            num_left_to_consider: ops.len() as i64,
        };
        let mut i = 0;
        while i < prune.worklist.len() && i < 1024 {
            // At least two operands are needed to prune.
            if prune.num_left_to_consider <= 1 {
                break;
            }
            let (current, op_number) = prune.worklist[i];
            let at = SDValue { node: current, res: 0 };
            match self.dag.opcode(at) {
                // Reaching the entry is the only way a search ends without
                // meeting another operand: do not count the operand done.
                ISD::EntryToken => prune.num_left_to_consider += 1,
                ISD::TokenFactor => {
                    for op in self.dag.node(at).ops.clone() {
                        prune.add(i, op.node, op_number);
                    }
                }
                ISD::CopyFromReg | ISD::CopyToReg => prune.add(i, self.dag.operand(at, 0).node, op_number),
                // A load or a store (`MemSDNode`).
                ISD::Load | ISD::Store => prune.add(i, self.dag.operand(at, 0).node, op_number),
                _ => {}
            }
            prune.op_work_count[op_number] -= 1;
            if prune.op_work_count[op_number] == 0 {
                prune.num_left_to_consider -= 1;
            }
            i += 1;
        }
        let (changed, did_prune_ops, seen_chains) = (prune.changed, prune.did_prune_ops, prune.seen_chains);
        if !changed {
            return None;
        }
        if ops.is_empty() {
            return Some(self.dag.entry_node());
        }
        if did_prune_ops {
            let kept: Vec<SDValue> = ops.iter().copied().filter(|op| !seen_chains.contains(&op.node)).collect();
            return Some(self.dag.get_token_factor(&kept));
        }
        Some(self.dag.get_token_factor(&ops))
    }
}

/// `GatherAllAliasesMaxDepth`.
const GATHER_ALL_ALIASES_MAX_DEPTH: usize = 6;

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
    fn gather_all_aliases(
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
    pub(super) fn visit_load(
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

    /// Entry tokens and repeated operands go; one operand left is that operand.
    #[test]
    fn a_token_factor_drops_entry_tokens_and_repeats() {
        let (dag, root) = combined(|dag| {
            let (entry, p) = (dag.entry_node(), dag.get_register(EVT::I32, 100));
            let load = dag.get_load(EVT::I32, entry, p, info());
            vec![entry, SDValue { node: load.node, res: 1 }, SDValue { node: load.node, res: 1 }]
        });
        assert_eq!(dag.opcode(root), ISD::Load, "{}", dag.dump());
    }

    /// A store whose chain is another operand makes that operand redundant.
    #[test]
    fn an_operand_another_operand_depends_on_is_dropped() {
        let (dag, root) = combined(|dag| {
            let (entry, p) = (dag.entry_node(), dag.get_register(EVT::I32, 100));
            let q = dag.get_register(EVT::I32, 101);
            let v = dag.get_constant(1, EVT::I32);
            let first = dag.get_store(entry, v, p, info());
            let second = dag.get_store(first, v, q, info());
            let other = dag.get_load(EVT::I32, entry, q, info());
            vec![first, second, SDValue { node: other.node, res: 1 }]
        });
        assert_eq!(dag.opcode(root), ISD::TokenFactor);
        let ops: Vec<ISD> = dag.node(root).ops.iter().map(|op| dag.opcode(*op)).collect();
        assert_eq!(ops, [ISD::Store, ISD::Load], "first is under second: {}", dag.dump());
    }

    /// A token factor only this one reads is taken in.
    #[test]
    fn a_nested_token_factor_is_flattened() {
        let (dag, root) = combined(|dag| {
            let (entry, p) = (dag.entry_node(), dag.get_register(EVT::I32, 100));
            let q = dag.get_register(EVT::I32, 101);
            let (a, b) = (dag.get_load(EVT::I32, entry, p, info()), dag.get_load(EVT::I32, entry, q, info()));
            let inner = dag.get_node(
                ISD::TokenFactor,
                EVT::OTHER,
                &[SDValue { node: a.node, res: 1 }, SDValue { node: b.node, res: 1 }],
            );
            let v = dag.get_constant(1, EVT::I32);
            let store = dag.get_store(entry, v, p, info());
            vec![inner, store]
        });
        let ops: Vec<ISD> = dag.node(root).ops.iter().map(|op| dag.opcode(*op)).collect();
        // The pin queues the inner factor and reads its operands last.
        assert_eq!(ops, [ISD::Store, ISD::Load, ISD::Load], "{}", dag.dump());
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
            vec![SDValue { node: load.node, res: 1 }]
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
}
