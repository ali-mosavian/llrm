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
//! DEVIATE SD209: a load or store through a far pointer sets `ES` with a
//! `CopyToReg` before it, ordered by the chain only: the access has no glue
//! operand, so two far accesses unordered by the chain could interleave their
//! `ES` writes. Exit: a glued far-access node. Owner sdag-b. Review 2026-12-10.
//!
//! DEVIATE SD33: no `NodeUpdateListener`:
//! [`SelectionDAG::replace_all_uses_with`] returns what it did to each user
//! ([`Rauw`]). Nodes replaced stay in the arena with no users (no
//! `RemoveDeadNodes`). Exit: none. Owner sdag-b. Review 2026-11-13.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::run`
//! ([L196](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L196)),
//! `AnalyzeNewNode`
//! ([L558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L558)),
//! `ReplaceValueWith`
//! ([L636](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeTypes.cpp#L636)).

use llrm_mir::hash::HashSet;
use llrm_support::hash::HashMap;
use llrm_sdag::lowering::ShiftAmount;

use super::actions::TargetLowering;
use super::dag::{BooleanContents, NodeData, Rauw, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::lowering::TypeAction;
use super::mvt::{EVT, MVT};
use super::x86_lowering::{ES, x86isd};

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
    /// The target whose routines a division calls (`None`: no library calls).
    target: Option<&'a dyn llrm_target::Target>,
    /// The target's custom lowerings, asked before a node with an illegal type
    /// is promoted or expanded (`CustomLowerNode`).
    hooks: Option<&'a mut dyn super::legalize_dag::Hooks>,
}

