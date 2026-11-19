//! `visitTokenFactor`: a token factor loses its entry tokens and duplicate
//! operands, takes in the operands of a token factor only it reads, and drops
//! an operand another operand's chain already reaches.
//!
//! # See also
//! LLVM's `DAGCombiner::visitTokenFactor`
//! ([L2257](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2257)).

use super::dag::{NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;
use crate::support::hash::HashSet;

/// `TokenFactorInlineLimit` (`-combiner-tokenfactor-inline-limit`).
const INLINE_LIMIT: usize = 2048;
/// The chain walk's bound in the pin's pruning loop.
const WALK_LIMIT: usize = 1024;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `getInputChainForNode`: the first operand of type `Other`, else the
    /// last, else one in between.
    fn input_chain(
        &self,
        node: SDValue,
    ) -> Option<SDValue> {
        let ops = &self.dag.node(node).ops;
        let is_chain = |op: &SDValue| self.dag.value_type(*op) == EVT::OTHER;
        let (first, last) = (ops.first()?, ops.last()?);
        if is_chain(first) {
            return Some(*first);
        }
        if is_chain(last) {
            return Some(*last);
        }
        ops.get(1..ops.len() - 1)?.iter().copied().find(is_chain)
    }

    pub(super) fn visit_token_factor(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.opt_none {
            return None;
        }
        let ops = self.dag.node(node).ops.clone();
        if let [a, b] = ops[..] {
            if self.input_chain(a) == Some(b) {
                return Some(a);
            }
            if self.input_chain(b) == Some(a) {
                return Some(b);
            }
        }
        if ops.len() > INLINE_LIMIT {
            return None;
        }
        // "If the sole user is a token factor, make sure we have a chance to
        // merge them together."
        if self.dag.has_one_use(node.node) {
            let user = self.dag.users(node).next();
            if let Some(user) = user.filter(|&u| self.dag.opcode(SDValue { node: u, res: 0 }) == ISD::TokenFactor) {
                self.add_to_worklist(user);
            }
        }

        let mut token_factors = vec![node];
        let mut kept: Vec<SDValue> = Vec::new();
        let mut seen: HashSet<u32> = HashSet::default();
        let mut changed = false;
        let mut at = 0;
        while at < token_factors.len() {
            // "Limit number of nodes to inline, to avoid quadratic compile
            // times": the outstanding token factors stay operands.
            if kept.len() > INLINE_LIMIT {
                kept.extend(token_factors[at..].iter().map(|&tf| tf));
                token_factors.truncate(at);
                break;
            }
            let tf = token_factors[at];
            for op in self.dag.node(tf).ops.clone() {
                match self.dag.opcode(op) {
                    ISD::EntryToken => changed = true,
                    ISD::TokenFactor if self.dag.has_one_use(op.node) && !token_factors.contains(&op) => {
                        token_factors.push(op);
                        changed = true;
                    }
                    _ => {
                        if seen.insert(op.node) {
                            kept.push(op);
                        } else {
                            changed = true;
                        }
                    }
                }
            }
            at += 1;
        }
        for tf in &token_factors[1..] {
            self.add_to_worklist(tf.node);
        }

        // "Remove nodes that are chained to another node in the list": climb
        // each operand's chain breadth first until another operand is met.
        let mut work: Vec<(u32, usize)> = kept.iter().enumerate().map(|(i, op)| (op.node, i)).collect();
        let mut op_work: Vec<usize> = vec![1; kept.len()];
        let mut seen_chains: HashSet<u32> = HashSet::default();
        let mut pruned = false;
        // NumLeftToConsider is unsigned in the pin and wraps below zero.
        let mut left = kept.len();
        let mut i = 0;
        while i < work.len() && i < WALK_LIMIT {
            if left <= 1 {
                break;
            }
            let (current, number) = work[i];
            let at_node = SDValue { node: current, res: 0 };
            let next: Option<u32> = match self.dag.opcode(at_node) {
                ISD::EntryToken => {
                    left = left.wrapping_add(1);
                    None
                }
                ISD::TokenFactor => {
                    for op in self.dag.node(at_node).ops.clone() {
                        self.climb(
                            &mut work,
                            &mut op_work,
                            &mut seen_chains,
                            &seen,
                            &kept,
                            &mut left,
                            &mut changed,
                            &mut pruned,
                            i,
                            op.node,
                            number,
                        );
                    }
                    None
                }
                ISD::LifetimeStart | ISD::LifetimeEnd | ISD::CopyFromReg | ISD::CopyToReg => {
                    Some(self.dag.operand(at_node, 0).node)
                }
                _ => match self.dag.node(at_node).data {
                    NodeData::Mem(_) => Some(self.dag.operand(at_node, 0).node),
                    _ => None,
                },
            };
            if let Some(chain) = next {
                self.climb(
                    &mut work,
                    &mut op_work,
                    &mut seen_chains,
                    &seen,
                    &kept,
                    &mut left,
                    &mut changed,
                    &mut pruned,
                    i,
                    chain,
                    number,
                );
            }
            op_work[number] -= 1;
            if op_work[number] == 0 {
                left = left.wrapping_sub(1);
            }
            i += 1;
        }

        if !changed {
            return None;
        }
        if kept.is_empty() {
            return Some(self.dag.entry_node());
        }
        if pruned {
            kept.retain(|op| !seen_chains.contains(&op.node));
        }
        Some(self.dag.get_token_factor(&kept))
    }

    /// The pin's `AddToWorklist` lambda in the pruning walk: reaching another
    /// operand folds that operand's search into this one's.
    #[allow(clippy::too_many_arguments)]
    fn climb(
        &self,
        work: &mut Vec<(u32, usize)>,
        op_work: &mut [usize],
        seen_chains: &mut HashSet<u32>,
        seen_ops: &HashSet<u32>,
        kept: &[SDValue],
        left: &mut usize,
        changed: &mut bool,
        pruned: &mut bool,
        current: usize,
        op: u32,
        number: usize,
    ) {
        if seen_ops.contains(&op) {
            *changed = true;
            *pruned = true;
            let original = kept.iter().position(|k| k.node == op).expect("a token factor operand");
            for entry in work.iter_mut().skip(current + 1) {
                if entry.1 == original {
                    entry.1 = number;
                }
            }
            op_work[number] += op_work[original];
            op_work[original] = 0;
            *left = left.wrapping_sub(1);
        }
        if seen_chains.insert(op) {
            op_work[number] += 1;
            work.push((op, number));
        }
    }
}
