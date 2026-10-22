//! The type legalizer: every value in the DAG gets a type a register holds
//! (the pin's `DAGTypeLegalizer`).
//!
//! # How it works
//! Nodes are processed operands first. A node's id says where it is: waiting
//! on `n` operands, ready, processed, or new (made by legalization and not yet
//! looked at). A ready node whose result type is illegal is promoted (its
//! result becomes a wider integer) or expanded (two halves); else an illegal
//! operand type is dealt with the same way. When a node's results are
//! replaced, [`TypeLegalizer::replace_value_with`] makes the replacement
//! visible to every user and re-analyses the users that changed.
//! `promoted`, `expanded` and `replaced` are the pin's `PromotedIntegers`,
//! `ExpandedIntegers` and `ReplacedValues`.
//!
//! # Runs
//! After the first combine and before `LegalizeDAG`; not wired in yet (B1).
//! This piece has the machinery and the cases of `Constant` and the simple
//! integer binary operations; any other node with an illegal type is refused
//! by name ([`LegalizeError::Unsupported`]), and the cases are added in the
//! pin's order by later pieces.
//!
//! DEVIATE SD33: no `NodeUpdateListener`: [`SelectionDAG::replace_all_uses_with`]
//! returns what it did to each user ([`Rauw`]). Nodes replaced stay in the
//! arena with no users (no `RemoveDeadNodes`). Exit: none. Owner sdag-b.
//! Review 2026-11-13.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::run`
//! ([L196](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L196)),
//! `AnalyzeNewNode`
//! ([L558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L558)),
//! `ReplaceValueWith`
//! ([L636](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L636)).

use std::collections::{HashMap, HashSet};

use super::actions::TargetLowering;
use super::dag::{NodeData, Rauw, SDValue, SelectionDAG};
use super::isd::ISD;
use super::lowering::TypeAction;
use super::mvt::EVT;

/// A node's place in the walk (the pin's `NodeIdFlags` and its counts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeId {
    Unanalyzed,
    /// Made by legalization; not yet analysed.
    New,
    /// Waiting on this many unprocessed operands.
    Pending(u32),
    Ready,
    Processed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegalizeError {
    /// A node with an illegal type that no translated case handles.
    Unsupported { op: ISD, ty: EVT, what: &'static str },
}

pub struct TypeLegalizer<'a> {
    dag: &'a mut SelectionDAG,
    tli: &'a TargetLowering,
    ids: Vec<NodeId>,
    worklist: Vec<u32>,
    replaced: HashMap<SDValue, SDValue>,
    promoted: HashMap<SDValue, SDValue>,
    expanded: HashMap<SDValue, (SDValue, SDValue)>,
}

