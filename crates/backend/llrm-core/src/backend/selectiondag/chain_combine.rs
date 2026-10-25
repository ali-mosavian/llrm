//! The combiner's work on chains: `visitTokenFactor` (the pin's
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
//! DEVIATE CC1: the pin skips all of this when optimising nothing
//! (`OptLevel == None`); no opt level reaches the combiner, so it runs at
//! every level. Exit: the level in `DagConfig`. Owner: w12.
//!
//! # Used by
//! `DagCombiner::visit`.

use llrm_support::hash::HashSet;

use super::dag::SDValue;
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
}
