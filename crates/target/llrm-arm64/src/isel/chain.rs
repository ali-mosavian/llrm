//! A branch on `a || b || ...` or `a && b && ...` of integer comparisons is a
//! chain of conditional branches, one per comparison, not a bit made of each
//! (`SelectionDAGBuilder::FindMergedConditions`, same-operator chains).
//!
//! The comparisons and the `or`/`and`s joining them are selected where the
//! branch is. `select a, b, false` and `select a, true, b`, LLVM's logical
//! `and` and `or` (`m_LogicalAnd`, `m_LogicalOr`), join like the instructions.
//! A mix of `or` and `and` is not a chain: it needs a block per subtree, and
//! stays a value.

use llrm_mir::dense::IdSet;
use llrm_mir::{BinaryOp, InstId, Opcode, Operand, ValueId};

use super::Selector;

/// The comparisons of one chain, in program order, and what joins them.
#[derive(Clone)]
pub(super) struct Chain {
    pub(super) or: bool,
    pub(super) leaves: Vec<ValueId>,
}

/// The chains of a function: by their root's value, and every value in any.
#[derive(Default)]
pub(super) struct Chains {
    pub(super) roots: llrm_mir::dense::IdMap<ValueId, Chain>,
    members: IdSet<ValueId>,
}

impl Chains {
    pub(super) fn contains(
        &self,
        value: ValueId,
    ) -> bool {
        self.members.contains(&value)
    }
}

impl Selector<'_> {
    /// Finds the chains: each conditional `br` whose condition is a chain.
    pub(super) fn find_chains(&mut self) {
        let mut chains = Chains::default();
        let mut found = Vec::new();
        for (_, inst) in self.function.walk() {
            let branch = self.function.instruction(inst);
            if branch.opcode == Opcode::Br
                && branch.operands.len() == 3
                && let Operand::Value(condition) = branch.operands[0]
                && let Some(chain) = self.chain_of(condition, inst)
            {
                found.push((condition, chain));
            }
        }
        for (root, chain) in found {
            let mut members = Vec::new();
            self.collect(root, chain.or, &mut members);
            for member in members {
                chains.members.insert(member);
            }
            chains.roots.insert(root, chain);
        }
        self.chains = chains;
    }

    /// Whether `value` is a one-use `or`/`and` of i1 read by `reader` in its
    /// own block, over comparisons and chains of the same operator.
    fn chain_of(
        &self,
        value: ValueId,
        reader: InstId,
    ) -> Option<Chain> {
        let (op, inst) = self.joiner(value, reader)?;
        let mut leaves = Vec::new();
        self.leaves_of(inst, op == BinaryOp::Or, &mut leaves).then_some(Chain { or: op == BinaryOp::Or, leaves })
    }

    /// `value` as an i1 `or`/`and`, or a logical one, with one use, `reader`,
    /// in the same block.
    fn joiner(
        &self,
        value: ValueId,
        reader: InstId,
    ) -> Option<(BinaryOp, InstId)> {
        let inst = super::value_def(self.function, value)?;
        let (op, _) = self.joined(inst)?;
        (self.use_count(value) == 1
            && self.types().int_bits(self.function.instruction(inst).ty) == Some(1)
            && self.function.parent(inst) == self.function.parent(reader))
        .then_some((op, inst))
    }

    /// What `inst` joins and how: an `or`/`and`, or a `select` whose false arm
    /// is `false` (`and` of its condition and true arm) or whose true arm is
    /// `true` (`or` of its condition and false arm).
    pub(super) fn joined(
        &self,
        inst: InstId,
    ) -> Option<(BinaryOp, [Operand; 2])> {
        let instruction = self.function.instruction(inst);
        match (&instruction.opcode, instruction.operands.as_slice()) {
            (Opcode::Binary(op @ (BinaryOp::Or | BinaryOp::And)), &[a, b]) => Some((*op, [a, b])),
            (Opcode::Select, &[condition, on_true, on_false]) if self.is_zero_constant(on_false) => {
                Some((BinaryOp::And, [condition, on_true]))
            }
            (Opcode::Select, &[condition, on_true, on_false]) if self.is_true_constant(on_true) => {
                Some((BinaryOp::Or, [condition, on_false]))
            }
            _ => None,
        }
    }

    fn is_true_constant(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Constant(id) if matches!(self.module.context.get(id).kind, llrm_mir::ConstantKind::Int(1))
        )
    }

    fn leaves_of(
        &self,
        joiner: InstId,
        or: bool,
        leaves: &mut Vec<ValueId>,
    ) -> bool {
        let operator = if or { BinaryOp::Or } else { BinaryOp::And };
        let Some((_, operands)) = self.joined(joiner) else { return false };
        for operand in operands {
            let Operand::Value(value) = operand else { return false };
            if let Some((op, inner)) = self.joiner(value, joiner) {
                if op != operator || !self.leaves_of(inner, or, leaves) {
                    return false;
                }
                continue;
            }
            let Some(inst) = super::value_def(self.function, value) else { return false };
            let leaf = self.function.instruction(inst);
            if !(matches!(leaf.opcode, Opcode::ICmp(_))
                && self.use_count(value) == 1
                && self.function.parent(inst) == self.function.parent(joiner))
            {
                return false;
            }
            leaves.push(value);
        }
        true
    }

    /// Every value of the chain under `value`: its joiners and comparisons.
    fn collect(
        &self,
        value: ValueId,
        or: bool,
        into: &mut Vec<ValueId>,
    ) {
        into.push(value);
        let Some(inst) = super::value_def(self.function, value) else { return };
        if let Some((_, operands)) = self.joined(inst) {
            for operand in operands {
                if let Operand::Value(inner) = operand {
                    self.collect(inner, or, into);
                }
            }
        }
    }
}