impl<'a> TypeLegalizer<'a> {
    pub fn new(dag: &'a mut SelectionDAG, tli: &'a TargetLowering) -> TypeLegalizer<'a> {
        TypeLegalizer { dag, tli, ids: Vec::new(), worklist: Vec::new(), replaced: HashMap::new(), promoted: HashMap::new(), expanded: HashMap::new() }
    }

    fn id(&self, node: u32) -> NodeId {
        self.ids.get(node as usize).copied().unwrap_or(NodeId::Unanalyzed)
    }

    fn set_id(&mut self, node: u32, id: NodeId) {
        if self.ids.len() <= node as usize {
            self.ids.resize(node as usize + 1, NodeId::Unanalyzed);
        }
        self.ids[node as usize] = id;
    }

    /// `IgnoreNodeResults`: constants and registers of a target have no type
    /// to legalize.
    fn ignores_results(&self, node: u32) -> bool {
        matches!(self.dag.opcode(SDValue { node, res: 0 }), ISD::TargetConstant | ISD::Register)
    }

    fn action(&self, vt: EVT) -> TypeAction {
        self.tli.types.type_action(vt)
    }

    /// `run`: legalize every type in the DAG. Returns whether anything changed.
    pub fn run(&mut self) -> Result<bool, LegalizeError> {
        let mut changed = false;
        for node in self.dag.node_ids() {
            if self.dag.node(SDValue { node, res: 0 }).ops.is_empty() {
                self.set_id(node, NodeId::Ready);
                self.worklist.push(node);
            } else {
                self.set_id(node, NodeId::Unanalyzed);
            }
        }
        while let Some(n) = self.worklist.pop() {
            debug_assert_eq!(self.id(n), NodeId::Ready, "a node on the worklist is ready");
            if !self.ignores_results(n) && self.legalize_results(n)? {
                changed = true;
            } else if self.legalize_operands(n)? {
                changed = true;
                if self.id(n) != NodeId::Ready {
                    continue;
                }
            }
            self.set_id(n, NodeId::Processed);
            self.release_users(n);
        }
        Ok(changed)
    }

    /// The first illegal result of `n` is promoted or expanded; true if one was.
    fn legalize_results(&mut self, n: u32) -> Result<bool, LegalizeError> {
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.clone();
        for (res, vt) in results.into_iter().enumerate() {
            let value = SDValue { node: n, res: res as u8 };
            match self.action(vt) {
                TypeAction::Legal => {}
                TypeAction::PromoteInteger => {
                    self.promote_integer_result(value)?;
                    return Ok(true);
                }
                TypeAction::ExpandInteger => {
                    self.expand_integer_result(value)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// The first illegal operand of `n` is dealt with. If `n` must be looked
    /// at again it is replaced by its re-analysed self and `n` is not done.
    fn legalize_operands(&mut self, n: u32) -> Result<bool, LegalizeError> {
        let ops = self.dag.node(SDValue { node: n, res: 0 }).ops.clone();
        for (at, op) in ops.into_iter().enumerate() {
            if self.ignores_results(op.node) {
                continue;
            }
            let reanalyze = match self.action(self.dag.value_type(op)) {
                TypeAction::Legal => continue,
                TypeAction::PromoteInteger => self.promote_integer_operand(n, at)?,
                TypeAction::ExpandInteger => self.expand_integer_operand(n, at)?,
            };
            if reanalyze {
                self.set_id(n, NodeId::New);
                let m = self.analyze_new_node(n);
                if m != n {
                    let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
                    for res in 0..results as u8 {
                        self.replace_value_with(SDValue { node: n, res }, SDValue { node: m, res });
                    }
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Each user of `n` has one operand fewer to wait for.
    fn release_users(&mut self, n: u32) {
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
        let mut users = Vec::new();
        for res in 0..results as u8 {
            users.extend(self.dag.users(SDValue { node: n, res }));
        }
        users.sort_unstable();
        users.dedup();
        for user in users {
            match self.id(user) {
                NodeId::Pending(count) => {
                    if count == 1 {
                        self.set_id(user, NodeId::Ready);
                        self.worklist.push(user);
                    } else {
                        self.set_id(user, NodeId::Pending(count - 1));
                    }
                }
                NodeId::New => {}
                NodeId::Unanalyzed => {
                    let operands = self.dag.node(SDValue { node: user, res: 0 }).ops.len() as u32;
                    if operands == 1 {
                        self.set_id(user, NodeId::Ready);
                        self.worklist.push(user);
                    } else {
                        self.set_id(user, NodeId::Pending(operands - 1));
                    }
                }
                NodeId::Ready | NodeId::Processed => {}
            }
        }
    }

    /// `AnalyzeNewNode`: look at a node made by legalization, after its
    /// operands. Returns the node to use in its place (its operands may have
    /// been replaced, which can make it equal to another node).
    fn analyze_new_node(&mut self, n: u32) -> u32 {
        if !matches!(self.id(n), NodeId::New | NodeId::Unanalyzed) {
            return n;
        }
        let original = self.dag.node(SDValue { node: n, res: 0 }).ops.clone();
        let mut ops = original.clone();
        let mut processed = 0;
        for op in ops.iter_mut() {
            *op = self.analyze_new_value(*op);
            if self.id(op.node) == NodeId::Processed {
                processed += 1;
            }
        }
        let mut n = n;
        if ops != original {
            let m = self.dag.update_node_operands(n, &ops);
            if m != n {
                self.set_id(n, NodeId::New);
                if !matches!(self.id(m), NodeId::New | NodeId::Unanalyzed) {
                    return m;
                }
                n = m;
            }
        }
        let waiting = ops.len() - processed;
        if waiting == 0 {
            self.set_id(n, NodeId::Ready);
            self.worklist.push(n);
        } else {
            self.set_id(n, NodeId::Pending(waiting as u32));
        }
        n
    }

    fn analyze_new_value(&mut self, value: SDValue) -> SDValue {
        let node = self.analyze_new_node(value.node);
        let value = SDValue { node, res: value.res };
        if self.id(node) == NodeId::Processed {
            self.remap(value)
        } else {
            value
        }
    }

    /// `RemapValue`: follow the replacements to the value now standing.
    fn remap(&self, mut value: SDValue) -> SDValue {
        while let Some(&next) = self.replaced.get(&value) {
            value = next;
        }
        value
    }

    /// `ReplaceValueWith`: `to` stands for `from` in every user; users that
    /// changed are analysed again.
    fn replace_value_with(&mut self, from: SDValue, to: SDValue) {
        assert_ne!(from.node, to.node, "potential legalization loop");
        let to = self.analyze_new_value(to);
        let mut pending: Vec<(SDValue, SDValue)> = vec![(from, to)];
        let mut to_analyze: Vec<u32> = Vec::new();
        let mut queued: HashSet<u32> = HashSet::new();
        loop {
            while let Some((from, to)) = pending.pop() {
                let (from, to) = (self.remap(from), self.remap(to));
                if from != to {
                    self.replaced.insert(from, to);
                }
                for event in self.dag.replace_all_uses_with(from, to) {
                    match event {
                        Rauw::Updated(user) => {
                            self.set_id(user, NodeId::New);
                            if queued.insert(user) {
                                to_analyze.push(user);
                            }
                        }
                        Rauw::Merged { from: gone, into } => {
                            queued.remove(&gone);
                            to_analyze.retain(|&queued_node| queued_node != gone);
                            if self.id(into) == NodeId::New && queued.insert(into) {
                                to_analyze.push(into);
                            }
                        }
                    }
                }
            }
            let Some(n) = to_analyze.pop() else { break };
            queued.remove(&n);
            if self.id(n) != NodeId::New {
                continue;
            }
            let m = self.analyze_new_node(n);
            if m != n {
                let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
                for res in 0..results as u8 {
                    let (old, mut new) = (SDValue { node: n, res }, SDValue { node: m, res });
                    if self.id(m) == NodeId::Processed {
                        new = self.remap(new);
                    }
                    pending.push((old, new));
                }
            }
        }
    }

    // --- promotion -------------------------------------------------------

    /// `GetPromotedInteger`.
    fn promoted_integer(&self, value: SDValue) -> SDValue {
        let value = self.remap(value);
        *self.promoted.get(&value).expect("operand promoted before its user")
    }

    /// `SetPromotedInteger`.
    fn set_promoted_integer(&mut self, value: SDValue, result: SDValue) {
        let value = self.remap(value);
        let result = self.analyze_new_value(result);
        self.promoted.insert(value, result);
    }

    /// `PromoteIntegerResult`.
    fn promote_integer_result(&mut self, value: SDValue) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let to = self.tli.types.type_conversion(vt).1;
        let result = match self.dag.opcode(value) {
            ISD::Constant => {
                // Zero extend an `i1`, sign extend everything else.
                let opcode = if vt.size_in_bits() % 8 == 0 { ISD::SignExtend } else { ISD::ZeroExtend };
                self.dag.get_node(opcode, to, &[value])
            }
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => {
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let (lhs, rhs) = (self.promoted_integer(lhs), self.promoted_integer(rhs));
                let wide = self.dag.value_type(lhs);
                self.dag.get_node(self.dag.opcode(value), wide, &[lhs, rhs])
            }
            ISD::Load => {
                // The access keeps its memory type; the result is wider
                // (an extending load), and the chain follows.
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else { unreachable!("a load has a memory operand") };
                let loaded = self.dag.get_load(to, chain, ptr, info);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: loaded.node, res: 1 });
                loaded
            }
            op => return Err(LegalizeError::Unsupported { op, ty: vt, what: "promote result" }),
        };
        self.set_promoted_integer(value, result);
        Ok(())
    }

    /// `PromoteIntegerOperand`: true if `n` was updated in place and must be
    /// looked at again.
    fn promote_integer_operand(&mut self, n: u32, at: usize) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let result = match self.dag.opcode(node) {
            ISD::Truncate => {
                let operand = self.promoted_integer(self.dag.operand(node, at));
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::Truncate, vt, &[operand])
            }
            op => return Err(LegalizeError::Unsupported { op, ty: self.dag.value_type(self.dag.operand(node, at)), what: "promote operand" }),
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }

    // --- expansion -------------------------------------------------------

    /// `GetExpandedInteger`: low and high halves.
    fn expanded_integer(&self, value: SDValue) -> (SDValue, SDValue) {
        let value = self.remap(value);
        *self.expanded.get(&value).expect("operand expanded before its user")
    }

    /// `SetExpandedInteger`.
    fn set_expanded_integer(&mut self, value: SDValue, lo: SDValue, hi: SDValue) {
        let value = self.remap(value);
        let (lo, hi) = (self.analyze_new_value(lo), self.analyze_new_value(hi));
        self.expanded.insert(value, (lo, hi));
    }

    /// `ExpandIntegerResult`.
    fn expand_integer_result(&mut self, value: SDValue) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let half = self.tli.types.type_conversion(vt).1;
        let (lo, hi) = match self.dag.opcode(value) {
            ISD::Constant => {
                let bits = self.dag.constant(value).expect("a constant has a value");
                let width = half.size_in_bits();
                let lo = self.dag.get_constant_with(bits, half, false, self.dag.is_opaque(value));
                let hi = self.dag.get_constant_with(bits >> width, half, false, self.dag.is_opaque(value));
                (lo, hi)
            }
            op @ (ISD::And | ISD::Or | ISD::Xor) => {
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
                (self.dag.get_node(op, half, &[ll, rl]), self.dag.get_node(op, half, &[lh, rh]))
            }
            op => return Err(LegalizeError::Unsupported { op, ty: vt, what: "expand result" }),
        };
        self.set_expanded_integer(value, lo, hi);
        Ok(())
    }

    /// `ExpandIntegerOperand`.
    fn expand_integer_operand(&mut self, n: u32, at: usize) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let result = match self.dag.opcode(node) {
            ISD::Truncate => {
                let (lo, _) = self.expanded_integer(self.dag.operand(node, at));
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::Truncate, vt, &[lo])
            }
            op => return Err(LegalizeError::Unsupported { op, ty: self.dag.value_type(self.dag.operand(node, at)), what: "expand operand" }),
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }

    /// True if no reachable node has an illegal result or operand type
    /// (the pin's final check in `run`).
    pub fn all_legal(&self) -> bool {
        let mut seen = HashSet::new();
        let mut stack = vec![self.dag.get_root().node];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            let node = self.dag.node(SDValue { node: n, res: 0 });
            if !self.ignores_results(n) && node.values.iter().any(|&vt| !self.tli.types.is_type_legal(vt)) {
                return false;
            }
            stack.extend(node.ops.iter().map(|op| op.node));
        }
        true
    }
}
