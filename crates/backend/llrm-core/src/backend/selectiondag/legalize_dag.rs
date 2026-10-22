//! Operation legalization: every node's operation is one the target can
//! select (the pin's `SelectionDAGLegalize`).
//!
//! # How it works
//! Types are already legal ([`super::legalize_types`]). The walk visits nodes
//! users first, so a node is seen with its original operands: the nodes
//! reachable from the root in reverse topological order. For each,
//! [`DagLegalizer::legalize_op`] asks the action table. `Legal` keeps the
//! node. `Custom` asks the target ([`Hooks::lower_operation`]); if it
//! declines the node is expanded. `Expand` is `expand_node`, and what that
//! cannot do goes to the library (`convert_node_to_libcall`). `Promote` makes
//! the node in the wider type and truncates the result. Nodes made on the way
//! are legalized by the next pass, until a pass legalizes nothing.
//!
//! # Runs
//! After the type legalizer and before the third combine; not wired in yet.
//! This piece has the driver, `Promote` of the simple integer operations, and
//! no `Expand` or library cases; a node that needs one is refused by name
//! ([`LegalizeError::Unsupported`]) until B1b and B1e.
//!
//! DEVIATE SD41: no `DAGNodeDeletedListener` and no dead-node deletion
//! (`DeleteNode`, `RemoveDeadNodes`): the arena keeps replaced nodes, and the
//! walk skips a node nothing reads. Exit: none. Owner sdag-b. Review
//! 2026-11-13.
//!
//! # See also
//! LLVM's `SelectionDAG::Legalize`
//! ([L6301](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L6301)),
//! `LegalizeOp`
//! ([L982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L982)),
//! `PromoteNode`
//! ([L5432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L5432)).

use std::collections::HashSet;

use super::actions::{Action, TargetLowering};
use super::dag::{SDValue, SelectionDAG};
use super::isd::ISD;
use super::legalize_types::LegalizeError;
use super::mvt::EVT;

/// What the target supplies (`TLI.LowerOperation`).
pub trait Hooks {
    /// The legal replacement of `node` (its first result), or `None` to have
    /// the node expanded instead.
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue>;
}

/// A target with no custom lowering.
pub struct NoHooks;

impl Hooks for NoHooks {
    fn lower_operation(
        &mut self,
        _dag: &mut SelectionDAG,
        _node: SDValue,
    ) -> Option<SDValue> {
        None
    }
}

pub struct DagLegalizer<'a, H: Hooks> {
    dag: &'a mut SelectionDAG,
    tli: &'a TargetLowering,
    hooks: &'a mut H,
    legalized: HashSet<u32>,
}

