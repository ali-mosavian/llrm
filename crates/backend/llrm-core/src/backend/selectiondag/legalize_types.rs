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
use super::dag::{BooleanContents, NodeData, Rauw, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::ISD;
use super::lowering::TypeAction;
use super::mvt::{EVT, MVT};
use super::x86_lowering::{x86isd, ES};

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
            ISD::SetCC => {
                // Compare into the target's own result type, then sign extend or
                // truncate to the promoted type.
                let ops = self.dag.node(value).ops.clone();
                let in_vt = self.dag.value_type(ops[0]);
                let wide = self.tli.types.set_cc_result_type(in_vt);
                let setcc = self.dag.get_node(ISD::SetCC, wide, &ops);
                if to.size_in_bits() > wide.size_in_bits() {
                    self.dag.get_node(ISD::SignExtend, to, &[setcc])
                } else if to.size_in_bits() < wide.size_in_bits() {
                    self.dag.get_node(ISD::Truncate, to, &[setcc])
                } else {
                    setcc
                }
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
            ISD::ZeroExtend => {
                // Extend the promoted source to the result type, then clear
                // what the source did not have (`getZeroExtendInReg`).
                let src = self.dag.operand(node, 0);
                let promoted = self.promoted_integer(src);
                let vt = self.dag.value_type(node);
                let wide = self.dag.get_node(ISD::AnyExtend, vt, &[promoted]);
                let mask = super::fold::mask(self.dag.value_type(src).size_in_bits());
                let mask = self.dag.get_constant(mask, vt);
                self.dag.get_node(ISD::And, vt, &[wide, mask])
            }
            ISD::Select if at == 0 => {
                // The condition becomes the target's boolean type (`PromoteTargetBoolean`).
                let (cond, then, other) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let bool_vt = self.tli.types.set_cc_result_type(self.dag.value_type(then));
                let extend = match self.dag.config().boolean_contents {
                    BooleanContents::ZeroOrOne => ISD::ZeroExtend,
                    BooleanContents::ZeroOrNegativeOne => ISD::SignExtend,
                    BooleanContents::Undefined => ISD::AnyExtend,
                };
                let cond = self.dag.get_node(extend, bool_vt, &[cond]);
                let updated = self.dag.update_node_operands(n, &[cond, then, other]);
                SDValue { node: updated, res: 0 }
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
            ISD::Load if vt == EVT::Simple(MVT::Far) => {
                // A far pointer is `les`: the offset, with the selector left in
                // `ES` and read back on the chain (SD2).
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else { unreachable!("a load has a memory operand") };
                let les = self.dag.intern(x86isd::LES, &[half, EVT::OTHER], &[chain, ptr], SDNodeFlags::default(), NodeData::Mem(info));
                let selector = self.dag.get_copy_from_reg(SDValue { node: les.node, res: 1 }, ES, half);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: selector.node, res: 1 });
                (les, selector)
            }
            op @ (ISD::Shl | ISD::Srl | ISD::Sra) => self.expand_shift(value, op, vt, half)?,
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

    /// `ExpandIntRes_Shift`: by a constant, or as the target's double-width
    /// shift (`SHL_PARTS`, ...). `ExpandShiftWithKnownAmountBit` needs known
    /// bits (SD202) and the library calls are not translated.
    fn expand_shift(&mut self, value: SDValue, op: ISD, vt: EVT, half: EVT) -> Result<(SDValue, SDValue), LegalizeError> {
        let (input, amount) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        if let Some(amt) = self.dag.constant(amount) {
            return Ok(self.expand_shift_by_constant(op, input, amt, vt, half));
        }
        let parts = match op {
            ISD::Shl => ISD::ShlParts,
            ISD::Srl => ISD::SrlParts,
            _ => ISD::SraParts,
        };
        let action = self.tli.operation_action(parts, half);
        if !(action == crate::backend::selectiondag::Action::Custom || self.tli.is_operation_legal(parts, half)) {
            return Err(LegalizeError::Unsupported { op, ty: vt, what: "expand shift to a library call" });
        }
        let (lo, hi) = self.expanded_integer(input);
        let shift_vt = EVT::I8;
        let amount = match self.dag.value_type(amount).size_in_bits().cmp(&8) {
            std::cmp::Ordering::Greater => self.dag.get_node(ISD::Truncate, shift_vt, &[amount]),
            std::cmp::Ordering::Less => self.dag.get_node(ISD::ZeroExtend, shift_vt, &[amount]),
            std::cmp::Ordering::Equal => amount,
        };
        let both = self.dag.get_node_multi(parts, &[half, half], &[lo, hi, amount]);
        Ok((both, SDValue { node: both.node, res: 1 }))
    }

    /// `ExpandShiftByConstant`.
    fn expand_shift_by_constant(&mut self, op: ISD, input: SDValue, amt: u128, vt: EVT, half: EVT) -> (SDValue, SDValue) {
        let (in_lo, in_hi) = self.expanded_integer(input);
        if amt == 0 {
            return (in_lo, in_hi);
        }
        let vt_bits = u128::from(vt.size_in_bits());
        let n = u128::from(half.size_in_bits());
        let by = |dag: &mut SelectionDAG, k: u128| dag.get_constant(k, EVT::I8);
        let zero = self.dag.get_constant(0, half);
        let funnel_legal = |legalizer: &Self, op: ISD| legalizer.tli.is_operation_legal(op, half);
        match op {
            ISD::Shl => {
                if amt >= vt_bits {
                    (zero, zero)
                } else if amt > n {
                    let k = by(self.dag, amt - n);
                    (zero, self.dag.get_node(ISD::Shl, half, &[in_lo, k]))
                } else if amt == n {
                    (zero, in_lo)
                } else {
                    let k = by(self.dag, amt);
                    let lo = self.dag.get_node(ISD::Shl, half, &[in_lo, k]);
                    let hi = if funnel_legal(self, ISD::FShl) {
                        self.dag.get_node(ISD::FShl, half, &[in_hi, in_lo, k])
                    } else {
                        let up = self.dag.get_node(ISD::Shl, half, &[in_hi, k]);
                        let back = by(self.dag, n - amt);
                        let down = self.dag.get_node(ISD::Srl, half, &[in_lo, back]);
                        self.dag.get_node(ISD::Or, half, &[up, down])
                    };
                    (lo, hi)
                }
            }
            ISD::Srl => {
                if amt >= vt_bits {
                    (zero, zero)
                } else if amt > n {
                    let k = by(self.dag, amt - n);
                    (self.dag.get_node(ISD::Srl, half, &[in_hi, k]), zero)
                } else if amt == n {
                    (in_hi, zero)
                } else {
                    let k = by(self.dag, amt);
                    let lo = if funnel_legal(self, ISD::FShr) {
                        self.dag.get_node(ISD::FShr, half, &[in_hi, in_lo, k])
                    } else {
                        let down = self.dag.get_node(ISD::Srl, half, &[in_lo, k]);
                        let back = by(self.dag, n - amt);
                        let up = self.dag.get_node(ISD::Shl, half, &[in_hi, back]);
                        self.dag.get_node(ISD::Or, half, &[down, up])
                    };
                    (lo, self.dag.get_node(ISD::Srl, half, &[in_hi, k]))
                }
            }
            _ => {
                let sign = by(self.dag, n - 1);
                if amt >= vt_bits {
                    let fill = self.dag.get_node(ISD::Sra, half, &[in_hi, sign]);
                    (fill, fill)
                } else if amt > n {
                    let k = by(self.dag, amt - n);
                    (self.dag.get_node(ISD::Sra, half, &[in_hi, k]), self.dag.get_node(ISD::Sra, half, &[in_hi, sign]))
                } else if amt == n {
                    (in_hi, self.dag.get_node(ISD::Sra, half, &[in_hi, sign]))
                } else {
                    let k = by(self.dag, amt);
                    let lo = if funnel_legal(self, ISD::FShr) {
                        self.dag.get_node(ISD::FShr, half, &[in_hi, in_lo, k])
                    } else {
                        let down = self.dag.get_node(ISD::Srl, half, &[in_lo, k]);
                        let back = by(self.dag, n - amt);
                        let up = self.dag.get_node(ISD::Shl, half, &[in_hi, back]);
                        self.dag.get_node(ISD::Or, half, &[down, up])
                    };
                    (lo, self.dag.get_node(ISD::Sra, half, &[in_hi, k]))
                }
            }
        }
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