impl<'a> TypeLegalizer<'a> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        tli: &'a TargetLowering,
    ) -> TypeLegalizer<'a> {
        TypeLegalizer {
            dag,
            tli,
            ids: Vec::new(),
            worklist: Vec::new(),
            replaced: HashMap::default(),
            promoted: HashMap::default(),
            expanded: HashMap::default(),
            target: None,
            hooks: None,
        }
    }

    /// The target's custom lowerings for the types it cannot hold
    /// (`CustomLowerNode`).
    pub fn with_hooks(
        mut self,
        hooks: &'a mut dyn super::legalize_dag::Hooks,
    ) -> TypeLegalizer<'a> {
        self.hooks = Some(hooks);
        self
    }

    /// The target whose inline routines `i64` division calls (SD19).
    pub fn with_target(
        mut self,
        target: &'a dyn llrm_target::Target,
    ) -> TypeLegalizer<'a> {
        self.target = Some(target);
        self
    }

    fn id(
        &self,
        node: u32,
    ) -> NodeId {
        self.ids.get(node as usize).copied().unwrap_or(NodeId::Unanalyzed)
    }

    fn set_id(
        &mut self,
        node: u32,
        id: NodeId,
    ) {
        if self.ids.len() <= node as usize {
            self.ids.resize(node as usize + 1, NodeId::Unanalyzed);
        }
        self.ids[node as usize] = id;
    }

    /// `IgnoreNodeResults`: constants and registers of a target have no type
    /// to legalize.
    fn ignores_results(
        &self,
        node: u32,
    ) -> bool {
        matches!(
            self.dag.opcode(SDValue { node, res: 0 }),
            ISD::TargetConstant | ISD::Register
        )
    }

    fn action(
        &self,
        vt: EVT,
    ) -> TypeAction {
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
            self.dag.cur_order = self.dag.node(SDValue { node: n, res: 0 }).order;
            if !self.ignores_results(n) && self.legalize_results(n)? {
                changed = true;
            } else if self.legalize_operands(n)? {
                changed = true;
                // A node updated in place was analyzed again and put back on
                // the worklist (`NeedsReanalyzing` ends the
                // pin's iteration): marking it processed here as well
                // left a processed node on the worklist.
                if self.id(n) != NodeId::Ready || self.worklist.contains(&n) {
                    continue;
                }
            }
            self.set_id(n, NodeId::Processed);
            self.release_users(n);
        }
        // What the debug records of an expanded value name: its two halves.
        for (value, halves) in &self.expanded {
            self.dag.expanded_integers.insert(*value, *halves);
        }
        Ok(changed)
    }

    /// The first illegal result of `n` is promoted or expanded; true if one
    /// was.
    fn legalize_results(
        &mut self,
        n: u32,
    ) -> Result<bool, LegalizeError> {
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.clone();
        for (res, vt) in results.into_iter().enumerate() {
            let value = SDValue { node: n, res: res as u8 };
            match self.action(vt) {
                TypeAction::Legal => {}
                TypeAction::PromoteInteger => {
                    if !self.custom_lower_node(n, vt) {
                        self.promote_integer_result(value)?;
                    }
                    return Ok(true);
                }
                TypeAction::ExpandInteger => {
                    if !self.custom_lower_node(n, vt) {
                        self.expand_integer_result(value)?;
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// The first illegal operand of `n` is dealt with. If `n` must be looked
    /// at again it is replaced by its re-analysed self and `n` is not done.
    fn legalize_operands(
        &mut self,
        n: u32,
    ) -> Result<bool, LegalizeError> {
        let ops = self.dag.node(SDValue { node: n, res: 0 }).ops.clone();
        for (at, op) in ops.into_iter().enumerate() {
            if self.ignores_results(op.node) {
                continue;
            }
            let operand_vt = self.dag.value_type(op);
            let reanalyze = match self.action(operand_vt) {
                TypeAction::Legal => continue,
                _ if self.custom_lower_node(n, operand_vt) => return Ok(true),
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
    fn release_users(
        &mut self,
        n: u32,
    ) {
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
        let mut users = Vec::new();
        for res in 0..results as u8 {
            users.extend(self.dag.users(SDValue { node: n, res }));
        }
        users.sort_unstable();
        users.dedup();
        for user in users {
            // One operand slot per read of `n`, whichever of its results.
            let slots =
                self.dag.node(SDValue { node: user, res: 0 }).ops.iter().filter(|op| op.node == n).count() as u32;
            match self.id(user) {
                NodeId::Pending(count) => {
                    if count <= slots {
                        self.set_id(user, NodeId::Ready);
                        self.worklist.push(user);
                    } else {
                        self.set_id(user, NodeId::Pending(count - slots));
                    }
                }
                NodeId::New => {}
                NodeId::Unanalyzed => {
                    let operands = self.dag.node(SDValue { node: user, res: 0 }).ops.len() as u32;
                    if operands <= slots {
                        self.set_id(user, NodeId::Ready);
                        self.worklist.push(user);
                    } else {
                        self.set_id(user, NodeId::Pending(operands - slots));
                    }
                }
                NodeId::Ready | NodeId::Processed => {}
            }
        }
    }

    /// `AnalyzeNewNode`: look at a node made by legalization, after its
    /// operands. Returns the node to use in its place (its operands may have
    /// been replaced, which can make it equal to another node).
    fn analyze_new_node(
        &mut self,
        n: u32,
    ) -> u32 {
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

    fn analyze_new_value(
        &mut self,
        value: SDValue,
    ) -> SDValue {
        let node = self.analyze_new_node(value.node);
        let value = SDValue { node, res: value.res };
        if self.id(node) == NodeId::Processed { self.remap(value) } else { value }
    }

    /// `RemapValue`: follow the replacements to the value now standing.
    fn remap(
        &self,
        mut value: SDValue,
    ) -> SDValue {
        while let Some(&next) = self.replaced.get(&value) {
            value = next;
        }
        value
    }

    /// `ReplaceValueWith`: `to` stands for `from` in every user; users that
    /// changed are analysed again.
    fn replace_value_with(
        &mut self,
        from: SDValue,
        to: SDValue,
    ) {
        assert_ne!(from.node, to.node, "potential legalization loop");
        let to = self.analyze_new_value(to);
        let mut pending: Vec<(SDValue, SDValue)> = vec![(from, to)];
        let mut to_analyze: Vec<u32> = Vec::new();
        let mut queued: HashSet<u32> = HashSet::default();
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

    /// A constant shift amount of `count` for a shift of `shifted`, in the
    /// target's shift-amount type (`getShiftAmountConstant`).
    fn shift_amount(
        &mut self,
        shifted: EVT,
        count: u32,
    ) -> SDValue {
        let ty = match self.tli.types.shift_amount() {
            ShiftAmount::I8 => EVT::I8,
            ShiftAmount::ByValue => shifted,
            ShiftAmount::Fixed(vt) => EVT::Simple(vt),
        };
        self.dag.get_constant(u128::from(count), ty)
    }

    /// `GetPromotedInteger`.
    fn promoted_integer(
        &self,
        value: SDValue,
    ) -> SDValue {
        let value = self.remap(value);
        *self.promoted.get(&value).expect("operand promoted before its user")
    }

    /// `SetPromotedInteger`.
    fn set_promoted_integer(
        &mut self,
        value: SDValue,
        result: SDValue,
    ) {
        let value = self.remap(value);
        let result = self.analyze_new_value(result);
        self.promoted.insert(value, result);
    }

    /// `PromoteIntegerResult`.
    fn promote_integer_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let to = self.tli.types.type_conversion(vt).1;
        let result = match self.dag.opcode(value) {
            ISD::Constant => {
                // Zero extend an `i1`, sign extend everything else.
                let opcode = if vt.size_in_bits() % 8 == 0 { ISD::SignExtend } else { ISD::ZeroExtend };
                self.dag.get_node(opcode, to, &[value])
            }
            // `PromoteIntRes_UNDEF`: an undef of the promoted type.
            ISD::Undef | ISD::Poison => self.dag.get_undef(to),
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => {
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let (lhs, rhs) = (self.promoted_integer(lhs), self.promoted_integer(rhs));
                let wide = self.dag.value_type(lhs);
                self.dag.get_node(self.dag.opcode(value), wide, &[lhs, rhs])
            }
            op @ (ISD::SMin | ISD::SMax | ISD::UMin | ISD::UMax) => {
                // `PromoteIntRes_SimpleIntBinOp` with `SExtPromotedInteger` for
                // the signed forms and `ZExtPromotedInteger` for the unsigned.
                let signed = matches!(op, ISD::SMin | ISD::SMax);
                let extend = |legalizer: &mut Self, operand: SDValue| {
                    let bits = legalizer.dag.value_type(operand).size_in_bits();
                    let promoted = legalizer.promoted_integer(operand);
                    let wide = legalizer.dag.value_type(promoted);
                    if signed {
                        let by = legalizer.shift_amount(wide, wide.size_in_bits() - bits);
                        let high = legalizer.dag.get_node(ISD::Shl, wide, &[promoted, by]);
                        legalizer.dag.get_node(ISD::Sra, wide, &[high, by])
                    } else {
                        let mask = legalizer.dag.get_constant(super::fold::mask(bits), wide);
                        legalizer.dag.get_node(ISD::And, wide, &[promoted, mask])
                    }
                };
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let (lhs, rhs) = (extend(self, lhs), extend(self, rhs));
                let wide = self.dag.value_type(lhs);
                self.dag.get_node(op, wide, &[lhs, rhs])
            }
            op @ (ISD::Shl | ISD::Srl | ISD::Sra) => {
                // `PromoteIntRes_SHL/SRL/SRA`: the value promoted, with the
                // bits above its width cleared (srl) or sign-filled (sra); a
                // promoted shift amount is zero extended.
                let (value, amount) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let bits = self.dag.value_type(value).size_in_bits();
                let promoted = self.promoted_integer(value);
                let wide = self.dag.value_type(promoted);
                let lhs = match op {
                    ISD::Shl => promoted,
                    ISD::Srl => {
                        let mask = self.dag.get_constant(super::fold::mask(bits), wide);
                        self.dag.get_node(ISD::And, wide, &[promoted, mask])
                    }
                    _ => {
                        let by = self.shift_amount(wide, wide.size_in_bits() - bits);
                        let high = self.dag.get_node(ISD::Shl, wide, &[promoted, by]);
                        self.dag.get_node(ISD::Sra, wide, &[high, by])
                    }
                };
                let amount = match self.action(self.dag.value_type(amount)) {
                    TypeAction::PromoteInteger => {
                        let amount_bits = self.dag.value_type(amount).size_in_bits();
                        let promoted = self.promoted_integer(amount);
                        let wide = self.dag.value_type(promoted);
                        let mask = self.dag.get_constant(super::fold::mask(amount_bits), wide);
                        self.dag.get_node(ISD::And, wide, &[promoted, mask])
                    }
                    _ => amount,
                };
                self.dag.get_node(op, wide, &[lhs, amount])
            }
            ISD::Select => {
                // `PromoteIntRes_SELECT`: the arms promoted, the condition as
                // is.
                let (cond, then, other) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let (then, other) = (self.promoted_integer(then), self.promoted_integer(other));
                let wide = self.dag.value_type(then);
                self.dag.get_node(ISD::Select, wide, &[cond, then, other])
            }
            ISD::Freeze => {
                // `PromoteIntRes_FREEZE`: the promoted operand, frozen.
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                let wide = self.dag.value_type(promoted);
                self.dag.get_node(ISD::Freeze, wide, &[promoted])
            }
            ISD::SetCC => {
                // Compare into the target's own result type, then sign extend
                // or truncate to the promoted type.
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
            ISD::Truncate => {
                // `PromoteIntRes_TRUNCATE`: truncate the (promoted) input to
                // the promoted type.
                let input = self.dag.operand(value, 0);
                let source = match self.action(self.dag.value_type(input)) {
                    TypeAction::PromoteInteger => self.promoted_integer(input),
                    _ => input,
                };
                self.dag.get_node(ISD::Truncate, to, &[source])
            }
            op @ (ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend) => {
                // `PromoteIntRes_INT_EXTEND`: a promoted source already has the
                // result width if it was promoted all the way;
                // extend it in the register.
                let input = self.dag.operand(value, 0);
                let from = self.dag.value_type(input);
                let mut made = None;
                if self.action(from) == TypeAction::PromoteInteger {
                    let promoted = self.promoted_integer(input);
                    if self.dag.value_type(promoted) == to {
                        made = Some(match op {
                            ISD::SignExtend => {
                                let ty = self.dag.get_value_type_node(from);
                                self.dag.get_node(ISD::SignExtendInReg, to, &[promoted, ty])
                            }
                            ISD::ZeroExtend => {
                                let mask = self.dag.get_constant(super::fold::mask(from.size_in_bits()), to);
                                self.dag.get_node(ISD::And, to, &[promoted, mask])
                            }
                            _ => promoted,
                        });
                    }
                }
                match made {
                    Some(result) => result,
                    None => self.dag.get_node(op, to, &[input]),
                }
            }
            ISD::Load => {
                // The access keeps its memory type; the result is wider
                // (an extending load), and the chain follows.
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("a load has a memory operand")
                };
                if let Some((round, extra)) = odd_memory(info.mem_vt) {
                    // `PromoteIntRes_LOAD` of memory that is not a power of
                    // two wide (`{ i64, i48 }`) is `ExpandLoad`: the low part
                    // zero-extended, the rest above it.
                    let low = self
                        .dag
                        .get_ext_load(
                            super::actions::LoadExt::ZExtLoad,
                            to,
                            chain,
                            ptr,
                            super::dag::MemInfo { mem_vt: round, ..info },
                        );
                    let high_ptr = self.address_plus(ptr, u128::from(round.size_in_bits() / 8));
                    let high_ptr_load = self.dag.get_ext_load(
                        super::actions::LoadExt::ZExtLoad,
                        to,
                        chain,
                        high_ptr,
                        super::dag::MemInfo { mem_vt: extra, align: info.align.min(round.size_in_bits() / 8), ..info },
                    );
                    let by = self.dag.get_constant(u128::from(round.size_in_bits()), EVT::I8);
                    let high = self.dag.get_node(ISD::Shl, to, &[high_ptr_load, by]);
                    let chains = self
                        .dag
                        .get_token_factor(
                            &[
                                SDValue { node: low.node, res: 1 },
                                SDValue { node: high_ptr_load.node, res: 1 },
                            ],
                        );
                    self.replace_value_with(SDValue { node: value.node, res: 1 }, chains);
                    let joined = self.dag.get_node(ISD::Or, to, &[low, high]);
                    self.set_promoted_integer(value, joined);
                    return Ok(());
                }
                let loaded = self.dag.get_load(to, chain, ptr, info);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: loaded.node, res: 1 });
                loaded
            }
            ISD::SDivFix => {
                // `PromoteIntRes_DIVFIX`: the operands sign-extended in the
                // promoted type, then `expandFixedPointDiv` there when the
                // headroom allows.
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let (lhs, rhs) = (self.promoted_integer(lhs), self.promoted_integer(rhs));
                let from = self.dag.value_type(self.dag.operand(value, 0));
                let ty = self.dag.get_value_type_node(from);
                let lhs = self.dag.get_node(ISD::SignExtendInReg, to, &[lhs, ty]);
                let rhs = self.dag.get_node(ISD::SignExtendInReg, to, &[rhs, ty]);
                let scale = self.dag.constant(self.dag.operand(value, 2)).unwrap_or(0) as u32;
                match self.expand_fixed_point_div(lhs, rhs, scale) {
                    Some(result) => result,
                    None => {
                        return Err(LegalizeError::Unsupported {
                            op: ISD::SDivFix,
                            ty: vt,
                            what: "promote result: no headroom for the scale (early expand not translated)",
                        });
                    }
                }
            }
            ISD::UAddO
            | ISD::USubO
            | ISD::UAddOCarry
            | ISD::USubOCarry
            | ISD::SAddO
            | ISD::SSubO
            | ISD::UMulO
            | ISD::SMulO
                if value.res == 1 =>
            {
                // `PromoteIntRes_Overflow`: the node again with the target's
                // boolean type for the flag; the sum is switched to the new
                // node and the flag is the promoted value.
                let sum_vt = self.dag.value_type(SDValue { node: value.node, res: 0 });
                let flag_vt = self.tli.types.set_cc_result_type(sum_vt);
                let ops = self.dag.node(value).ops.clone();
                let node = self.dag.get_node_multi(self.dag.opcode(value), &[sum_vt, flag_vt], &ops);
                self.replace_value_with(SDValue { node: value.node, res: 0 }, SDValue { node: node.node, res: 0 });
                SDValue { node: node.node, res: 1 }
            }
            op => return Err(LegalizeError::Unsupported { op, ty: vt, what: "promote result" }),
        };
        self.set_promoted_integer(value, result);
        Ok(())
    }

    /// `PromoteIntegerOperand`: true if `n` was updated in place and must be
    /// looked at again.
    fn promote_integer_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
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
            ISD::AnyExtend => {
                // `PromoteIntOp_ANY_EXTEND`.
                let promoted = self.promoted_integer(self.dag.operand(node, 0));
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::AnyExtend, vt, &[promoted])
            }
            ISD::SignExtend => {
                // `PromoteIntOp_SIGN_EXTEND`.
                let src = self.dag.operand(node, 0);
                let promoted = self.promoted_integer(src);
                let vt = self.dag.value_type(node);
                let wide = self.dag.get_node(ISD::AnyExtend, vt, &[promoted]);
                let inner = self.dag.get_value_type_node(self.dag.value_type(src));
                self.dag.get_node(ISD::SignExtendInReg, vt, &[wide, inner])
            }
            ISD::Store
                if at == 1
                    && matches!(
                        self.dag.node(node).data,
                        NodeData::Mem(info) if odd_memory(info.mem_vt).is_some()
                    ) =>
            {
                // `PromoteIntOp_STORE` of memory that is not a power of two
                // wide (`{ i64, i48 }`): the truncating store the pin expands
                // to a store of the low part and one of the rest above it.
                let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let (round, extra) = odd_memory(info.mem_vt).unwrap_or((info.mem_vt, info.mem_vt));
                let promoted = self.promoted_integer(self.dag.operand(node, 1));
                let wide = self.dag.value_type(promoted);
                let low = self.dag.get_node(ISD::Truncate, round, &[promoted]);
                let by = self.dag.get_constant(u128::from(round.size_in_bits()), EVT::I8);
                let upper = self.dag.get_node(ISD::Srl, wide, &[promoted, by]);
                let upper = self.dag.get_node(ISD::Truncate, extra, &[upper]);
                let high_ptr = self.address_plus(ptr, u128::from(round.size_in_bits() / 8));
                let low_store = self.dag.get_store(chain, low, ptr, super::dag::MemInfo { mem_vt: round, ..info });
                let high_info =
                    super::dag::MemInfo { mem_vt: extra, align: info.align.min(round.size_in_bits() / 8), ..info };
                let high_store = self.dag.get_store(chain, upper, high_ptr, high_info);
                self.dag.get_token_factor(&[low_store, high_store])
            }
            ISD::Select if at == 0 => {
                // The condition becomes the target's boolean type
                // (`PromoteTargetBoolean`).
                let (cond, then, other) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
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
            ISD::SetCC | ISD::SelectCC if at < 2 => {
                // `PromoteSetCCOperands` (and `PromoteIntOp_SELECT_CC`): both
                // operands promoted and extended
                // in the register to the compare's signedness.
                let Some(code) = self.setcc_code(node) else {
                    return Err(LegalizeError::Unsupported {
                        op: ISD::SetCC,
                        ty: self.dag.value_type(self.dag.operand(node, at)),
                        what: "promote operand",
                    });
                };
                let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let extend = |legalizer: &mut Self, value: SDValue| {
                    let bits = legalizer.dag.value_type(value).size_in_bits();
                    let promoted = legalizer.promoted_integer(value);
                    let wide = legalizer.dag.value_type(promoted);
                    if code.is_signed_int() {
                        let by = legalizer.shift_amount(wide, wide.size_in_bits() - bits);
                        let high = legalizer.dag.get_node(ISD::Shl, wide, &[promoted, by]);
                        legalizer.dag.get_node(ISD::Sra, wide, &[high, by])
                    } else {
                        let mask = legalizer.dag.get_constant(super::fold::mask(bits), wide);
                        legalizer.dag.get_node(ISD::And, wide, &[promoted, mask])
                    }
                };
                let (lhs, rhs) = (extend(self, lhs), extend(self, rhs));
                let mut ops: Vec<SDValue> = (0..if self.dag.opcode(node) == ISD::SelectCC { 5 } else { 3 })
                    .map(|i| self.dag.operand(node, i))
                    .collect();
                (ops[0], ops[1]) = (lhs, rhs);
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
            ISD::UAddOCarry | ISD::USubOCarry if at == 2 => {
                // `PromoteIntOp_ADDSUBO_CARRY`: the carry in as the target's
                // boolean type (`PromoteTargetBoolean`).
                let (lhs, rhs, carry) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let bool_vt = self.tli.types.set_cc_result_type(self.dag.value_type(lhs));
                let extend = match self.dag.config().boolean_contents {
                    BooleanContents::ZeroOrOne => ISD::ZeroExtend,
                    BooleanContents::ZeroOrNegativeOne => ISD::SignExtend,
                    BooleanContents::Undefined => ISD::AnyExtend,
                };
                let carry = self.dag.get_node(extend, bool_vt, &[carry]);
                let updated = self.dag.update_node_operands(n, &[lhs, rhs, carry]);
                SDValue { node: updated, res: 0 }
            }
            ISD::BrCond if at == 1 => {
                // `PromoteIntOp_BRCOND`: the condition to the target's boolean
                // type.
                let (chain, cond, dest) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let bool_vt = self.tli.types.set_cc_result_type(EVT::OTHER);
                let extend = match self.dag.config().boolean_contents {
                    BooleanContents::ZeroOrOne => ISD::ZeroExtend,
                    BooleanContents::ZeroOrNegativeOne => ISD::SignExtend,
                    BooleanContents::Undefined => ISD::AnyExtend,
                };
                let cond = self.dag.get_node(extend, bool_vt, &[cond]);
                let updated = self.dag.update_node_operands(n, &[chain, cond, dest]);
                SDValue { node: updated, res: 0 }
            }
            ISD::Store
                if at == 1
                    && self.dag.value_type(self.dag.operand(node, 1)).is_integer()
                    && self.dag.value_type(self.dag.operand(node, 1)).size_in_bits() % 8 == 0
                    && !self.dag.value_type(self.dag.operand(node, 1)).size_in_bits().is_power_of_two() =>
            {
                // `PromoteIntOp_STORE` and the pin's split of a truncating
                // store of an odd width (`LegalizeDAG`): the promoted value
                // stored in power-of-two pieces, the low one first.
                let (chain, value, ptr) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let promoted = self.promoted_integer(value);
                let wide = self.dag.value_type(promoted);
                let (mut left, mut at_bit) = (self.dag.value_type(value).size_in_bits(), 0);
                let mut stores = Vec::new();
                while left > 0 {
                    let piece = 1 << left.ilog2();
                    let vt = EVT::integer(piece);
                    let shifted = if at_bit == 0 {
                        promoted
                    } else {
                        let by = self.shift_amount(wide, at_bit);
                        self.dag.get_node(ISD::Srl, wide, &[promoted, by])
                    };
                    let part = self.dag.get_node(ISD::Truncate, vt, &[shifted]);
                    let at = self.address_plus(ptr, u128::from(at_bit / 8));
                    stores.push(self.dag.get_store(chain, part, at, super::dag::MemInfo { mem_vt: vt, ..info }));
                    at_bit += piece;
                    left -= piece;
                }
                self.dag.get_token_factor(&stores)
            }
            ISD::Store
                if at == 1
                    && self.dag.value_type(self.dag.operand(node, 1)).is_integer()
                    && self.dag.value_type(self.dag.operand(node, 1)).size_in_bits() % 8 == 0 =>
            {
                // `PromoteIntOp_STORE`: a truncating store of the promoted value.
                let (chain, value, ptr) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let promoted = self.promoted_integer(value);
                self.dag.get_store(chain, promoted, ptr, info)
            }
            op => {
                return Err(LegalizeError::Unsupported {
                    op,
                    ty: self.dag.value_type(self.dag.operand(node, at)),
                    what: "promote operand",
                });
            }
        };
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }

    // --- expansion -------------------------------------------------------

    /// `GetExpandedInteger`: low and high halves.
    fn expanded_integer(
        &self,
        value: SDValue,
    ) -> (SDValue, SDValue) {
        let value = self.remap(value);
        *self.expanded.get(&value).expect("operand expanded before its user")
    }

    /// `SetExpandedInteger`.
    fn set_expanded_integer(
        &mut self,
        value: SDValue,
        lo: SDValue,
        hi: SDValue,
    ) {
        let value = self.remap(value);
        let (lo, hi) = (self.analyze_new_value(lo), self.analyze_new_value(hi));
        self.expanded.insert(value, (lo, hi));
    }

    /// `ExpandIntegerResult`.
    fn expand_integer_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
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
            op @ (ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend) => self.expand_extend(value, op, half),
            ISD::SignExtendInReg => {
                // `ExpandIntRes_SIGN_EXTEND_INREG`: within the low half, extend
                // there and fill the high half with its sign; above it, extend
                // the high half in its register.
                let NodeData::ValueType(inner) = self.dag.node(self.dag.operand(value, 1)).data else {
                    return Err(LegalizeError::Unsupported { op: ISD::SignExtendInReg, ty: vt, what: "expand result" });
                };
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                if inner.size_in_bits() <= half.size_in_bits() {
                    let ty = self.dag.get_value_type_node(inner);
                    let lo = self.dag.get_node(ISD::SignExtendInReg, half, &[lo, ty]);
                    let by = self.dag.get_constant(u128::from(half.size_in_bits() - 1), EVT::I8);
                    (lo, self.dag.get_node(ISD::Sra, half, &[lo, by]))
                } else {
                    let rest = EVT::integer(inner.size_in_bits() - half.size_in_bits());
                    let ty = self.dag.get_value_type_node(rest);
                    (lo, self.dag.get_node(ISD::SignExtendInReg, half, &[hi, ty]))
                }
            }
            ISD::Load if vt == EVT::Simple(MVT::Far) && !self.addresses_memory(value) => self.expand_load(value, half),
            ISD::Load if vt == EVT::Simple(MVT::Far) => {
                // A far pointer is `les`: the offset, with the selector left in
                // `ES` and read back on the chain (SD2).
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("a load has a memory operand")
                };
                let les = self
                    .dag
                    .intern(
                        x86isd::LES,
                        &[half, EVT::OTHER],
                        &[chain, ptr],
                        SDNodeFlags::default(),
                        NodeData::Mem(info),
                    );
                let selector = self.dag.get_copy_from_reg(SDValue { node: les.node, res: 1 }, ES, half);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: selector.node, res: 1 });
                (les, selector)
            }
            ISD::Load => self.expand_load(value, half),
            x86isd::HUGE_ADD if vt == EVT::Simple(MVT::Far) => self.expand_huge_add(value, half)?,
            x86isd::LLRM_WINDOW if vt == EVT::Simple(MVT::Far) => self.expand_window(value, half)?,
            ISD::BuildPair => (self.dag.operand(value, 0), self.dag.operand(value, 1)),
            op @ (ISD::Undef | ISD::Poison) => {
                // `ExpandIntRes_UNDEF`: an undef (poison) of each half.
                let make =
                    |dag: &mut SelectionDAG| if op == ISD::Undef { dag.get_undef(half) } else { dag.get_poison(half) };
                (make(self.dag), make(self.dag))
            }
            ISD::Select => {
                // `ExpandIntRes_SELECT`: one select per half.
                let (cond, then, other) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let ((tl, th), (fl, fh)) = (self.expanded_integer(then), self.expanded_integer(other));
                (self.dag.get_select(half, cond, tl, fl), self.dag.get_select(half, cond, th, fh))
            }
            ISD::BitCast if self.dag.value_type(self.dag.operand(value, 0)).is_floating_point() => {
                // `ExpandRes_BITCAST` of a float: stored to a stack slot, the
                // halves loaded from it.
                let src = self.dag.operand(value, 0);
                let (slot, store, cell) = self.store_to_slot(src);
                let info = super::dag::MemInfo {
                    mem_vt: half,
                    align: 4.min(half.size_in_bits() / 8),
                    ..self.slot_info(src, cell)
                };
                let step = u128::from(half.size_in_bits() / 8);
                let pointer = self.dag.value_type(slot);
                let offset = self.dag.get_constant(step, pointer);
                let high_slot = self.dag.get_node(ISD::Add, pointer, &[slot, offset]);
                (self.dag.get_load(half, store, slot, info), self.dag.get_load(half, store, high_slot, info))
            }
            ISD::BitCast if self.tli.types.is_type_legal(self.dag.value_type(self.dag.operand(value, 0))) => {
                // `ExpandIntRes_BITCAST` from a same-size integer: its low
                // word and the word above it.
                let src = self.dag.operand(value, 0);
                let svt = self.dag.value_type(src);
                let shift = self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
                let high = self.dag.get_node(ISD::Srl, svt, &[src, shift]);
                (self.dag.get_node(ISD::Truncate, half, &[src]), self.dag.get_node(ISD::Truncate, half, &[high]))
            }
            ISD::CopyFromReg if vt == EVT::Simple(MVT::Far) => {
                // A far pointer a call returns in the register pair (`dx:ax`,
                // named by the dword register): read it as the dword and take
                // it apart as a bitcast does.
                let mut values = self.dag.node(value).values.clone();
                values[0] = EVT::I32;
                let mut ops = self.dag.node(value).ops.clone();
                if let NodeData::PhysReg(reg) = self.dag.node(ops[1]).data {
                    ops[1] = self.dag.get_phys_register(EVT::I32, reg);
                }
                let dword = self.dag.get_node_multi(ISD::CopyFromReg, &values, &ops);
                for res in 1..values.len() as u8 {
                    self.replace_value_with(SDValue { node: value.node, res }, SDValue { node: dword.node, res });
                }
                let shift = self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
                let high = self.dag.get_node(ISD::Srl, EVT::I32, &[dword, shift]);
                (self.dag.get_node(ISD::Truncate, half, &[dword]), self.dag.get_node(ISD::Truncate, half, &[high]))
            }
            ISD::Freeze => {
                // `SplitRes_FREEZE`: each half frozen.
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                (self.dag.get_node(ISD::Freeze, half, &[lo]), self.dag.get_node(ISD::Freeze, half, &[hi]))
            }
            ISD::CtPop => self.expand_ctpop(value, half),
            // `ExpandIntRes_BITREVERSE`: each half reversed, and swapped.
            ISD::BitReverse => {
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                (self.dag.get_node(ISD::BitReverse, half, &[hi]), self.dag.get_node(ISD::BitReverse, half, &[lo]))
            }
            op @ (ISD::CtLz | ISD::CtLzZeroPoison) => self.expand_ctlz(value, op, half),
            op @ (ISD::CtTz | ISD::CtTzZeroPoison) => self.expand_cttz(value, op, half),
            op @ (ISD::Add | ISD::Sub) => self.expand_add_sub(value, op, half)?,
            op @ (ISD::UAddO | ISD::USubO) => self.expand_addsub_overflow(value, op, half)?,
            op @ (ISD::SAddO | ISD::SSubO) => self.expand_signed_addsub_overflow(value, op, half),
            ISD::SMulFix | ISD::UMulFix | ISD::SMulFixSat | ISD::UMulFixSat => self.expand_mulfix(value, half),
            ISD::SShlSat | ISD::UShlSat => {
                // `ExpandIntRes_SHLSAT`.
                let expanded = super::expand_fixed::expand_shl_sat(self.dag, self.tli, value);
                self.split_integer(expanded, half)
            }
            ISD::SAddSat | ISD::UAddSat | ISD::SSubSat | ISD::USubSat => {
                // `ExpandIntRes_ADDSUBSAT`.
                let expanded = super::expand_sat::expand_add_sub_sat(self.dag, self.tli, value);
                self.split_integer(expanded, half)
            }
            ISD::UMulO => self.expand_umulo(value, half),
            ISD::SMulO => self.expand_smulo(value, half),
            op @ (ISD::SMin | ISD::SMax | ISD::UMin | ISD::UMax) => {
                // `ExpandIntRes_MINMAX`: the high halves' own min/max, and the
                // low half of the winning high half, or the unsigned min/max
                // of the low halves when the high halves are equal.
                let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
                let (cc, low) = match op {
                    ISD::SMin => (CondCode::SetLt, ISD::UMin),
                    ISD::SMax => (CondCode::SetGt, ISD::UMax),
                    ISD::UMin => (CondCode::SetULt, ISD::UMin),
                    _ => (CondCode::SetUGt, ISD::UMax),
                };
                let cc_vt = self.tli.types.set_cc_result_type(half);
                let hi = self.dag.get_node(op, half, &[lh, rh]);
                let equal = self.dag.get_setcc(cc_vt, lh, rh, CondCode::SetEq);
                let wins = self.dag.get_setcc(cc_vt, lh, rh, cc);
                let winner = self.dag.get_select(half, wins, ll, rl);
                let min_max = self.dag.get_node(low, half, &[ll, rl]);
                (self.dag.get_select(half, equal, min_max, winner), hi)
            }
            ISD::Mul => self.expand_mul(value, half)?,
            op @ (ISD::SDiv | ISD::UDiv | ISD::SRem | ISD::URem) => {
                let by_constant = (matches!(op, ISD::UDiv | ISD::URem)
                    && self.dag.constant(self.dag.operand(value, 1)).is_some())
                .then(|| {
                    let (ll, lh) = self.expanded_integer(self.dag.operand(value, 0));
                    super::expand_divrem_const::expand_divrem_by_constant(self.dag, self.tli, value, half, ll, lh)
                })
                .flatten();
                match by_constant {
                    Some(halves) => halves,
                    None => self.expand_division(value, op, half)?,
                }
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
    /// shift (`SHL_PARTS`, ...). The library calls are not translated.
    fn expand_shift(
        &mut self,
        value: SDValue,
        op: ISD,
        vt: EVT,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
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
        if let Some(pair) = self.expand_shift_with_known_amount_bit(op, (lo, hi), amount, half) {
            return Ok(pair);
        }
        let both = self.dag.get_node_multi(parts, &[half, half], &[lo, hi, amount]);
        Ok((both, SDValue { node: both.node, res: 1 }))
    }

    /// `ExpandShiftWithKnownAmountBit`: bit `log2(half)` of the amount is
    /// known, so the shift is one half-width shift (set) or a shift and a
    /// funnel shift (clear). DEVIATE SD241: a clear bit without a legal or
    /// custom funnel shift stays a parts node (the pin shifts, subtracts and
    /// selects). Exit: with the generic `Amt == 0` expansion. Owner sdag-b.
    /// Review 2026-12-10.
    fn expand_shift_with_known_amount_bit(
        &mut self,
        op: ISD,
        (lo, hi): (SDValue, SDValue),
        amount: SDValue,
        half: EVT,
    ) -> Option<(SDValue, SDValue)> {
        let high = 1u128 << half.size_in_bits().ilog2();
        let known = super::known_bits::known_bits(self.dag, amount);
        let (set, clear) = (known.one & high != 0, known.zero & high != 0);
        let funnel = if op == ISD::Shl { ISD::FShl } else { ISD::FShr };
        if !set && !(clear && self.tli.is_operation_legal_or_custom(funnel, half)) {
            return None;
        }
        if set {
            let keep = self.dag.get_constant(!high & 0xFF, EVT::I8);
            let rest = self.dag.get_node(ISD::And, EVT::I8, &[amount, keep]);
            return Some(match op {
                ISD::Shl => (self.dag.get_constant(0, half), self.dag.get_node(ISD::Shl, half, &[lo, rest])),
                ISD::Srl => (self.dag.get_node(ISD::Srl, half, &[hi, rest]), self.dag.get_constant(0, half)),
                _ => {
                    let sign = self.dag.get_constant(u128::from(half.size_in_bits() - 1), EVT::I8);
                    (self.dag.get_node(ISD::Sra, half, &[hi, rest]), self.dag.get_node(ISD::Sra, half, &[hi, sign]))
                }
            });
        }
        Some(match op {
            ISD::Shl => (
                self.dag.get_node(ISD::Shl, half, &[lo, amount]),
                self.dag.get_node(ISD::FShl, half, &[hi, lo, amount]),
            ),
            _ => (
                self.dag.get_node(ISD::FShr, half, &[hi, lo, amount]),
                self.dag.get_node(if op == ISD::Srl { ISD::Srl } else { ISD::Sra }, half, &[hi, amount]),
            ),
        })
    }

    /// `ExpandShiftByConstant`.
    fn expand_shift_by_constant(
        &mut self,
        op: ISD,
        input: SDValue,
        amt: u128,
        vt: EVT,
        half: EVT,
    ) -> (SDValue, SDValue) {
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

    /// `ExpandIntRes_ADDSUB`, the carry forms: the low halves with an overflow
    /// result, the high halves with that carry in. The ADDC/ADDE form (glue)
    /// and the compare-for-carry form are not translated.
    fn expand_add_sub(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let (lo_op, carry_op, plain) = if op == ISD::Add {
            (ISD::UAddO, ISD::UAddOCarry, ISD::Add)
        } else {
            (ISD::USubO, ISD::USubOCarry, ISD::Sub)
        };
        if !self.tli.is_operation_legal_or_custom(carry_op, half) {
            return Err(LegalizeError::Unsupported {
                op,
                ty: self.dag.value_type(value),
                what: "expand add or subtract without a carry operation",
            });
        }
        let overflow_vt = self.tli.types.set_cc_result_type(half);
        let lo = self.dag.get_node_multi(lo_op, &[half, overflow_vt], &[ll, rl]);
        let carry = SDValue { node: lo.node, res: 1 };
        // A constant-zero carry needs no carry operation (`computeKnownBits`,
        // SD202).
        let hi = if self.dag.constant(carry) == Some(0) {
            self.dag.get_node(plain, half, &[lh, rh])
        } else {
            self.dag.get_node_multi(carry_op, &[half, overflow_vt], &[lh, rh, carry])
        };
        Ok((lo, hi))
    }

    /// `ExpandIntRes_UADDSUBO`: the low halves' overflow form, the high halves
    /// with its carry in, whose overflow is the whole one. Without a carry
    /// operation the sum is the plain one, and overflow is `sum < lhs` (`sum >
    /// lhs` for a subtract).
    fn expand_addsub_overflow(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let (carry_op, plain, cc) = if op == ISD::UAddO {
            (ISD::UAddOCarry, ISD::Add, CondCode::SetULt)
        } else {
            (ISD::USubOCarry, ISD::Sub, CondCode::SetUGt)
        };
        let overflow_vt = self.dag.node(value).values[1];
        let (lo, hi, overflow) = if self.tli.is_operation_legal_or_custom(carry_op, half) {
            let lo = self.dag.get_node_multi(op, &[half, overflow_vt], &[ll, rl]);
            let hi =
                self.dag.get_node_multi(carry_op, &[half, overflow_vt], &[lh, rh, SDValue { node: lo.node, res: 1 }]);
            (lo, SDValue { node: hi.node, res: 0 }, SDValue { node: hi.node, res: 1 })
        } else {
            let wide = self.dag.value_type(value);
            let sum = self.dag.get_node(plain, wide, &[lhs, rhs]);
            let (lo, hi) = self.split_integer(sum, half);
            (lo, hi, self.dag.get_setcc(overflow_vt, sum, lhs, cc))
        };
        self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
        Ok((lo, hi))
    }

    /// `ExpandIntRes_SADDSUBO` without a signed carry operation: the plain sum
    /// or difference, and the overflow as the sign of `(~(l ^ r) & (l ^ sum))`
    /// for an add, `((l ^ r) & (l ^ sum))` for a subtract.
    fn expand_signed_addsub_overflow(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let wide = self.dag.value_type(value);
        let is_add = op == ISD::SAddO;
        let sum = self.dag.get_node(if is_add { ISD::Add } else { ISD::Sub }, wide, &[lhs, rhs]);
        let (lo, hi) = self.split_integer(sum, half);
        let mut signs_match = self.dag.get_node(ISD::Xor, wide, &[lhs, rhs]);
        if is_add {
            signs_match = self.dag.get_not(signs_match, wide);
        }
        let sum_sign_ne = self.dag.get_node(ISD::Xor, wide, &[lhs, sum]);
        let both = self.dag.get_node(ISD::And, wide, &[signs_match, sum_sign_ne]);
        let zero = self.dag.get_constant(0, wide);
        let overflow_vt = self.dag.node(value).values[1];
        let overflow = self.dag.get_setcc(overflow_vt, both, zero, CondCode::SetLt);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
        (lo, hi)
    }

    /// `ExpandIntRes_XMULO` for the unsigned multiply: the product of the low
    /// halves, the two cross products (each with its own overflow) added into
    /// the high half, and overflow when both high halves are set.
    fn expand_umulo(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let wide = self.dag.value_type(value);
        let bit = self.dag.node(value).values[1];
        let zero = self.dag.get_constant(0, half);
        let lh_set = self.dag.get_setcc(bit, lh, zero, CondCode::SetNe);
        let rh_set = self.dag.get_setcc(bit, rh, zero, CondCode::SetNe);
        let mut overflow = self.dag.get_node(ISD::And, bit, &[lh_set, rh_set]);
        let one = self.dag.get_node_multi(ISD::UMulO, &[half, bit], &[lh, rl]);
        overflow = self.dag.get_node(ISD::Or, bit, &[overflow, SDValue { node: one.node, res: 1 }]);
        let two = self.dag.get_node_multi(ISD::UMulO, &[half, bit], &[rh, ll]);
        overflow = self.dag.get_node(ISD::Or, bit, &[overflow, SDValue { node: two.node, res: 1 }]);
        let high_sum = self.dag.get_node(ISD::Add, half, &[one, two]);
        let (wl, wr) =
            (self.dag.get_node(ISD::ZeroExtend, wide, &[ll]), self.dag.get_node(ISD::ZeroExtend, wide, &[rl]));
        let three = self.dag.get_node(ISD::Mul, wide, &[wl, wr]);
        let (lo, hi) = self.split_integer(three, half);
        let hi = self.dag.get_node_multi(ISD::UAddO, &[half, bit], &[hi, high_sum]);
        overflow = self.dag.get_node(ISD::Or, bit, &[overflow, SDValue { node: hi.node, res: 1 }]);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
        (lo, SDValue { node: hi.node, res: 0 })
    }

    /// `ExpandIntRes_MULFIX`: the product of the whole values as four
    /// half-width pieces, shifted right by the scale, and for the saturating
    /// forms clamped by the bits above the result.
    ///
    /// DEVIATE SD103: the pieces come from `forceExpandMultiply`, never
    /// `expandMUL_LOHI` (which uses a legal `UMUL_LOHI` of the half when there
    /// is one): the same value, more instructions. Exit: translate
    /// `expandMUL_LOHI`. Owner sdag-a. Review 2026-11-27.
    ///
    /// DEVIATE SD104: an unsigned scale above a half shifts `HH`, where the
    /// pin shifts `HL` (its own comment says `HH`; upstream main fixed it):
    /// bits `64 + scale` and up of the product lie in `HH`. Exit: the next
    /// pin bump. Owner sdag-a. Review 2026-11-27.
    fn expand_mulfix(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let op = self.dag.opcode(value);
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let vt = self.dag.value_type(value);
        let scale = self.dag.constant(self.dag.operand(value, 2)).expect("a constant scale") as u32;
        let saturating = matches!(op, ISD::SMulFixSat | ISD::UMulFixSat);
        let signed = matches!(op, ISD::SMulFix | ISD::SMulFixSat);
        let bits = vt.size_in_bits();
        let half_bits = half.size_in_bits();
        let bool_vt = self.tli.types.set_cc_result_type(vt);

        // A scale of zero is a plain multiply, clamped by its overflow.
        if scale == 0 {
            let result = if !saturating {
                self.dag.get_node(ISD::Mul, vt, &[lhs, rhs])
            } else {
                let product =
                    self.dag.get_node_multi(if signed { ISD::SMulO } else { ISD::UMulO }, &[vt, bool_vt], &[lhs, rhs]);
                let overflow = SDValue { node: product.node, res: 1 };
                if signed {
                    let (min, max) = (1u128 << (bits - 1), (1u128 << (bits - 1)) - 1);
                    let (sat_min, sat_max) = (self.dag.get_constant(min, vt), self.dag.get_constant(max, vt));
                    let zero = self.dag.get_constant(0, vt);
                    // The inputs' xor: sign bit clear, the product is positive.
                    let xor = self.dag.get_node(ISD::Xor, vt, &[lhs, rhs]);
                    let negative = self.dag.get_setcc(bool_vt, xor, zero, CondCode::SetLt);
                    let saturated = self.dag.get_select(vt, negative, sat_min, sat_max);
                    self.dag.get_select(vt, overflow, saturated, product)
                } else {
                    let sat_max = self.dag.get_all_ones_constant(vt);
                    self.dag.get_select(vt, overflow, sat_max, product)
                }
            };
            return self.split_integer(result, half);
        }

        let (low, high) = super::expand_fixed::force_expand_multiply(self.dag, signed, lhs, rhs);
        let (p0, p1) = self.split_integer(low, half);
        let (p2, p3) = self.split_integer(high, half);
        let pieces = [p0, p1, p2, p3];

        // Shift right by the scale: two funnel shifts pick the result's
        // halves, none when the scale is a whole number of pieces.
        let first = (scale / half_bits) as usize;
        let (mut lo, mut hi) = if scale % half_bits != 0 {
            let amount = self.dag.get_constant(u128::from(scale % half_bits), EVT::I8);
            (
                self.dag.get_node(ISD::FShr, half, &[pieces[first + 1], pieces[first], amount]),
                self.dag.get_node(ISD::FShr, half, &[pieces[first + 2], pieces[first + 1], amount]),
            )
        } else {
            (pieces[first], pieces[first + 1])
        };
        // No integer part, so nothing to overflow.
        if !saturating || scale == bits {
            return (lo, hi);
        }

        let (result_hl, result_hh) = (pieces[2], pieces[3]);
        let bool_half = self.tli.types.set_cc_result_type(half);
        let zero = self.dag.get_constant(0, half);
        let ones = self.dag.get_all_ones_constant(half);
        let test = |this: &mut Self, a: SDValue, b: SDValue, cc: CondCode| this.dag.get_setcc(bool_half, a, b, cc);
        let half_mask = super::fold::mask(half_bits);

        if !signed {
            let sat_max = if scale < half_bits {
                // Overflowed if (HH | (HL >> Scale)) != 0.
                let amount = self.dag.get_constant(u128::from(scale), EVT::I8);
                let adjusted = self.dag.get_node(ISD::Srl, half, &[result_hl, amount]);
                let any = self.dag.get_node(ISD::Or, half, &[adjusted, result_hh]);
                test(self, any, zero, CondCode::SetNe)
            } else if scale == half_bits {
                test(self, result_hh, zero, CondCode::SetNe)
            } else {
                // Overflowed if (HH >> (Scale - half)) != 0.
                let amount = self.dag.get_constant(u128::from(scale - half_bits), EVT::I8);
                let adjusted = self.dag.get_node(ISD::Srl, half, &[result_hh, amount]);
                test(self, adjusted, zero, CondCode::SetNe)
            };
            hi = self.dag.get_select(half, sat_max, ones, hi);
            lo = self.dag.get_select(half, sat_max, ones, lo);
            return (lo, hi);
        }

        let (sat_max, sat_min);
        if scale < half_bits {
            // Overflowed past the maximum if the top (width - scale + 1) bits
            // are above zero, past the minimum if below -1.
            let overflow_bits = bits - scale + 1;
            let hl_hi_mask =
                self.dag.get_constant(half_mask & !super::fold::mask(half_bits - (overflow_bits - half_bits)), half);
            let hl_lo_mask = self.dag.get_constant(super::fold::mask(bits - overflow_bits), half);
            // Max if HH > 0 or (HH == 0 && HL > HLLoMask).
            let hh_gt0 = test(self, result_hh, zero, CondCode::SetGt);
            let hh_eq0 = test(self, result_hh, zero, CondCode::SetEq);
            let hl_ugt = test(self, result_hl, hl_lo_mask, CondCode::SetUGt);
            let and = self.dag.get_node(ISD::And, bool_half, &[hh_eq0, hl_ugt]);
            sat_max = self.dag.get_node(ISD::Or, bool_half, &[hh_gt0, and]);
            // Min if HH < -1 or (HH == -1 && HL < HLHiMask).
            let hh_lt = test(self, result_hh, ones, CondCode::SetLt);
            let hh_eq = test(self, result_hh, ones, CondCode::SetEq);
            let hl_ult = test(self, result_hl, hl_hi_mask, CondCode::SetULt);
            let and = self.dag.get_node(ISD::And, bool_half, &[hh_eq, hl_ult]);
            sat_min = self.dag.get_node(ISD::Or, bool_half, &[hh_lt, and]);
        } else if scale == half_bits {
            // Max if HH > 0 or (HH == 0 && the sign bit of HL is set).
            let hh_gt0 = test(self, result_hh, zero, CondCode::SetGt);
            let hh_eq0 = test(self, result_hh, zero, CondCode::SetEq);
            let hl_neg = test(self, result_hl, zero, CondCode::SetLt);
            let and = self.dag.get_node(ISD::And, bool_half, &[hh_eq0, hl_neg]);
            sat_max = self.dag.get_node(ISD::Or, bool_half, &[hh_gt0, and]);
            // Min if HH < -1 or (HH == -1 && the sign bit of HL is clear).
            let hh_lt = test(self, result_hh, ones, CondCode::SetLt);
            let hh_eq = test(self, result_hh, ones, CondCode::SetEq);
            let hl_pos = test(self, result_hl, zero, CondCode::SetGe);
            let and = self.dag.get_node(ISD::And, bool_half, &[hh_eq, hl_pos]);
            sat_min = self.dag.get_node(ISD::Or, bool_half, &[hh_lt, and]);
        } else {
            // As the first case, but only HH matters.
            let overflow_bits = bits - scale + 1;
            let hh_hi_mask = self.dag.get_constant(half_mask & !super::fold::mask(half_bits - overflow_bits), half);
            let hh_lo_mask = self.dag.get_constant(super::fold::mask(half_bits - overflow_bits), half);
            sat_max = test(self, result_hh, hh_lo_mask, CondCode::SetGt);
            sat_min = test(self, result_hh, hh_hi_mask, CondCode::SetLt);
        }

        // Saturate to the signed maximum, then the signed minimum.
        let max_hi = self.dag.get_constant(half_mask >> 1, half);
        hi = self.dag.get_select(half, sat_max, max_hi, hi);
        lo = self.dag.get_select(half, sat_max, ones, lo);
        let min_hi = self.dag.get_constant(1u128 << (half_bits - 1), half);
        hi = self.dag.get_select(half, sat_min, min_hi, hi);
        lo = self.dag.get_select(half, sat_min, zero, lo);
        (lo, hi)
    }

    /// `ExpandIntRes_XMULO` for the signed multiply, with no `__mulodi4` (the
    /// pin leaves it to compiler-rt) and no 128-bit multiply:
    /// `forceExpandWideMUL` through `forceExpandMultiply`, Hacker's
    /// Delight's signed product of two `i64` as two `i64`s of 32-bit
    /// pieces; overflow when the high word is not the sign of the low.
    fn expand_smulo(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let wide = self.dag.value_type(value);
        let bits = wide.size_in_bits();
        let (product_lo, product_hi) = super::expand_fixed::force_expand_multiply(self.dag, true, lhs, rhs);
        let sign_shift = self.dag.get_constant(u128::from(bits - 1), EVT::I8);
        let sign = self.dag.get_node(ISD::Sra, wide, &[product_lo, sign_shift]);
        let overflow_vt = self.dag.node(value).values[1];
        let overflow = self.dag.get_setcc(overflow_vt, product_hi, sign, CondCode::SetNe);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
        self.split_integer(product_lo, half)
    }

    /// `ExpandIntRes_ZERO_EXTEND`, `_SIGN_EXTEND`, `_ANY_EXTEND`: the source
    /// fits the half (extend it for the low part; the high part is zero, its
    /// sign, or undefined) or was promoted to the whole result (split it, and
    /// clear or extend the excess bits of the high part).
    fn expand_extend(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let input = self.dag.operand(value, 0);
        let from = self.dag.value_type(input);
        if from.bits_le(half) {
            let lo = self.dag.get_node(op, half, &[input]);
            let hi = match op {
                ISD::ZeroExtend => self.dag.get_constant(0, half),
                ISD::SignExtend => {
                    let sign = self.dag.get_constant(u128::from(half.size_in_bits() - 1), EVT::I8);
                    self.dag.get_node(ISD::Sra, half, &[lo, sign])
                }
                _ => self.dag.get_undef(half),
            };
            return (lo, hi);
        }
        // Over the half but not the whole: the source was promoted to the
        // result's type.
        let promoted = self.promoted_integer(input);
        let (lo, hi) = self.split_integer(promoted, half);
        let excess = from.size_in_bits() - half.size_in_bits();
        let hi = match op {
            ISD::ZeroExtend => {
                let mask = self.dag.get_constant(super::fold::mask(excess), half);
                self.dag.get_node(ISD::And, half, &[hi, mask])
            }
            ISD::SignExtend => {
                let ty = self.dag.get_value_type_node(EVT::integer(excess));
                self.dag.get_node(ISD::SignExtendInReg, half, &[hi, ty])
            }
            _ => hi,
        };
        (lo, hi)
    }

    /// `SplitInteger`: the low and high halves of `value` as `truncate` and
    /// `truncate (srl value, half)`.
    fn split_integer(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let whole = self.dag.value_type(value);
        let lo = self.dag.get_node(ISD::Truncate, half, &[value]);
        let by = self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
        let upper = self.dag.get_node(ISD::Srl, whole, &[value, by]);
        (lo, self.dag.get_node(ISD::Truncate, half, &[upper]))
    }

    /// `ExpandIntRes_MUL` through `expandMUL_LOHI`: the full product of the low
    /// halves, plus the two cross products in the high half. Two operands that
    /// are both zero-extended or both sign-extended (by their known bits) need
    /// no cross products. No library call is made: the pin has none for the
    /// types that come here, and `force_expand_multiply` is its fallback.
    fn expand_mul(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        // "The inputs are both zero-extended" and "both sign-extended".
        let bits = half.size_in_bits();
        let outer = self.dag.value_type(value).size_in_bits();
        let high_mask = super::fold::mask(outer) & !super::fold::mask(bits);
        let widening = |this: &mut Self, signed: bool| {
            let (op, other) = if signed { (ISD::SMulLoHi, ISD::MulHS) } else { (ISD::UMulLoHi, ISD::MulHU) };
            if this.tli.is_operation_legal_or_custom(op, half) {
                let both = this.dag.get_node_multi(op, &[half, half], &[ll, rl]);
                Some((both, SDValue { node: both.node, res: 1 }))
            } else if this.tli.is_operation_legal_or_custom(other, half) {
                Some((this.dag.get_node(ISD::Mul, half, &[ll, rl]), this.dag.get_node(other, half, &[ll, rl])))
            } else {
                None
            }
        };
        if super::known_bits::masked_value_is_zero(self.dag, lhs, high_mask)
            && super::known_bits::masked_value_is_zero(self.dag, rhs, high_mask)
            && let Some(pair) = widening(self, false)
        {
            return Ok(pair);
        }
        if super::known_bits::max_significant_bits(self.dag, lhs) <= bits
            && super::known_bits::max_significant_bits(self.dag, rhs) <= bits
            && let Some(pair) = widening(self, true)
        {
            return Ok(pair);
        }
        let (lo, hi) = if self.tli.is_operation_legal_or_custom(ISD::UMulLoHi, half) {
            let both = self.dag.get_node_multi(ISD::UMulLoHi, &[half, half], &[ll, rl]);
            (both, SDValue { node: both.node, res: 1 })
        } else if self.tli.is_operation_legal_or_custom(ISD::MulHU, half) {
            (self.dag.get_node(ISD::Mul, half, &[ll, rl]), self.dag.get_node(ISD::MulHU, half, &[ll, rl]))
        } else {
            return Ok(self.force_expand_multiply(ll, rl, lh, rh));
        };
        let cross_right = self.dag.get_node(ISD::Mul, half, &[ll, rh]);
        let cross_left = self.dag.get_node(ISD::Mul, half, &[lh, rl]);
        let hi = self.dag.get_node(ISD::Add, half, &[hi, cross_right]);
        let hi = self.dag.get_node(ISD::Add, half, &[hi, cross_left]);
        Ok((lo, hi))
    }

    /// `TargetLowering::forceExpandMultiply` (unsigned, with the high halves
    /// `hi_lhs` and `hi_rhs` given): Hacker's Delight 8-2 on `half`-wide
    /// values, for a half with no widening multiply. `ExpandIntRes_MUL` takes
    /// it when the target has no library call for the type; x86-32 has none
    /// for i128 (`__multi3` is 64-bit only), so a wide product of an i66
    /// sum of products came to "expand multiply without a widening
    /// multiply".
    fn force_expand_multiply(
        &mut self,
        lhs: SDValue,
        rhs: SDValue,
        hi_lhs: SDValue,
        hi_rhs: SDValue,
    ) -> (SDValue, SDValue) {
        let vt = self.dag.value_type(lhs);
        let half_bits = vt.size_in_bits() / 2;
        let mask = self.dag.get_constant(super::fold::mask(half_bits), vt);
        let shift = self.dag.get_constant(u128::from(half_bits), EVT::I8);
        let node = |this: &mut Self, op, a, b| this.dag.get_node(op, vt, &[a, b]);
        let ll = node(self, ISD::And, lhs, mask);
        let rl = node(self, ISD::And, rhs, mask);
        let t = node(self, ISD::Mul, ll, rl);
        let tl = node(self, ISD::And, t, mask);
        let th = node(self, ISD::Srl, t, shift);
        let lh = node(self, ISD::Srl, lhs, shift);
        let rh = node(self, ISD::Srl, rhs, shift);
        let lh_rl = node(self, ISD::Mul, lh, rl);
        let u = node(self, ISD::Add, lh_rl, th);
        let ul = node(self, ISD::And, u, mask);
        let uh = node(self, ISD::Srl, u, shift);
        let ll_rh = node(self, ISD::Mul, ll, rh);
        let v = node(self, ISD::Add, ll_rh, ul);
        let vh = node(self, ISD::Srl, v, shift);
        let v_up = node(self, ISD::Shl, v, shift);
        let lo = node(self, ISD::Add, tl, v_up);
        let high_products = node(self, ISD::Mul, lh, rh);
        let carries = node(self, ISD::Add, uh, vh);
        let hi = node(self, ISD::Add, high_products, carries);
        let cross_a = node(self, ISD::Mul, hi_rhs, lhs);
        let cross_b = node(self, ISD::Mul, rhs, hi_lhs);
        let crosses = node(self, ISD::Add, cross_a, cross_b);
        (lo, node(self, ISD::Add, hi, crosses))
    }

    /// `expandFixedPointDiv` for the signed, non-saturating divide: the
    /// numerator shifted up by the headroom it has (its redundant sign bits),
    /// the denominator down by the rest of the scale, then a divide that rounds
    /// toward negative infinity. `None` when the type has not the room.
    fn expand_fixed_point_div(
        &mut self,
        lhs: SDValue,
        rhs: SDValue,
        scale: u32,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(lhs);
        let lead = super::known_bits::num_sign_bits(self.dag, lhs) - 1;
        let known = super::known_bits::known_bits(self.dag, rhs);
        // the trailing zero bits the denominator is known to have
        let trail = (known.zero.trailing_ones()).min(vt.size_in_bits());
        if lead + trail < scale {
            return None;
        }
        let lhs_shift = lead.min(scale);
        let rhs_shift = scale - lhs_shift;
        let (mut lhs, mut rhs) = (lhs, rhs);
        if lhs_shift != 0 {
            let by = self.dag.get_constant(u128::from(lhs_shift), EVT::I8);
            lhs = self.dag.get_node(ISD::Shl, vt, &[lhs, by]);
        }
        if rhs_shift != 0 {
            let by = self.dag.get_constant(u128::from(rhs_shift), EVT::I8);
            rhs = self.dag.get_node(ISD::Sra, vt, &[rhs, by]);
        }
        let bool_vt = self.tli.types.set_cc_result_type(vt);
        let quotient = self.dag.get_node(ISD::SDiv, vt, &[lhs, rhs]);
        let remainder = self.dag.get_node(ISD::SRem, vt, &[lhs, rhs]);
        let zero = self.dag.get_constant(0, vt);
        let rem_nonzero = self.dag.get_setcc(bool_vt, remainder, zero, CondCode::SetNe);
        let lhs_negative = self.dag.get_setcc(bool_vt, lhs, zero, CondCode::SetLt);
        let rhs_negative = self.dag.get_setcc(bool_vt, rhs, zero, CondCode::SetLt);
        let quotient_negative = self.dag.get_node(ISD::Xor, bool_vt, &[lhs_negative, rhs_negative]);
        let one = self.dag.get_constant(1, vt);
        let below = self.dag.get_node(ISD::Sub, vt, &[quotient, one]);
        let adjust = self.dag.get_node(ISD::And, bool_vt, &[rem_nonzero, quotient_negative]);
        Some(self.dag.get_select(vt, adjust, below, quotient))
    }

    /// `ExpandIntRes_CTPOP`: the population count of both halves, added.
    fn expand_ctpop(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
        let (lo_count, hi_count) =
            (self.dag.get_node(ISD::CtPop, half, &[lo]), self.dag.get_node(ISD::CtPop, half, &[hi]));
        (self.dag.get_node(ISD::Add, half, &[lo_count, hi_count]), self.dag.get_constant(0, half))
    }

    /// `ExpandIntRes_CTLZ`: the high half's count if it is not zero, else the
    /// low half's plus the width of a half.
    fn expand_ctlz(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
        let zero = self.dag.get_constant(0, half);
        let hi_not_zero = self.dag.get_setcc(self.tli.types.set_cc_result_type(half), hi, zero, CondCode::SetNe);
        let lo_count = self.dag.get_node(op, half, &[lo]);
        let hi_count = self.dag.get_node(ISD::CtLzZeroPoison, half, &[hi]);
        let width = self.dag.get_constant(u128::from(half.size_in_bits()), half);
        let lo_plus = self.dag.get_node(ISD::Add, half, &[lo_count, width]);
        (self.dag.get_select(half, hi_not_zero, hi_count, lo_plus), zero)
    }

    /// `ExpandIntRes_CTTZ`: the low half's count if it is not zero, else the
    /// high half's plus the width of a half.
    fn expand_cttz(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
        let zero = self.dag.get_constant(0, half);
        let lo_not_zero = self.dag.get_setcc(self.tli.types.set_cc_result_type(half), lo, zero, CondCode::SetNe);
        let lo_count = self.dag.get_node(ISD::CtTzZeroPoison, half, &[lo]);
        let hi_count = self.dag.get_node(op, half, &[hi]);
        let width = self.dag.get_constant(u128::from(half.size_in_bits()), half);
        let hi_plus = self.dag.get_node(ISD::Add, half, &[hi_count, width]);
        (self.dag.get_select(half, lo_not_zero, lo_count, hi_plus), zero)
    }

    /// `ExpandIntRes_SDIV` and the others, as the library call the pin makes
    /// (`ExpandDivRemLibCall`): a call to the target's inline `i64` divide
    /// routine (`divide_routine`), operands and results in the registers it
    /// states (SD19). The divisor is "narrow" when its high dword is the
    /// constant 0, which has a cheaper routine.
    fn expand_division(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        use super::libcall::{CallEnv, Place, Returned, lib_call};
        let vt = self.dag.value_type(value);
        let Some(arch) = self.target else {
            return Err(LegalizeError::Unsupported { op, ty: vt, what: "expand a division without a target" });
        };
        let signed = matches!(op, ISD::SDiv | ISD::SRem);
        let wants_quotient = matches!(op, ISD::SDiv | ISD::UDiv);
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((dividend_lo, dividend_hi), (divisor_lo, divisor_hi)) =
            (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let wide = self.dag.constant(divisor_hi) != Some(0);
        let used =
            if wants_quotient { llrm_target::routines::Used::Quotient } else { llrm_target::routines::Used::Remainder };
        let Ok(routine) = arch.divide_routine_used(signed, wide, used) else {
            return Err(LegalizeError::Unsupported { op, ty: vt, what: "the i64 divide routine is unavailable" });
        };
        let mut args = vec![
            (dividend_lo, Place::Register(routine.dividend[0])),
            (dividend_hi, Place::Register(routine.dividend[1])),
            (divisor_lo, Place::Register(routine.divisor[0])),
        ];
        if wide {
            args.push((divisor_hi, Place::Register(routine.divisor[1])));
        }
        let returns: Vec<Returned> =
            [routine.quotient[0], routine.quotient[1], routine.remainder[0], routine.remainder[1]]
                .into_iter()
                .map(|reg| Returned { reg, vt: half })
                .collect();
        let pointer = self.dag.config().pointer;
        let callee = self.dag.get_target_external_symbol(pointer, routine.name);
        let root = self.dag.entry_node();
        let env = CallEnv { arch, pointer };
        let (_chain, results) = lib_call(self.dag, &env, root, callee, &args, 0, 0, &returns);
        Ok(if wants_quotient { (results[0], results[1]) } else { (results[2], results[3]) })
    }

    /// `CustomLowerNode`: ask the target to lower `n` when its action for the
    /// illegal type `vt` is Custom; its result replaces the node's.
    fn custom_lower_node(
        &mut self,
        n: u32,
        vt: EVT,
    ) -> bool {
        let node = SDValue { node: n, res: 0 };
        if self.tli.operation_action(self.dag.opcode(node), vt) != super::actions::Action::Custom {
            return false;
        }
        let Some(hooks) = self.hooks.as_mut() else { return false };
        let Some(result) = hooks.lower_operation(self.dag, node) else { return false };
        if result == node {
            return false;
        }
        let results = self.dag.node(node).values.len();
        for res in 0..results as u8 {
            let new = if results == 1 { result } else { SDValue { node: result.node, res } };
            self.replace_value_with(SDValue { node: n, res }, new);
        }
        true
    }

    /// `ExpandIntRes_LOAD`, a plain load: the low dword at the address, the
    /// high at +4 (little endian), both on the load's chain, the chains joined.
    fn expand_load(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let NodeData::Mem(info) = self.dag.node(value).data else { unreachable!("a load has a memory operand") };
        if let Some(ext) = info.ext {
            // `ExpandIntRes_LOAD`, an extending load from memory no wider than
            // the half: the low half is the (extending) load of
            // that memory; the high is zero, the sign, or undefined.
            let lo = if info.mem_vt == half {
                self.dag.get_load(half, chain, ptr, super::dag::MemInfo { ext: None, ..info })
            } else {
                self.dag.get_ext_load(ext, half, chain, ptr, info)
            };
            let hi = match ext {
                super::actions::LoadExt::ZExtLoad => self.dag.get_constant(0, half),
                super::actions::LoadExt::SExtLoad => {
                    let sign = self.dag.get_constant(u128::from(half.size_in_bits() - 1), EVT::I8);
                    self.dag.get_node(ISD::Sra, half, &[lo, sign])
                }
                super::actions::LoadExt::ExtLoad => self.dag.get_undef(half),
            };
            self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: lo.node, res: 1 });
            return (lo, hi);
        }
        let info = super::dag::MemInfo { mem_vt: half, ..info };
        let step = u128::from(half.size_in_bits() / 8);
        let high_ptr = self.address_plus(ptr, step);
        let lo = self.dag.get_load(half, chain, ptr, info);
        let hi = self.dag.get_load(half, chain, high_ptr, info);
        let chains = self.dag.get_token_factor(&[SDValue { node: lo.node, res: 1 }, SDValue { node: hi.node, res: 1 }]);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, chains);
        (lo, hi)
    }

    /// Whether a far pointer may be addressed, so its selector must be in `ES`
    /// (`les`). Only a pointer every user of which ends in this block as data,
    /// a huge-pointer difference, is loaded as two words, which a `movzx`
    /// folds. Any other user (a store, a call, a register copy to another
    /// block) may address it later.
    fn addresses_memory(
        &self,
        pointer: SDValue,
    ) -> bool {
        let value = SDValue { node: pointer.node, res: 0 };
        let mut users = self.dag.users(value).peekable();
        users.peek().is_none()
            || users.any(|user| self.dag.opcode(SDValue { node: user, res: 0 }) != x86isd::LLRM_PTRDIFF)
    }

    /// `ptr + step` for the second half of a split access. A far pointer moves
    /// in its offset word and keeps its selector (a far `getelementptr`
    /// does the same); a type with no constant panicked on a far `i64`
    /// load.
    ///
    /// DEVIATE SD308: no carry into the selector, so a half that crosses a
    /// segment of a huge pointer wraps. Reason: an object lies in one
    /// segment unless the pointer is huge, and a split 8-byte access of a
    /// huge element is not seen in the corpus. Exit: `HUGE_ADD` here when
    /// the access's pointer space is known. Owner sdag-c. Review 2026-12-10.
    fn address_plus(
        &mut self,
        ptr: SDValue,
        step: u128,
    ) -> SDValue {
        let pointer = self.dag.value_type(ptr);
        if pointer == EVT::Simple(MVT::Far) {
            let (offset, selector) = self.expanded_integer(ptr);
            let word = self.dag.value_type(offset);
            let delta = self.dag.get_constant(step, word);
            let moved = self.dag.get_node(ISD::Add, word, &[offset, delta]);
            return self.dag.get_node(ISD::BuildPair, pointer, &[moved, selector]);
        }
        let offset = self.dag.get_constant(step, pointer);
        self.dag.get_node(ISD::Add, pointer, &[ptr, offset])
    }

    /// `ExpandIntOp_STORE`, a plain store: the low dword, then the high at +4.
    fn expand_store(
        &mut self,
        store: SDValue,
    ) -> SDValue {
        let (chain, value, ptr) = (self.dag.operand(store, 0), self.dag.operand(store, 1), self.dag.operand(store, 2));
        let NodeData::Mem(info) = self.dag.node(store).data else { unreachable!("a store has a memory operand") };
        let (lo, hi) = self.expanded_integer(value);
        let half = self.dag.value_type(lo);
        let info = super::dag::MemInfo { mem_vt: half, ..info };
        let step = u128::from(half.size_in_bits() / 8);
        let high_ptr = self.address_plus(ptr, step);
        let low_store = self.dag.get_store(chain, lo, ptr, info);
        let high_store = self.dag.get_store(chain, hi, high_ptr, info);
        self.dag.get_token_factor(&[low_store, high_store])
    }

    /// A huge pointer plus a dword: the offset widened and added as a
    /// dword, the offset the sum's low word, the selector stepped by its high
    /// word at the machine's stride (`Machine::huge_shift`).
    fn expand_huge_add(
        &mut self,
        node: SDValue,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        let shift = self
            .target
            .and_then(|target| target.machine().huge_shift())
            .ok_or(
                LegalizeError::Unsupported {
                    op: x86isd::HUGE_ADD,
                    ty: self.dag.value_type(node),
                    what: "expand result: no selector stride",
                },
            )?;
        let (far, delta) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let (offset, selector) = self.expanded_integer(far);
        let dword = self.dag.value_type(delta);
        // A constant displacement past 64K from a symbol: the object is
        // longer than a segment, so it is alone in its own and starts at
        // offset 0 (`data.rs` `split`). The displacement is the symbol's
        // offset plus its low word and the selector steps by its high word:
        // `offset _big+14464`, `seg _big` + 4096, nothing computed.
        if let (ISD::GlobalAddress, Some(bytes)) = (self.dag.opcode(offset), self.dag.constant(delta)) {
            let half_bits = half.size_in_bits();
            if bytes >> half_bits != 0 && bytes >> 32 == 0 {
                let super::dag::NodeData::Global { symbol, offset: at } = self.dag.node(offset).data else {
                    unreachable!("a global address")
                };
                let low = self.dag.get_global_address(half, symbol, at + (bytes & ((1 << half_bits) - 1)) as i64);
                let steps = self.dag.get_constant((bytes >> half_bits) << shift & ((1 << half_bits) - 1), half);
                return Ok((low, self.dag.get_node(ISD::Add, half, &[selector, steps])));
            }
        }
        // A step of less than one offset range, known at compile time, moves
        // the selector by at most one stride, and the offset's carry (borrow)
        // says whether: `add; sbb; and`, not the dword sum's high word.
        if let Some(step) = self.dag.constant(delta).and_then(|bits| {
            let (width, bits) = (dword.size_in_bits(), bits);
            let half_bits = half.size_in_bits();
            let range = 1u128 << half_bits;
            let negative = bits >= (1u128 << width) - range;
            (bits != 0 && (bits < range || negative))
                .then(|| (negative, if negative { (1u128 << width) - bits } else { bits }))
        }) {
            let (negative, magnitude) = step;
            let amount = self.dag.get_constant(magnitude, half);
            // the wrap is the add's own carry (`UADDO`, `add; setb`), not a
            // compare against the offset before the step, which keeps that
            // offset alive across the loop
            let bool_vt = self.tli.types.set_cc_result_type(half);
            let both = self
                .dag
                .get_node_multi(if negative { ISD::USubO } else { ISD::UAddO }, &[half, bool_vt], &[offset, amount]);
            let (moved, wrapped) = (both, SDValue { node: both.node, res: 1 });
            let stride = self.dag.get_constant(1u128 << shift, half);
            let zero = self.dag.get_constant(0, half);
            let carry = self.dag.get_select(half, wrapped, stride, zero);
            let op = if negative { ISD::Sub } else { ISD::Add };
            return Ok((moved, self.dag.get_node(op, half, &[selector, carry])));
        }
        // A delta that fits the offset adds as a word and the carry steps the
        // selector: `add bx, 4; sbb di, di; and di, 4096`, not a dword add and
        // a shift of its high word. (The expansion of the dword add: the
        // low halves' `UADDO`, the carry as the high half's step.)
        let half_bits = half.size_in_bits();
        let high_zero = !super::known_bits::known_bits(self.dag, delta).zero
            & super::fold::mask(dword.size_in_bits()) >> half_bits << half_bits
            == 0;
        if high_zero && self.tli.is_operation_legal_or_custom(ISD::UAddO, half) {
            let low = self.dag.get_node(ISD::Truncate, half, &[delta]);
            let overflow_vt = self.tli.types.set_cc_result_type(half);
            let sum = self.dag.get_node_multi(ISD::UAddO, &[half, overflow_vt], &[offset, low]);
            let carry = SDValue { node: sum.node, res: 1 };
            let stride = self.dag.get_constant(1u128 << shift, half);
            let zero = self.dag.get_constant(0, half);
            let step = self.dag.get_select(half, carry, stride, zero);
            return Ok((sum, self.dag.get_node(ISD::Add, half, &[selector, step])));
        }
        let wide = self.dag.get_node(ISD::ZeroExtend, dword, &[offset]);
        let sum = self.dag.get_node(ISD::Add, dword, &[wide, delta]);
        let (width, stride_shift) = (
            self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8),
            self.dag.get_constant(u128::from(shift), EVT::I8),
        );
        let carry = self.dag.get_node(ISD::Sra, dword, &[sum, width]);
        let stride = self.dag.get_node(ISD::Shl, dword, &[carry, stride_shift]);
        let stride = self.dag.get_node(ISD::Truncate, half, &[stride]);
        Ok((self.dag.get_node(ISD::Truncate, half, &[sum]), self.dag.get_node(ISD::Add, half, &[selector, stride])))
    }

    /// A huge pointer normalized: the offset's whole selector steps move into
    /// the selector, leaving an offset below one step. A step is
    /// `2^(bits - huge_shift)` bytes (16 on DOS), the selector counts steps.
    fn expand_window(
        &mut self,
        node: SDValue,
        half: EVT,
    ) -> Result<(SDValue, SDValue), LegalizeError> {
        let shift = self
            .target
            .and_then(|target| target.machine().huge_shift())
            .ok_or(
                LegalizeError::Unsupported {
                    op: x86isd::LLRM_WINDOW,
                    ty: self.dag.value_type(node),
                    what: "expand result: no selector stride",
                },
            )?;
        let below = half.size_in_bits() - shift;
        let (offset, selector) = self.expanded_integer(self.dag.operand(node, 0));
        let by = self.dag.get_constant(u128::from(below), EVT::I8);
        let steps = self.dag.get_node(ISD::Srl, half, &[offset, by]);
        let mask = self.dag.get_constant((1u128 << below) - 1, half);
        Ok((self.dag.get_node(ISD::And, half, &[offset, mask]), self.dag.get_node(ISD::Add, half, &[selector, steps])))
    }

    /// The bytes between two carried huge pointers into one object: both are
    /// canonical, so the selectors differ by whole strides and the offsets by
    /// the rest: `((sel_a - sel_b) >> huge_shift << 16) + (off_a - off_b)`,
    /// each difference taken as a zero-extended dword.
    fn expand_ptrdiff(
        &mut self,
        node: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let shift = self
            .target
            .and_then(|target| target.machine().huge_shift())
            .ok_or(
                LegalizeError::Unsupported {
                    op: x86isd::LLRM_PTRDIFF,
                    ty: self.dag.value_type(node),
                    what: "expand operand: no selector stride",
                },
            )?;
        let vt = self.dag.value_type(node);
        let ((a_off, a_sel), (b_off, b_sel)) =
            (self.expanded_integer(self.dag.operand(node, 0)), self.expanded_integer(self.dag.operand(node, 1)));
        let half = self.dag.value_type(a_off);
        let widen = |dag: &mut SelectionDAG, value| dag.get_node(ISD::ZeroExtend, vt, &[value]);
        let (offsets, selectors) =
            ((widen(self.dag, a_off), widen(self.dag, b_off)), (widen(self.dag, a_sel), widen(self.dag, b_sel)));
        let within = self.dag.get_node(ISD::Sub, vt, &[offsets.0, offsets.1]);
        let between = self.dag.get_node(ISD::Sub, vt, &[selectors.0, selectors.1]);
        let (by_stride, by_offset) = (
            self.dag.get_constant(u128::from(shift), EVT::I8),
            self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8),
        );
        let strides = self.dag.get_node(ISD::Sra, vt, &[between, by_stride]);
        let high = self.dag.get_node(ISD::Shl, vt, &[strides, by_offset]);
        Ok(self.dag.get_node(ISD::Add, vt, &[within, high]))
    }

    /// `src` stored to a new stack slot: the slot's address, the store and
    /// the frame index.
    fn store_to_slot(
        &mut self,
        src: SDValue,
    ) -> (SDValue, SDValue, i32) {
        let vt = self.dag.value_type(src);
        let size = vt.size_in_bits().div_ceil(8);
        let cell = self.dag.create_stack_temporary(size, 4);
        let pointer = self.dag.config().pointer;
        let slot = self.dag.get_frame_index(pointer, cell);
        let info = self.slot_info(src, cell);
        let entry = self.dag.entry_node();
        let store = self.dag.get_store(entry, src, slot, info);
        (slot, store, cell)
    }

    fn slot_info(
        &self,
        src: SDValue,
        cell: i32,
    ) -> super::dag::MemInfo {
        super::dag::MemInfo {
            mem_vt: self.dag.value_type(src),
            align: 4,
            volatile: false,
            base: super::dag::MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        }
    }

    /// A load or store through a far pointer: the access is at the offset
    /// word, and `ES` is set to the selector before it (`MemInfo::space` 259
    /// makes the access `es:`-relative). Ordered by the chain only (SD209).
    fn expand_far_address(
        &mut self,
        access: SDValue,
        at: usize,
    ) -> SDValue {
        let (offset, selector) = self.expanded_integer(self.dag.operand(access, at));
        let chain = self.dag.get_copy_to_reg(self.dag.operand(access, 0), ES, selector);
        let mut ops = self.dag.node(access).ops.clone();
        ops[0] = chain;
        ops[at] = offset;
        // The unindexed access's offset operand is a poison of the pointer
        // type (`les` and `lds` have no such operand).
        if matches!(self.dag.opcode(access), ISD::Load | ISD::Store) {
            let last = ops.len() - 1;
            ops[last] = self.dag.get_poison(self.dag.value_type(offset));
        }
        let updated = self.dag.update_node_operands(access.node, &ops);
        SDValue { node: updated, res: 0 }
    }

    /// A call through a far pointer: offset then selector stored to a 4-byte
    /// stack cell, and the call reads the cell (`LLRM_FARCALL`, SD208). The
    /// stores come before `callseq_start`: the argument copies, the call and
    /// `callseq_end` are one glued run, and a store chained after a copy would
    /// have to precede the call that is glued to that copy (a cycle the
    /// scheduler left unresolved: the call and everything it read vanished).
    fn expand_far_call(
        &mut self,
        call: SDValue,
    ) -> SDValue {
        let (chain, callee) = (self.dag.operand(call, 0), self.dag.operand(call, 1));
        let (offset, selector) = self.expanded_integer(callee);
        let pointer = self.dag.config().pointer;
        let cell = self.dag.create_stack_temporary(4, 2);
        let slot = self.dag.get_frame_index(pointer, cell);
        let high = self.dag.get_constant(2, pointer);
        let high_slot = self.dag.get_node(ISD::Add, pointer, &[slot, high]);
        let info = super::dag::MemInfo {
            mem_vt: EVT::I16,
            align: 2,
            volatile: false,
            base: super::dag::MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        };
        // the `callseq_start` of this call: back along the chain through the
        // argument copies and the stores of the stack arguments joined before
        // them
        let mut start = chain;
        let mut work = vec![chain];
        let mut seen = llrm_mir::hash::HashSet::default();
        while let Some(at) = work.pop() {
            if !seen.insert(at.node) {
                continue;
            }
            if self.dag.opcode(at) == ISD::CallSeqStart {
                start = at;
                break;
            }
            if matches!(
                self.dag.opcode(at),
                ISD::CopyToReg | ISD::TokenFactor | ISD::Store
            ) {
                let ops = self.dag.node(at).ops.clone();
                let first = usize::from(self.dag.opcode(at) != ISD::TokenFactor);
                work.extend(ops.iter().take(if first == 1 { 1 } else { ops.len() }).copied());
            }
        }
        let before = if self.dag.opcode(start) == ISD::CallSeqStart { self.dag.operand(start, 0) } else { chain };
        let low_store = self.dag.get_store(before, offset, slot, info);
        let high_store = self.dag.get_store(before, selector, high_slot, info);
        if self.dag.opcode(start) == ISD::CallSeqStart {
            let stores = self.dag.get_token_factor(&[before, low_store, high_store]);
            let mut ops = self.dag.node(start).ops.clone();
            ops[0] = stores;
            self.dag.update_node_operands(start.node, &ops);
        }
        let target = self.dag.get_target_frame_index(pointer, cell);
        let mut ops = self.dag.node(call).ops.clone();
        if self.dag.opcode(start) != ISD::CallSeqStart {
            ops[0] = self.dag.get_token_factor(&[low_store, high_store]);
        }
        ops[1] = target;
        let values = self.dag.node(call).values.clone();
        let far_call = self.dag.get_node_multi(super::x86isd::LLRM_FARCALL, &values, &ops);
        self.replace_value_with(SDValue { node: call.node, res: 0 }, SDValue { node: far_call.node, res: 0 });
        self.replace_value_with(SDValue { node: call.node, res: 1 }, SDValue { node: far_call.node, res: 1 });
        far_call
    }

    /// `IntegerExpandSetCCOperands`: equality is the halves' xors or-ed
    /// against zero; an ordering is the low halves' unsigned compare unless
    /// the high halves differ.
    fn expand_setcc(
        &mut self,
        setcc: SDValue,
        code: CondCode,
    ) -> SDValue {
        let vt = self.dag.value_type(setcc);
        let ((ll, lh), (rl, rh)) =
            (self.expanded_integer(self.dag.operand(setcc, 0)), self.expanded_integer(self.dag.operand(setcc, 1)));
        let half = self.dag.value_type(ll);
        if matches!(code, CondCode::SetEq | CondCode::SetNe) {
            let (low, high) =
                (self.dag.get_node(ISD::Xor, half, &[ll, rl]), self.dag.get_node(ISD::Xor, half, &[lh, rh]));
            let both = self.dag.get_node(ISD::Or, half, &[low, high]);
            let zero = self.dag.get_constant(0, half);
            return self.dag.get_setcc(vt, both, zero, code);
        }
        let low_code = match code {
            CondCode::SetLt => CondCode::SetULt,
            CondCode::SetLe => CondCode::SetULe,
            CondCode::SetGt => CondCode::SetUGt,
            CondCode::SetGe => CondCode::SetUGe,
            unsigned => unsigned,
        };
        let low = self.dag.get_setcc(vt, ll, rl, low_code);
        // Comparing the low bits is enough.
        if lh == rh {
            return low;
        }
        // Lower with SETCCCARRY if the target supports it: the high halves
        // subtracted with the low halves' borrow; `>` and `<=` flip the
        // operands.
        if self.tli.is_operation_legal_or_custom(ISD::SetCCCarry, self.dag.value_type(lh)) {
            let (code, flip) = match code {
                CondCode::SetGt => (CondCode::SetLt, true),
                CondCode::SetUGt => (CondCode::SetULt, true),
                CondCode::SetLe => (CondCode::SetGe, true),
                CondCode::SetULe => (CondCode::SetUGe, true),
                other => (other, false),
            };
            let ((ll, rl), (lh, rh)) = if flip { ((rl, ll), (rh, lh)) } else { ((ll, rl), (lh, rh)) };
            let overflow_vt = self.tli.types.set_cc_result_type(half);
            let borrow = self.dag.get_node_multi(ISD::USubO, &[half, overflow_vt], &[ll, rl]);
            let borrow = SDValue { node: borrow.node, res: 1 };
            let cc = self.dag.get_cond_code(code);
            return self.dag.get_node(ISD::SetCCCarry, vt, &[lh, rh, borrow, cc]);
        }
        let high = self.dag.get_setcc(vt, lh, rh, code);
        let same = self.dag.get_setcc(vt, lh, rh, CondCode::SetEq);
        self.dag.get_select(vt, same, low, high)
    }

    fn setcc_code(
        &self,
        setcc: SDValue,
    ) -> Option<CondCode> {
        let at = if self.dag.opcode(setcc) == ISD::SelectCC { 4 } else { 2 };
        match self.dag.node(self.dag.operand(setcc, at)).data {
            NodeData::CondCode(code) => Some(code),
            _ => None,
        }
    }

    /// An `i64` constant zero (an opaque one too).
    fn is_null_wide(
        &self,
        value: SDValue,
    ) -> bool {
        self.dag.constant(value) == Some(0)
    }

    /// `ExpandIntegerOperand`.
    fn expand_integer_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let result = match self.dag.opcode(node) {
            ISD::Truncate => {
                let (lo, _) = self.expanded_integer(self.dag.operand(node, at));
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::Truncate, vt, &[lo])
            }
            ISD::ExtractElement if at == 0 => {
                // `ExpandOp_EXTRACT_ELEMENT`: the low or the high half of the
                // pair.
                let (lo, hi) = self.expanded_integer(self.dag.operand(node, 0));
                if self.dag.constant(self.dag.operand(node, 1)).is_some_and(|which| which != 0) { hi } else { lo }
            }
            ISD::Store if at == 1 => self.expand_store(node),
            ISD::CopyToReg if at == 2 => {
                // A far pointer copied to the register pair a dword stands for
                // (a return or an argument in `dx:ax`): the halves joined into
                // that dword.
                let (lo, hi) = self.expanded_integer(self.dag.operand(node, 2));
                let dword = self.dag.get_node(ISD::BuildPair, EVT::I32, &[lo, hi]);
                let mut ops = self.dag.node(node).ops.clone();
                ops[2] = dword;
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
            ISD::Load if at == 1 => self.expand_far_address(node, at),
            x86isd::LES | x86isd::LDS if at == 1 => self.expand_far_address(node, at),
            ISD::Store if at == 2 => self.expand_far_address(node, at),
            super::x86isd::CALL if at == 1 => self.expand_far_call(node),
            x86isd::LLRM_PTRDIFF => self.expand_ptrdiff(node)?,
            ISD::BitCast if self.dag.value_type(node).is_floating_point() => {
                // `ExpandOp_BITCAST` to a float: the halves stored to a stack
                // slot, the float loaded from it.
                let (lo, hi) = self.expanded_integer(self.dag.operand(node, 0));
                let vt = self.dag.value_type(node);
                let size = vt.size_in_bits().div_ceil(8);
                let cell = self.dag.create_stack_temporary(size, 4);
                let pointer = self.dag.config().pointer;
                let slot = self.dag.get_frame_index(pointer, cell);
                let half = self.dag.value_type(lo);
                let info = super::dag::MemInfo {
                    mem_vt: half,
                    align: 4,
                    volatile: false,
                    base: super::dag::MemBase::Stack(cell),
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                let step = self.dag.get_constant(u128::from(half.size_in_bits() / 8), pointer);
                let high_slot = self.dag.get_node(ISD::Add, pointer, &[slot, step]);
                let entry = self.dag.entry_node();
                let (low_store, high_store) =
                    (self.dag.get_store(entry, lo, slot, info), self.dag.get_store(entry, hi, high_slot, info));
                let stores = self.dag.get_token_factor(&[low_store, high_store]);
                let load = super::dag::MemInfo { mem_vt: vt, ..info };
                self.dag.get_load(vt, stores, slot, load)
            }
            ISD::BitCast => {
                // `ExpandOp_BITCAST` to a same-size integer: the halves
                // joined.
                let (lo, hi) = self.expanded_integer(self.dag.operand(node, 0));
                self.dag.get_node(ISD::BuildPair, self.dag.value_type(node), &[lo, hi])
            }
            ISD::SetCC
                if matches!(self.setcc_code(node), Some(CondCode::SetLt | CondCode::SetGe))
                    && self.is_null_wide(self.dag.operand(node, 1)) =>
            {
                // `x < 0` and `x >= 0` on a wide value are the sign of its high
                // half (`ExpandIntegerSetCCOperands`).
                let (_, hi) = self.expanded_integer(self.dag.operand(node, 0));
                let zero = self.dag.get_constant(0, self.dag.value_type(hi));
                let code = self.dag.operand(node, 2);
                self.dag.get_node(ISD::SetCC, self.dag.value_type(node), &[hi, zero, code])
            }
            ISD::SetCC => match self.setcc_code(node) {
                Some(code) => self.expand_setcc(node, code),
                None => {
                    return Err(LegalizeError::Unsupported {
                        op: ISD::SetCC,
                        ty: self.dag.value_type(self.dag.operand(node, at)),
                        what: "expand operand",
                    });
                }
            },
            op => {
                return Err(LegalizeError::Unsupported {
                    op,
                    ty: self.dag.value_type(self.dag.operand(node, at)),
                    what: "expand operand",
                });
            }
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
        let mut seen = HashSet::with_capacity_and_hasher(self.dag.nodes().len(), Default::default());
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

/// A memory type that is not a power of two wide, as the power of two below it
/// and what is left (`i48`: `i32` and `i16`); none if the rest is no type.
fn odd_memory(mem: EVT) -> Option<(EVT, EVT)> {
    let bits = mem.size_in_bits();
    if bits.is_power_of_two() {
        return None;
    }
    let round = bits.next_power_of_two() / 2;
    let extra = bits - round;
    (extra.is_power_of_two() && extra >= 8).then(|| (EVT::integer(round), EVT::integer(extra)))
}