impl<'a, H: Hooks> DagLegalizer<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        tli: &'a TargetLowering,
        hooks: &'a mut H,
    ) -> DagLegalizer<'a, H> {
        DagLegalizer { dag, tli, hooks, legalized: HashSet::new() }
    }

    /// `SelectionDAG::Legalize`.
    pub fn run(&mut self) -> Result<(), LegalizeError> {
        loop {
            let mut any = false;
            for n in self.reverse_topological() {
                if self.is_dead(n) || !self.legalized.insert(n) {
                    continue;
                }
                any = true;
                self.legalize_op(n)?;
            }
            if !any {
                return Ok(());
            }
        }
    }

    /// The nodes reachable from the root, users before their operands.
    fn reverse_topological(&self) -> Vec<u32> {
        let mut order = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![(self.dag.get_root().node, false)];
        while let Some((n, expanded)) = stack.pop() {
            if expanded {
                order.push(n);
                continue;
            }
            if !seen.insert(n) {
                continue;
            }
            stack.push((n, true));
            for op in &self.dag.node(SDValue { node: n, res: 0 }).ops {
                stack.push((op.node, false));
            }
        }
        order.reverse();
        order
    }

    /// A node nothing reads that is not the root.
    fn is_dead(
        &self,
        n: u32,
    ) -> bool {
        if n == self.dag.get_root().node {
            return false;
        }
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.len() as u8;
        (0..results).all(|res| self.dag.users(SDValue { node: n, res }).next().is_none())
    }

    /// The type the action table is asked about for `value`'s node (the
    /// switch at the top of `LegalizeOp`).
    fn action_type(
        &self,
        node: SDValue,
    ) -> EVT {
        match self.dag.opcode(node) {
            ISD::Br | ISD::BrCond | ISD::BrCC | ISD::BrJT => EVT::OTHER,
            ISD::SetCC | ISD::Store => {
                self.dag.value_type(self.dag.operand(node, if self.dag.opcode(node) == ISD::Store { 1 } else { 0 }))
            }
            _ => self.dag.value_type(node),
        }
    }

    /// `LegalizeOp`.
    fn legalize_op(
        &mut self,
        n: u32,
    ) -> Result<(), LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        if matches!(self.dag.opcode(node), ISD::TargetConstant | ISD::Register) {
            return Ok(());
        }
        let vt = self.action_type(node);
        let mut action = match vt {
            EVT::Simple(_) => self.tli.operation_action(self.dag.opcode(node), vt),
            EVT::Int(_) => Action::Expand,
        };
        if action == Action::Custom {
            if let Some(result) = self.hooks.lower_operation(self.dag, node) {
                if result != node {
                    // A node with several results is replaced result by result
                    // (the node of `MERGE_VALUES` the target returns).
                    let results = self.dag.node(node).values.len() as u8;
                    if results > 1 && self.dag.node(result).values.len() == results as usize {
                        for res in 0..results {
                            self.replace_node(SDValue { node: n, res }, SDValue { node: result.node, res });
                        }
                    } else {
                        self.replace_node(node, result);
                    }
                }
                return Ok(());
            }
            action = Action::Expand;
        }
        match action {
            Action::Legal => Ok(()),
            Action::Promote => self.promote_node(node),
            Action::Expand => match self.expand_node(node) {
                Some(results) => {
                    for (res, new) in results.into_iter().enumerate() {
                        self.replace_node(SDValue { node: node.node, res: res as u8 }, new);
                    }
                    Ok(())
                }
                None => Err(LegalizeError::Unsupported {
                    op: self.dag.opcode(node),
                    ty: vt,
                    what: "convert node to libcall",
                }),
            },
            Action::LibCall | Action::Custom => {
                Err(LegalizeError::Unsupported { op: self.dag.opcode(node), ty: vt, what: "expand node" })
            }
        }
    }

    /// `ExpandNode`, for the cases translated: the replacement for each result
    /// of `node`, or `None` if it has to become a library call.
    fn expand_node(
        &mut self,
        node: SDValue,
    ) -> Option<Vec<SDValue>> {
        let op = self.dag.opcode(node);
        let vt = self.dag.value_type(node);
        let ready = |legalizer: &Self, op: ISD| legalizer.tli.is_operation_legal_or_custom(op, vt);
        match op {
            ISD::UDiv | ISD::SDiv => {
                let divrem = if op == ISD::SDiv { ISD::SDivRem } else { ISD::UDivRem };
                if !ready(self, divrem) {
                    return None;
                }
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                Some(vec![self.dag.get_node_multi(divrem, &[vt, vt], &[a, b])])
            }
            ISD::URem | ISD::SRem => {
                // `TargetLowering::expandREM`.
                let (divrem, div) = if op == ISD::SRem { (ISD::SDivRem, ISD::SDiv) } else { (ISD::UDivRem, ISD::UDiv) };
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                if ready(self, divrem) {
                    let both = self.dag.get_node_multi(divrem, &[vt, vt], &[a, b]);
                    return Some(vec![SDValue { node: both.node, res: 1 }]);
                }
                if ready(self, div) {
                    let quotient = self.dag.get_node(div, vt, &[a, b]);
                    let product = self.dag.get_node(ISD::Mul, vt, &[quotient, b]);
                    return Some(vec![self.dag.get_node(ISD::Sub, vt, &[a, product])]);
                }
                None
            }
            ISD::MulHU | ISD::MulHS => {
                let lohi = if op == ISD::MulHU { ISD::UMulLoHi } else { ISD::SMulLoHi };
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let both = self.dag.get_node_multi(lohi, &[vt, vt], &[a, b]);
                Some(vec![SDValue { node: both.node, res: 1 }])
            }
            ISD::BSwap => self.expand_bswap(node).map(|result| vec![result]),
            _ => None,
        }
    }

    /// `TargetLowering::expandBSWAP`, for `i16` (a rotate by 8). Other widths
    /// are legal on the targets here or not yet translated.
    fn expand_bswap(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if vt != EVT::I16 {
            return None;
        }
        let amount = self.dag.get_constant(8, EVT::I8);
        Some(self.dag.get_node(ISD::Rotl, vt, &[self.dag.operand(node, 0), amount]))
    }

    /// `ReplaceNode(Old, New)` for a node with one value result.
    fn replace_node(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        self.dag.replace_all_uses_with(old, new);
    }

    /// `PromoteNode`, for the simple integer operations: make the node in the
    /// wider type from any-extended operands and truncate the result.
    fn promote_node(
        &mut self,
        node: SDValue,
    ) -> Result<(), LegalizeError> {
        let op = self.dag.opcode(node);
        let EVT::Simple(from) = self.dag.value_type(node) else { unreachable!("promotion applies to simple types") };
        let to: EVT = self.tli.type_to_promote_to(op, from).into();
        let result = match op {
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => {
                let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let lhs = self.dag.get_node(ISD::AnyExtend, to, &[lhs]);
                let rhs = self.dag.get_node(ISD::AnyExtend, to, &[rhs]);
                let wide = self.dag.get_node(op, to, &[lhs, rhs]);
                self.dag.get_node(ISD::Truncate, from.into(), &[wide])
            }
            _ => return Err(LegalizeError::Unsupported { op, ty: from.into(), what: "promote node" }),
        };
        self.replace_node(node, result);
        Ok(())
    }
}
