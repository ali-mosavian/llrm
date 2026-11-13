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
use llrm_sdag::actions::Action;
use llrm_sdag::lowering::ShiftAmount;
use llrm_support::hash::HashMap;

use super::actions::TargetLowering;
use super::dag::{BooleanContents, NodeData, Rauw, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::libcalls::{self, Cmp3, Libcall};
use super::lowering::TypeAction;
use super::mvt::{EVT, MVT};

mod vector;

/// The width of the C `int` the soft-float routines take for a conversion: a
/// 32-bit integer is passed as is, a narrower one is extended to it.
const INT_BITS: u32 = 32;

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
    /// A vector value and its two halves (`SplitVectors`).
    split: HashMap<SDValue, (SDValue, SDValue)>,
    /// A float value and the integer of its width that stands for it
    /// (`SoftenedFloats`).
    softened: HashMap<SDValue, SDValue>,
    /// The target whose routines a division calls (`None`: no library calls).
    target: Option<&'a dyn llrm_target::Target>,
    /// The target's custom lowerings, asked before a node with an illegal type
    /// is promoted or expanded (`CustomLowerNode`).
    hooks: Option<&'a mut dyn super::legalize_dag::Hooks>,
}

impl<'a> TypeLegalizer<'a> {
    /// `getShiftAmountConstant`: `k` in the type the target shifts `vt` by.
    fn shift_const(
        &mut self,
        k: u128,
        vt: EVT,
    ) -> SDValue {
        let amount_vt = self.shift_amount_type(vt);
        self.dag.get_constant(k, amount_vt)
    }

    /// `getShiftAmountTy` for a shift of `vt` (`i8` on x86).
    /// The type a library shift takes its amount in: `EVT::getIntegerVT(
    /// getIntSize())`.
    fn libcall_shift_amount_type(
        &self,
        vt: EVT,
    ) -> EVT {
        if self.tli.types.narrow_library_shift() {
            return self.shift_amount_type(vt);
        }
        EVT::integer(self.tli.types.int_size())
    }

    fn shift_amount_type(
        &self,
        vt: EVT,
    ) -> EVT {
        match self.tli.types.shift_amount() {
            ShiftAmount::I8 => EVT::I8,
            ShiftAmount::ByValue => vt,
            ShiftAmount::Fixed(fixed) => EVT::Simple(fixed),
        }
    }

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
            split: HashMap::default(),
            softened: HashMap::default(),
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
                TypeAction::SoftenFloat => {
                    self.soften_float_result(value)?;
                    return Ok(true);
                }
                TypeAction::SplitVector => {
                    if !self.custom_lower_node(n, vt) {
                        self.split_vector_result(value)?;
                    }
                    return Ok(true);
                }
                // The builder creates a vector only where a register class
                // holds it.
                TypeAction::ScalarizeVector => {
                    let op = self.dag.node(value).opcode;
                    return Err(LegalizeError::Unsupported { op, ty: vt, what: "vector with no register class" });
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
                TypeAction::SoftenFloat => self.soften_float_operand(n, at)?,
                TypeAction::SplitVector => self.split_vector_operand(n, at)?,
                TypeAction::ScalarizeVector => {
                    let op = self.dag.node(SDValue { node: n, res: 0 }).opcode;
                    return Err(LegalizeError::Unsupported {
                        op,
                        ty: operand_vt,
                        what: "vector with no register class",
                    });
                }
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
        self.shift_const(u128::from(count), shifted)
    }

    /// `GetSoftenedFloat`.
    fn softened_float(
        &self,
        value: SDValue,
    ) -> SDValue {
        let value = self.remap(value);
        *self.softened.get(&value).expect("operand softened before its user")
    }

    /// `SetSoftenedFloat`.
    fn set_softened_float(
        &mut self,
        value: SDValue,
        result: SDValue,
    ) {
        let value = self.remap(value);
        let result = self.analyze_new_value(result);
        self.softened.insert(value, result);
    }

    /// `SoftenFloatResult`: a float result becomes the integer of its width,
    /// the operation a call to the routine the target names for it.
    fn soften_float_result(
        &mut self,
        value: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(value);
        let (_, int) = self.tli.types.type_conversion(vt);
        let result = match self.dag.opcode(value) {
            op @ (ISD::FAdd | ISD::FSub | ISD::FMul | ISD::FDiv) => {
                // `SoftenFloatRes_Binary`: `makeLibCall` of the softened
                // operands.
                let args =
                    [self.softened_float(self.dag.operand(value, 0)), self.softened_float(self.dag.operand(value, 1))];
                let call = self.tli.libcall_name(op, vt).and_then(|name| self.soft_call(name, &args, int));
                call.ok_or(LegalizeError::Unsupported { op, ty: vt, what: "soften: the target names no routine" })?
            }
            ISD::Load => {
                // `SoftenFloatRes_LOAD`: the same load as an integer.
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("a load has a memory operand")
                };
                let load = self.dag.get_load(int, chain, ptr, super::dag::MemInfo { mem_vt: int, ..info });
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: load.node, res: 1 });
                load
            }
            ISD::VAArg => {
                // `SoftenFloatRes_VAARG`: the same read as an integer.
                let ops: Vec<SDValue> = (0..4).map(|at| self.dag.operand(value, at)).collect();
                let read = self.dag.get_node_multi(ISD::VAArg, &[int, EVT::OTHER], &ops);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: read.node, res: 1 });
                SDValue { node: read.node, res: 0 }
            }
            op @ (ISD::SIntToFp | ISD::UIntToFp | ISD::FpExtend | ISD::FpRound) => {
                // `SoftenFloatRes_XINT_TO_FP`, `_FP_EXTEND`, `_FP_ROUND`: the
                // conversion routine over the operand as the integer that
                // stands for it.
                let operand = self.dag.operand(value, 0);
                let from = self.dag.value_type(operand);
                let unsupported = |what| LegalizeError::Unsupported { op, ty: vt, what };
                let (arg, call) = if from.is_floating_point() {
                    let (EVT::Simple(from), EVT::Simple(to)) = (from, vt) else {
                        return Err(unsupported("soften a conversion between these floats"));
                    };
                    (
                        self.softened_float(operand),
                        if op == ISD::FpExtend { Libcall::FpExt(from, to) } else { Libcall::FpRound(from, to) },
                    )
                } else {
                    let bits = from.size_in_bits();
                    // DEVIATE SOFT-UITOFP-NONNEG: the signed routine for a value known non-negative (`nneg`, or
                    // a clear sign bit), where the target asks (`TargetLowering::signed_conversion_of_nonneg`)
                    let nonneg = op == ISD::UIntToFp
                        && self.tli.signed_conversion_of_nonneg()
                        && (self.dag.node(value).flags.0 & SDNodeFlags::NON_NEG.0 != 0
                            || bits < INT_BITS
                            || (bits <= INT_BITS
                                && super::known_bits::masked_value_is_zero(self.dag, operand, 1u128 << (bits - 1))));
                    let op = if nonneg { ISD::SIntToFp } else { op };
                    let extension = if op == ISD::SIntToFp { ISD::SignExtend } else { ISD::ZeroExtend };
                    let arg =
                        if bits < INT_BITS { self.dag.get_node(extension, EVT::I32, &[operand]) } else { operand };
                    let (EVT::Simple(to), true) = (vt, op != ISD::FpExtend) else {
                        return Err(unsupported("soften a conversion to this float"));
                    };
                    let int = if bits <= INT_BITS { MVT::I32 } else { MVT::I64 };
                    (arg, if op == ISD::SIntToFp { Libcall::SIntToFp(int, to) } else { Libcall::UIntToFp(int, to) })
                };
                let name = libcalls::name(call).ok_or(unsupported("soften a conversion between these types"))?;
                self.soft_c_call(name, &[arg], int, op, vt)?
            }
            ISD::FNeg => {
                // `SoftenFloatRes_FNEG`: the sign bit flipped.
                let sign = self.dag.get_constant(1u128 << (int.size_in_bits() - 1), int);
                self.dag.get_node(ISD::Xor, int, &[self.softened_float(self.dag.operand(value, 0)), sign])
            }
            ISD::FAbs => {
                // `SoftenFloatRes_FABS`: the sign bit cleared.
                let mask = self.dag.get_constant((1u128 << (int.size_in_bits() - 1)) - 1, int);
                self.dag.get_node(ISD::And, int, &[self.softened_float(self.dag.operand(value, 0)), mask])
            }
            ISD::Select => {
                // `SoftenFloatRes_SELECT`: a select of the integers.
                let (cond, a, b) = (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                self.dag.get_node(ISD::Select, int, &[cond, self.softened_float(a), self.softened_float(b)])
            }
            ISD::ConstantFP => {
                // `SoftenFloatRes_ConstantFP`: the constant's bits.
                let NodeData::ConstFp { bits } = self.dag.node(value).data else {
                    unreachable!("a float constant has bits")
                };
                self.dag.get_constant(bits, int)
            }
            // `SoftenFloatRes_UNDEF`
            ISD::Undef => self.dag.get_undef(int),
            ISD::BitCast if !self.dag.value_type(self.dag.operand(value, 0)).is_floating_point() => {
                // `SoftenFloatRes_BITCAST`: the integer it was cast from.
                self.dag.operand(value, 0)
            }
            op => return Err(LegalizeError::Unsupported { op, ty: vt, what: "soften a float result" }),
        };
        self.set_softened_float(value, result);
        Ok(())
    }

    /// `SoftenFloatOperand`: a float operand read as the integer that
    /// stands for it. True if `n` is to be looked at again.
    fn soften_float_operand(
        &mut self,
        n: u32,
        at: usize,
    ) -> Result<bool, LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        let result = match self.dag.opcode(node) {
            ISD::Store if at == 1 => {
                // `SoftenFloatOp_STORE`: the integer stored in its place.
                let (chain, value, ptr) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let bits = self.softened_float(value);
                let int = self.dag.value_type(bits);
                self.dag.get_store(chain, bits, ptr, super::dag::MemInfo { mem_vt: int, ..info })
            }
            ISD::BitCast => self.softened_float(self.dag.operand(node, 0)),
            ISD::SetCC if at < 2 => self.soften_setcc(node)?,
            op @ (ISD::FpToSInt | ISD::FpToUInt) => {
                // `SoftenFloatOp_FP_TO_XINT`: the routine over the integer
                // that stands for the float.
                let to = self.dag.value_type(node);
                let arg = self.softened_float(self.dag.operand(node, 0));
                let wide = to.size_in_bits() > 32;
                let name = self
                    .fp_to_xint_name(op == ISD::FpToSInt, wide, self.dag.value_type(self.dag.operand(node, 0)))
                    .ok_or(LegalizeError::Unsupported {
                        op,
                        ty: to,
                        what: "soften a conversion between these types",
                    })?;
                let call = self.soft_c_call(name, &[arg], if wide { to } else { EVT::I32 }, op, to)?;
                if to.size_in_bits() < 32 { self.dag.get_node(ISD::Truncate, to, &[call]) } else { call }
            }
            ISD::CopyToReg if at == 2 => {
                // `SoftenFloatOp_COPY_TO_REG`'s effect: the integer is copied.
                let mut ops = self.dag.node(node).ops.clone();
                ops[2] = self.softened_float(ops[2]);
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
            op => {
                return Err(LegalizeError::Unsupported {
                    op,
                    ty: self.dag.value_type(self.dag.operand(node, at)),
                    what: "soften operand",
                });
            }
        };
        // Updated in place (`UpdateNodeOperands` found no twin): look at it
        // again.
        if result.node == n {
            return Ok(true);
        }
        self.replace_value_with(node, result);
        Ok(false)
    }

    /// compiler-rt's float-to-integer routine (`RTLIB::getFPTOSINT`).
    fn fp_to_xint_name(
        &self,
        signed: bool,
        wide: bool,
        from: EVT,
    ) -> Option<&'static str> {
        let EVT::Simple(from) = from else { return None };
        let int = if wide { MVT::I64 } else { MVT::I32 };
        libcalls::name(if signed { Libcall::FpToSInt(from, int) } else { Libcall::FpToUInt(from, int) })
    }

    /// `makeLibCall` of the C routine `name` over `args`, its result of type
    /// `ret`.
    fn soft_c_call(
        &mut self,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        op: ISD,
        vt: EVT,
    ) -> Result<SDValue, LegalizeError> {
        self.soft_call(name, args, ret).ok_or(LegalizeError::Unsupported {
            op,
            ty: vt,
            what: "the routine cannot be called",
        })
    }

    /// `makeLibCall` of a softened float's routine: the target's own call
    /// lowering when it has one (`Hooks::make_lib_call`), else the C
    /// convention over the target's calling description.
    fn soft_call(
        &mut self,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
    ) -> Option<SDValue> {
        if let Some(hooks) = self.hooks.as_mut() {
            if let Some(call) = hooks.make_lib_call(self.dag, name, args, ret, false) {
                return Some(call);
            }
        }
        let arch = self.target?;
        let env = super::libcall::CallEnv {
            arch,
            pointer: self.dag.config().pointer,
            call: super::builder::LoweringNodes::installed().call.expect("a target with a call node"),
        };
        super::libcall::c_lib_call(
            self.dag,
            &env,
            &self.tli.libcall_symbol(name),
            self.tli.libcall_convention(name),
            args,
            ret,
        )
    }

    /// `SoftenFloatOp_SETCC` over `softenSetCCOperands`: the compare as
    /// compiler-rt's three-way routine and a compare of its result with zero.
    /// An unordered-or-less code is the inverse of an ordered one; the two that
    /// no routine answers alone (`one`, `ueq`) are two calls joined.
    fn soften_setcc(
        &mut self,
        node: SDValue,
    ) -> Result<SDValue, LegalizeError> {
        let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let float_vt = self.dag.value_type(lhs);
        let unsupported = |what| LegalizeError::Unsupported { op: ISD::SetCC, ty: float_vt, what };
        let Some(code) = self.setcc_code(node) else { return Err(unsupported("soften a compare with no code")) };
        let (a, b) = (self.softened_float(lhs), self.softened_float(rhs));
        let vt = self.dag.value_type(node);
        let EVT::Simple(float_mvt) = float_vt else { return Err(unsupported("soften a compare of this float")) };
        let pick = |cmp: Cmp3| libcalls::name(Libcall::Fcmp3(cmp, float_mvt));
        let compare = |this: &mut Self, cmp: Cmp3, cc: CondCode| -> Result<SDValue, LegalizeError> {
            // The routines return a C `int`: the pointer's width up to a long
            // (16 at m16, 32 above).
            let int = EVT::integer(this.dag.config().pointer.size_in_bits().min(32));
            let result = pick(cmp)
                .and_then(|name| this.soft_call(name, &[a, b], int))
                .ok_or_else(|| unsupported("the compare routine cannot be called"))?;
            let zero = this.dag.get_constant(0, int);
            Ok(this.dag.get_setcc(vt, result, zero, cc))
        };
        use CondCode::*;
        Ok(match code {
            SetOEq | SetEq => compare(self, Cmp3::Oeq, SetEq)?,
            SetUNe | SetNe => compare(self, Cmp3::Une, SetNe)?,
            SetOGe | SetGe => compare(self, Cmp3::Oge, SetGe)?,
            SetOLt | SetLt => compare(self, Cmp3::Olt, SetLt)?,
            SetOLe | SetLe => compare(self, Cmp3::Ole, SetLe)?,
            SetOGt | SetGt => compare(self, Cmp3::Ogt, SetGt)?,
            SetO => compare(self, Cmp3::Uo, SetEq)?,
            SetUo => compare(self, Cmp3::Uo, SetNe)?,
            // unordered or less: not (ordered and greater or equal)
            SetULt => compare(self, Cmp3::Oge, SetLt)?,
            SetULe => compare(self, Cmp3::Ogt, SetLe)?,
            SetUGt => compare(self, Cmp3::Ole, SetGt)?,
            SetUGe => compare(self, Cmp3::Olt, SetGe)?,
            SetONe => {
                let (lt, gt) = (compare(self, Cmp3::Olt, SetLt)?, compare(self, Cmp3::Ogt, SetGt)?);
                self.dag.get_node(ISD::Or, vt, &[lt, gt])
            }
            SetUEq => {
                let (unord, eq) = (compare(self, Cmp3::Uo, SetNe)?, compare(self, Cmp3::Oeq, SetEq)?);
                self.dag.get_node(ISD::Or, vt, &[unord, eq])
            }
            _ => return Err(unsupported("soften a compare with a constant or integer code")),
        })
    }

    /// `GetPromotedInteger`.
    fn promoted_integer(
        &self,
        value: SDValue,
    ) -> SDValue {
        let value = self.remap(value);
        *self.promoted.get(&value).expect("operand promoted before its user")
    }

    /// `SExtPromotedInteger`: the promoted value with its bits above `value`'s
    /// own width made copies of the sign.
    fn sext_promoted_integer(
        &mut self,
        value: SDValue,
    ) -> SDValue {
        let narrow = self.dag.get_value_type_node(self.dag.value_type(value));
        let promoted = self.promoted_integer(value);
        let wide = self.dag.value_type(promoted);
        self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, narrow])
    }

    /// `ZExtPromotedInteger`: the promoted value with its bits above `value`'s
    /// own width cleared.
    fn zext_promoted_integer(
        &mut self,
        value: SDValue,
    ) -> SDValue {
        let bits = self.dag.value_type(value).size_in_bits();
        let promoted = self.promoted_integer(value);
        let wide = self.dag.value_type(promoted);
        let mask = self.dag.get_constant(super::fold::mask(bits), wide);
        self.dag.get_node(ISD::And, wide, &[promoted, mask])
    }

    /// `PromoteIntRes_ADDSUBSHLSAT`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L1067>).
    ///
    /// DEVIATE: `SExtOrZExtPromotedOperands` (`usubsat`) always inserts the
    /// extension where the pin skips it for operands whose known bits already
    /// fit; the combiner removes it. Exit: when that helper is translated.
    fn promote_saturating(
        &mut self,
        value: SDValue,
        op: ISD,
        to: EVT,
    ) -> SDValue {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let old = self.dag.value_type(lhs);
        let old_bits = old.size_in_bits();
        let new_bits = to.size_in_bits();
        if op == ISD::USubSat {
            let (lhs, rhs) = if self.tli.is_sext_cheaper_than_zext(old, to) {
                (self.sext_promoted_integer(lhs), self.sext_promoted_integer(rhs))
            } else {
                (self.zext_promoted_integer(lhs), self.zext_promoted_integer(rhs))
            };
            return self.dag.get_node(ISD::USubSat, to, &[lhs, rhs]);
        }
        if op == ISD::UAddSat {
            if self.tli.is_sext_cheaper_than_zext(old, to) {
                let (lhs, rhs) = (self.sext_promoted_integer(lhs), self.sext_promoted_integer(rhs));
                return self.dag.get_node(ISD::UAddSat, to, &[lhs, rhs]);
            }
            let (lhs, rhs) = (self.zext_promoted_integer(lhs), self.zext_promoted_integer(rhs));
            let top = self.dag.get_constant(super::fold::mask(old_bits), to);
            let sum = self.dag.get_node(ISD::Add, to, &[lhs, rhs]);
            return self.dag.get_node(ISD::UMin, to, &[sum, top]);
        }
        let shift = matches!(op, ISD::SShlSat | ISD::UShlSat);
        let (mut lhs, mut rhs) = if shift {
            let amount = if self.action(self.dag.value_type(rhs)) == TypeAction::PromoteInteger {
                self.zext_promoted_integer(rhs)
            } else {
                rhs
            };
            (self.promoted_integer(lhs), amount)
        } else {
            (self.sext_promoted_integer(lhs), self.sext_promoted_integer(rhs))
        };
        // A shift cannot be a min and a max: no overflow is seen once every
        // bit has been shifted out.
        if shift || self.tli.is_operation_legal(op, to) {
            let back = if op == ISD::UShlSat { ISD::Srl } else { ISD::Sra };
            let amount = self.shift_const(u128::from(new_bits - old_bits), to);
            lhs = self.dag.get_node(ISD::Shl, to, &[lhs, amount]);
            if !shift {
                rhs = self.dag.get_node(ISD::Shl, to, &[rhs, amount]);
            }
            let result = self.dag.get_node(op, to, &[lhs, rhs]);
            return self.dag.get_node(back, to, &[result, amount]);
        }
        let combine = if op == ISD::SAddSat { ISD::Add } else { ISD::Sub };
        let widen = |value: i128| (value as u128) & super::fold::mask(new_bits);
        let min = self.dag.get_constant(widen(-(1_i128 << (old_bits - 1))), to);
        let max = self.dag.get_constant(widen((1_i128 << (old_bits - 1)) - 1), to);
        let result = self.dag.get_node(combine, to, &[lhs, rhs]);
        let result = self.dag.get_node(ISD::SMin, to, &[result, max]);
        self.dag.get_node(ISD::SMax, to, &[result, min])
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
                        let ty = legalizer.dag.get_value_type_node(legalizer.dag.value_type(operand));
                        legalizer.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
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
            op @ (ISD::SDiv | ISD::SRem | ISD::UDiv | ISD::URem) => {
                // `PromoteIntRes_SDIV/UDIV`: both operands extended in the
                // register, signed for a signed divide, zero
                // for an unsigned one.
                let signed = matches!(op, ISD::SDiv | ISD::SRem);
                let mut extended = Vec::with_capacity(2);
                for at in 0..2 {
                    let operand = self.dag.operand(value, at);
                    let bits = self.dag.value_type(operand).size_in_bits();
                    let promoted = self.promoted_integer(operand);
                    let wide = self.dag.value_type(promoted);
                    extended.push(if signed {
                        let ty = self.dag.get_value_type_node(self.dag.value_type(operand));
                        self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
                    } else {
                        let mask = self.dag.get_constant(super::fold::mask(bits), wide);
                        self.dag.get_node(ISD::And, wide, &[promoted, mask])
                    });
                }
                let wide = self.dag.value_type(extended[0]);
                self.dag.get_node(op, wide, &extended)
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
                        let ty = self.dag.get_value_type_node(self.dag.value_type(value));
                        self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
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
            op @ (ISD::AtomicSwap
            | ISD::AtomicLoadAdd
            | ISD::AtomicLoadSub
            | ISD::AtomicLoadAnd
            | ISD::AtomicLoadClr
            | ISD::AtomicLoadOr
            | ISD::AtomicLoadXor) => {
                // `PromoteIntRes_Atomic1`: the access keeps its memory type,
                // the operand is the promoted value, the result
                // is wider and the chain follows.
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("an atomic has a memory operand")
                };
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let operand = self.promoted_integer(self.dag.operand(value, 2));
                let new = if op == ISD::AtomicSwap {
                    self.dag.get_atomic_swap(chain, ptr, operand, info)
                } else {
                    self.dag.get_atomic_rmw(op, chain, ptr, operand, info)
                };
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: new.node, res: 1 });
                new
            }
            ISD::AtomicCmpSwapWithSuccess if value.res == 1 => {
                // `PromoteIntRes_AtomicCmpSwap`: only the success flag is
                // widened.
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("an atomic has a memory operand")
                };
                let ops: Vec<SDValue> = (0..4).map(|at| self.dag.operand(value, at)).collect();
                let new = self.dag.get_atomic_cmp_swap(Some(to), ops[0], ops[1], ops[2], ops[3], info);
                self.replace_value_with(SDValue { node: value.node, res: 0 }, SDValue { node: new.node, res: 0 });
                self.replace_value_with(SDValue { node: value.node, res: 2 }, SDValue { node: new.node, res: 2 });
                SDValue { node: new.node, res: 1 }
            }
            ISD::AtomicLoad => {
                // `PromoteIntRes_Atomic0`: a zero-extending atomic load.
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("an atomic has a memory operand")
                };
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let new = self
                    .dag
                    .get_atomic_load(
                        to,
                        chain,
                        ptr,
                        super::dag::MemInfo { ext: Some(super::actions::LoadExt::ZExtLoad), ..info },
                    );
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: new.node, res: 1 });
                new
            }
            ISD::Load => {
                // The access keeps its memory type; the result is wider
                // (an extending load), and the chain follows.
                let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                let NodeData::Mem(info) = self.dag.node(value).data else {
                    unreachable!("a load has a memory operand")
                };
                if let Some(pieces) = odd_memory(info.mem_vt) {
                    // `PromoteIntRes_LOAD` of memory that is not a power of
                    // two wide (`{ i64, i48 }`, a seven byte bit-field) is
                    // `ExpandLoad`: the low part zero-extended, the rest
                    // above it, piece by piece.
                    let mut joined: Option<SDValue> = None;
                    let mut chains = Vec::new();
                    let mut at = 0u32;
                    for piece in pieces {
                        let piece_ptr = if at == 0 { ptr } else { self.address_plus(ptr, u128::from(at / 8)) };
                        let piece_info = super::dag::MemInfo {
                            mem_vt: piece,
                            align: if at == 0 { info.align } else { info.align.min(at / 8) },
                            ..info
                        };
                        let part =
                            self.dag.get_ext_load(super::actions::LoadExt::ZExtLoad, to, chain, piece_ptr, piece_info);
                        chains.push(SDValue { node: part.node, res: 1 });
                        let part = if at == 0 {
                            part
                        } else {
                            let by = self.shift_const(u128::from(at), to);
                            self.dag.get_node(ISD::Shl, to, &[part, by])
                        };
                        joined = Some(match joined {
                            Some(low) => self.dag.get_node(ISD::Or, to, &[low, part]),
                            None => part,
                        });
                        at += piece.size_in_bits();
                    }
                    let chains = self.dag.get_token_factor(&chains);
                    self.replace_value_with(SDValue { node: value.node, res: 1 }, chains);
                    let joined = joined.expect("a piece");
                    self.set_promoted_integer(value, joined);
                    return Ok(());
                }
                // `PromoteIntRes_LOAD`: a non-extending load becomes an
                // any-extending one.
                let ext = info.ext.unwrap_or(super::actions::LoadExt::ExtLoad);
                let loaded = self.dag.get_ext_load(ext, to, chain, ptr, info);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: loaded.node, res: 1 });
                loaded
            }
            op @ (ISD::SAddSat | ISD::UAddSat | ISD::SSubSat | ISD::USubSat | ISD::SShlSat | ISD::UShlSat) => {
                self.promote_saturating(value, op, to)
            }
            op @ (ISD::SMulFix | ISD::UMulFix | ISD::SMulFixSat | ISD::UMulFixSat) => {
                // `PromoteIntRes_MULFIX`
                // (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L1162>):
                // the operands extended in the promoted type, then the same
                // operation there. A saturating one saturates at the wider
                // width, so one operand is shifted up by the difference and
                // the result shifted back.
                let signed = matches!(op, ISD::SMulFix | ISD::SMulFixSat);
                let saturating = matches!(op, ISD::SMulFixSat | ISD::UMulFixSat);
                let narrow = self.dag.value_type(self.dag.operand(value, 0));
                let narrow_node = self.dag.get_value_type_node(narrow);
                let mut extended = Vec::with_capacity(2);
                for at in 0..2 {
                    let promoted = self.promoted_integer(self.dag.operand(value, at));
                    extended.push(if signed {
                        self.dag.get_node(ISD::SignExtendInReg, to, &[promoted, narrow_node])
                    } else {
                        let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), to);
                        self.dag.get_node(ISD::And, to, &[promoted, mask])
                    });
                }
                let scale = self.dag.operand(value, 2);
                if saturating {
                    let difference = u128::from(to.size_in_bits() - narrow.size_in_bits());
                    let up = self.shift_const(difference, to);
                    let lhs = self.dag.get_node(ISD::Shl, to, &[extended[0], up]);
                    let result = self.dag.get_node(op, to, &[lhs, extended[1], scale]);
                    let back = self.shift_const(difference, to);
                    self.dag.get_node(if signed { ISD::Sra } else { ISD::Srl }, to, &[result, back])
                } else {
                    self.dag.get_node(op, to, &[extended[0], extended[1], scale])
                }
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
            ISD::LRint | ISD::LLRint => {
                // `PromoteIntRes_XRINT`: the same rounding to the wider type.
                let input = self.dag.operand(value, 0);
                self.dag.get_node(self.dag.opcode(value), to, &[input])
            }
            ISD::SignExtendInReg => {
                // `PromoteIntRes_SIGN_EXTEND_INREG`: the promoted operand,
                // extended from the same type.
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                let from = self.dag.operand(value, 1);
                self.dag.get_node(ISD::SignExtendInReg, self.dag.value_type(promoted), &[promoted, from])
            }
            op @ (ISD::UMulO | ISD::SMulO) if value.res == 0 => {
                // `PromoteIntRes_XMULO` (result 0): the multiply in the wider
                // type on extended operands; it overflowed there, or its high
                // part is not the extension of its low part.
                let signed = op == ISD::SMulO;
                let narrow = self.dag.value_type(self.dag.operand(value, 0));
                let ty = self.dag.get_value_type_node(narrow);
                let mut extended = Vec::with_capacity(2);
                for at in 0..2 {
                    let promoted = self.promoted_integer(self.dag.operand(value, at));
                    let wide = self.dag.value_type(promoted);
                    extended.push(if signed {
                        self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
                    } else {
                        let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), wide);
                        self.dag.get_node(ISD::And, wide, &[promoted, mask])
                    });
                }
                let flag_vt = self.dag.value_type(SDValue { node: value.node, res: 1 });
                let product = self.dag.get_node_multi(op, &[to, flag_vt], &[extended[0], extended[1]]);
                let mul = SDValue { node: product.node, res: 0 };
                let high_part = if signed {
                    let extended_low = self.dag.get_node(ISD::SignExtendInReg, to, &[mul, ty]);
                    self.dag.get_setcc(flag_vt, extended_low, mul, CondCode::SetNe)
                } else {
                    let shift = self.shift_const(u128::from(narrow.size_in_bits()), to);
                    let high = self.dag.get_node(ISD::Srl, to, &[mul, shift]);
                    let zero = self.dag.get_constant(0, to);
                    self.dag.get_setcc(flag_vt, high, zero, CondCode::SetNe)
                };
                let wide_overflow = SDValue { node: product.node, res: 1 };
                let overflow = self.dag.get_node(ISD::Or, flag_vt, &[high_part, wide_overflow]);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
                mul
            }
            op @ (ISD::UAddO | ISD::USubO | ISD::SAddO | ISD::SSubO) if value.res == 0 => {
                // `PromoteIntRes_UADDSUBO` and `PromoteIntRes_SADDSUBO`
                // (result 0): the arithmetic in the wider type on operands
                // extended from the narrow one; it overflowed iff the result
                // is not the extension of its truncation.
                let signed = matches!(op, ISD::SAddO | ISD::SSubO);
                let narrow = self.dag.value_type(self.dag.operand(value, 0));
                let ty = self.dag.get_value_type_node(narrow);
                let mut extended = Vec::with_capacity(2);
                for at in 0..2 {
                    let promoted = self.promoted_integer(self.dag.operand(value, at));
                    let wide = self.dag.value_type(promoted);
                    extended.push(if signed {
                        self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
                    } else {
                        let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), wide);
                        self.dag.get_node(ISD::And, wide, &[promoted, mask])
                    });
                }
                let add = matches!(op, ISD::UAddO | ISD::SAddO);
                let sum = self.dag.get_node(if add { ISD::Add } else { ISD::Sub }, to, &[extended[0], extended[1]]);
                let truncated = if signed {
                    self.dag.get_node(ISD::SignExtendInReg, to, &[sum, ty])
                } else {
                    let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), to);
                    self.dag.get_node(ISD::And, to, &[sum, mask])
                };
                let flag_vt = self.dag.value_type(SDValue { node: value.node, res: 1 });
                let overflow = self.dag.get_setcc(flag_vt, truncated, sum, CondCode::SetNe);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, overflow);
                sum
            }
            ISD::UAddO
            | ISD::USubO
            | ISD::UAddOCarry
            | ISD::USubOCarry
            | ISD::SAddOCarry
            | ISD::SSubOCarry
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
            op @ (ISD::FpToSInt | ISD::FpToUInt) => {
                // `PromoteIntRes_FP_TO_XINT`: the wider conversion (signed when
                // only that one is legal), asserted to fit the
                // type it had.
                let input = self.dag.operand(value, 0);
                let wide_op = if op == ISD::FpToUInt
                    && !self.tli.is_operation_legal(ISD::FpToUInt, to)
                    && self.tli.is_operation_legal_or_custom(ISD::FpToSInt, to)
                {
                    ISD::FpToSInt
                } else {
                    op
                };
                let converted = self.dag.get_node(wide_op, to, &[input]);
                let ty = self.dag.get_value_type_node(vt);
                self.dag.get_node(
                    if op == ISD::FpToUInt { ISD::AssertZext } else { ISD::AssertSext },
                    to,
                    &[converted, ty],
                )
            }
            op @ (ISD::CtPop | ISD::CtLz | ISD::CtLzZeroPoison) => {
                // `PromoteIntRes_CTLZ` and `PromoteIntRes_CTPOP`: the count of
                // the zero-extended value in the wider type, less the bits
                // added above the narrow one (none for a population count).
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                let mask = self.dag.get_constant(super::fold::mask(vt.size_in_bits()), to);
                let extended = self.dag.get_node(ISD::And, to, &[promoted, mask]);
                let count = self.dag.get_node(op, to, &[extended]);
                if op == ISD::CtPop {
                    count
                } else {
                    let added = self.dag.get_constant(u128::from(to.size_in_bits() - vt.size_in_bits()), to);
                    self.dag.get_node(ISD::Sub, to, &[count, added])
                }
            }
            op @ (ISD::CtTz | ISD::CtTzZeroPoison) => {
                // `PromoteIntRes_CTTZ`: the count is the same in the wider
                // type except for zero, which setting the bit just above the
                // narrow type makes the narrow width.
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                let wide = if op == ISD::CtTz {
                    let top = self.dag.get_constant(1u128 << vt.size_in_bits(), to);
                    self.dag.get_node(ISD::Or, to, &[promoted, top])
                } else {
                    promoted
                };
                self.dag.get_node(op, to, &[wide])
            }
            op @ (ISD::Abs | ISD::AbsMinPoison) => {
                // `PromoteIntRes_ABS`: the absolute value of the
                // sign-extended operand.
                let promoted = self.sext_promoted_integer(self.dag.operand(value, 0));
                let wide = self.dag.value_type(promoted);
                self.dag.get_node(op, wide, &[promoted])
            }
            op @ (ISD::BitReverse | ISD::BSwap) => {
                // `PromoteIntRes_BSWAP` and `PromoteIntRes_BITREVERSE`: the
                // wider reversal, shifted down by the bits added.
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                // "If the larger BITREVERSE isn't supported by the target, try
                // to expand now": the narrow type keeps the work small.
                if op == ISD::BitReverse && !self.tli.is_operation_legal_or_custom_or_promote(op, to) {
                    if let Some(expanded) = super::legalize_dag::expand_bitreverse(self.dag, value) {
                        let wide = self.dag.get_node(ISD::AnyExtend, to, &[expanded]);
                        self.set_promoted_integer(value, wide);
                        return Ok(());
                    }
                }
                let reversed = self.dag.get_node(op, to, &[promoted]);
                let added = self.shift_const(u128::from(to.size_in_bits() - vt.size_in_bits()), to);
                self.dag.get_node(ISD::Srl, to, &[reversed, added])
            }
            ISD::BitCast => {
                // `PromoteIntRes_BITCAST`: through a stack slot, the value
                // stored as its own type and loaded as the narrow integer.
                let src = self.dag.operand(value, 0);
                let (slot, store, cell) = self.store_to_slot(src);
                let info = super::dag::MemInfo { mem_vt: vt, ..self.slot_info(src, cell) };
                let load = self.dag.get_load(vt, store, slot, info);
                self.dag.get_node(ISD::AnyExtend, to, &[load])
            }
            op @ (ISD::AssertSext | ISD::AssertZext) => {
                // `PromoteIntRes_AssertSext` and `PromoteIntRes_AssertZext`:
                // the new bits are the sign (or zero), and the assertion
                // continues on the wider value.
                let promoted = self.promoted_integer(self.dag.operand(value, 0));
                let wide = if op == ISD::AssertSext {
                    let ty = self.dag.get_value_type_node(vt);
                    self.dag.get_node(ISD::SignExtendInReg, to, &[promoted, ty])
                } else {
                    let mask = self.dag.get_constant(super::fold::mask(vt.size_in_bits()), to);
                    self.dag.get_node(ISD::And, to, &[promoted, mask])
                };
                self.dag.get_node(op, to, &[wide, self.dag.operand(value, 1)])
            }
            op @ (ISD::Rotl | ISD::Rotr) => {
                // `PromoteIntRes_Rotate`
                // (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L1533>):
                // with room for the value twice, a
                // variable rotate is one shift of the value concatenated
                // with itself; otherwise the rotate becomes shifts and ORs
                // (`expandROT`) that promote on their own.
                let amount = self.dag.operand(value, 1);
                let (old_bits, new_bits) = (vt.size_in_bits(), to.size_in_bits());
                let custom_shifts = self.tli.operation_action(ISD::Shl, vt) == super::actions::Action::Custom
                    || self.tli.operation_action(ISD::Srl, vt) == super::actions::Action::Custom;
                if new_bits >= 2 * old_bits
                    && self.dag.constant(amount).is_none()
                    && !self.tli.is_operation_legal_or_custom(op, to)
                    && !custom_shifts
                {
                    let op0 = self.promoted_integer(self.dag.operand(value, 0));
                    let amount = if self.action(self.dag.value_type(amount)) == TypeAction::PromoteInteger {
                        self.zext_promoted_integer(amount)
                    } else {
                        amount
                    };
                    let amount_vt = self.dag.value_type(amount);
                    // The amount is taken modulo the old width.
                    let width = self.dag.get_constant(u128::from(old_bits), amount_vt);
                    let amount = self.dag.get_node(ISD::URem, amount_vt, &[amount, width]);
                    let hi_shift = self.shift_const(u128::from(old_bits), to);
                    let hi = self.dag.get_node(ISD::Shl, to, &[op0, hi_shift]);
                    let mask = self.dag.get_constant(super::fold::mask(old_bits), to);
                    let lo = self.dag.get_node(ISD::And, to, &[op0, mask]);
                    let both = self.dag.get_node(ISD::Or, to, &[hi, lo]);
                    let is_right = op == ISD::Rotr;
                    let rotated = self.dag.get_node(if is_right { ISD::Srl } else { ISD::Shl }, to, &[both, amount]);
                    if is_right { rotated } else { self.dag.get_node(ISD::Srl, to, &[rotated, hi_shift]) }
                } else {
                    let expanded = super::expand_funnel::expand_rotate(self.dag, self.tli, value);
                    self.replace_value_with(value, expanded);
                    return Ok(());
                }
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
            ISD::AtomicStore if at == 1 => {
                // `PromoteIntOp_ATOMIC_STORE`: the promoted value, the same
                // memory type.
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("an atomic has a memory operand")
                };
                let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 2));
                let operand = self.promoted_integer(self.dag.operand(node, 1));
                self.dag.get_atomic_store(chain, ptr, operand, info)
            }
            ISD::Truncate => {
                let operand = self.promoted_integer(self.dag.operand(node, at));
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::Truncate, vt, &[operand])
            }
            ISD::BuildVector => {
                // `PromoteIntOp_BUILD_VECTOR`: the vector is legal and its lane
                // type is not; the operands, all promoted, may be wider than
                // the lanes.
                let ops: Vec<SDValue> =
                    self.dag.node(node).ops.clone().into_iter().map(|op| self.promoted_integer(op)).collect();
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
            ISD::InsertVectorElt if at == 1 => {
                // `PromoteIntOp_INSERT_VECTOR_ELT`: the inserted element, wider
                // than the lane.
                let (vector, index) = (self.dag.operand(node, 0), self.dag.operand(node, 2));
                let element = self.promoted_integer(self.dag.operand(node, 1));
                let updated = self.dag.update_node_operands(n, &[vector, element, index]);
                SDValue { node: updated, res: 0 }
            }
            ISD::ScalarToVector => {
                // `PromoteIntOp_SCALAR_TO_VECTOR`.
                let element = self.promoted_integer(self.dag.operand(node, 0));
                let updated = self.dag.update_node_operands(n, &[element]);
                SDValue { node: updated, res: 0 }
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
                let pieces = odd_memory(info.mem_vt).unwrap_or_else(|| vec![info.mem_vt]);
                let promoted = self.promoted_integer(self.dag.operand(node, 1));
                let wide = self.dag.value_type(promoted);
                let mut stores = Vec::new();
                let mut at = 0u32;
                for piece in pieces {
                    let part = if at == 0 {
                        promoted
                    } else {
                        let by = self.shift_const(u128::from(at), wide);
                        self.dag.get_node(ISD::Srl, wide, &[promoted, by])
                    };
                    let part = self.dag.get_node(ISD::Truncate, piece, &[part]);
                    let piece_ptr = if at == 0 { ptr } else { self.address_plus(ptr, u128::from(at / 8)) };
                    let piece_info = super::dag::MemInfo {
                        mem_vt: piece,
                        align: if at == 0 { info.align } else { info.align.min(at / 8) },
                        ..info
                    };
                    stores.push(self.dag.get_store(chain, part, piece_ptr, piece_info));
                    at += piece.size_in_bits();
                }
                self.dag.get_token_factor(&stores)
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
                // `SExtOrZExtPromotedOperands`: promoted operands whose
                // significant bits fit the narrow type are
                // compared as they are, with no zext_inreg.
                let fits = |legalizer: &Self, value: SDValue| {
                    let promoted = legalizer.promoted_integer(value);
                    let wide = legalizer.dag.value_type(promoted).size_in_bits();
                    let narrow = legalizer.dag.value_type(value).size_in_bits();
                    wide + 1 - super::known_bits::num_sign_bits(legalizer.dag, promoted) <= narrow
                };
                let as_is = !code.is_signed_int() && fits(self, lhs) && fits(self, rhs);
                let extend = |legalizer: &mut Self, value: SDValue| {
                    let bits = legalizer.dag.value_type(value).size_in_bits();
                    let promoted = legalizer.promoted_integer(value);
                    let wide = legalizer.dag.value_type(promoted);
                    if as_is {
                        promoted
                    } else if code.is_signed_int() {
                        let ty = legalizer.dag.get_value_type_node(legalizer.dag.value_type(value));
                        legalizer.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
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
            ISD::UAddOCarry | ISD::USubOCarry | ISD::SAddOCarry | ISD::SSubOCarry if at == 2 => {
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
            op @ (ISD::SIntToFp | ISD::UIntToFp) => {
                // `PromoteIntOp_SINT_TO_FP`, `PromoteIntOp_UINT_TO_FP`: the
                // promoted source extended in the register as
                // the conversion reads it.
                let src = self.dag.operand(node, 0);
                let promoted = self.promoted_integer(src);
                let wide = self.dag.value_type(promoted);
                let from = self.dag.value_type(src);
                let extended = if op == ISD::SIntToFp {
                    let ty = self.dag.get_value_type_node(from);
                    self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty])
                } else {
                    let mask = self.dag.get_constant(super::fold::mask(from.size_in_bits()), wide);
                    self.dag.get_node(ISD::And, wide, &[promoted, mask])
                };
                let updated = self.dag.update_node_operands(n, &[extended]);
                SDValue { node: updated, res: 0 }
            }
            ISD::Shl | ISD::Srl | ISD::Sra if at == 1 => {
                // `PromoteIntOp_Shift`: the amount zero extended in the
                // register.
                let amount = self.dag.operand(node, 1);
                let bits = self.dag.value_type(amount).size_in_bits();
                let promoted = self.promoted_integer(amount);
                let wide = self.dag.value_type(promoted);
                let mask = self.dag.get_constant(super::fold::mask(bits), wide);
                let amount = self.dag.get_node(ISD::And, wide, &[promoted, mask]);
                let updated = self.dag.update_node_operands(n, &[self.dag.operand(node, 0), amount]);
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
                    && matches!(
                        self.dag.value_type(self.dag.operand(node, 1)).size_in_bits(),
                        1 | 8 | 16 | 32 | 64 | 128
                    ) =>
            {
                // `PromoteIntOp_STORE`: a truncating store of the promoted
                // value (of an `i1`, which `LegalizeDAG` widens to a byte).
                let (chain, value, ptr) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let NodeData::Mem(info) = self.dag.node(node).data else {
                    unreachable!("a store has a memory operand")
                };
                let promoted = self.promoted_integer(value);
                if self.dag.value_type(value) == EVT::I1 {
                    // A one-bit memory is a byte with the upper bits zero
                    // (`LegalizeDAG`'s truncating store of a non-byte width:
                    // `getZeroExtendInReg`, then the byte-sized store).
                    let wide = self.dag.value_type(promoted);
                    let one = self.dag.get_constant(1, wide);
                    let bit = self.dag.get_node(ISD::And, wide, &[promoted, one]);
                    self.dag.get_store(chain, bit, ptr, super::dag::MemInfo { mem_vt: EVT::I8, ..info })
                } else {
                    self.dag.get_store(chain, promoted, ptr, info)
                }
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
        // `ReplaceNodeWith` deletes the node it replaced, so what it read no
        // longer counts it as a user.
        self.dag.delete_replaced(n);
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
        // A division expanded with its sibling (DEVIATE DIVREM-HELPER) is done.
        if self.expanded.contains_key(&self.remap(value)) {
            return Ok(());
        }
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
            // `ExpandRes_MERGE_VALUES`: each result is its operand, expanded.
            ISD::MergeValues => self.expanded_integer(self.dag.operand(value, value.res as usize)),
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
                    let by = self.shift_const(u128::from(half.size_in_bits() - 1), half);
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
                        self.dag.config().nodes.far_load_es.expect("a target with far loads"),
                        &[half, EVT::OTHER],
                        &[chain, ptr],
                        SDNodeFlags::default(),
                        NodeData::Mem(info),
                    );
                let selector = self
                    .dag
                    .get_copy_from_reg(
                        SDValue { node: les.node, res: 1 },
                        self.dag.config().nodes.far_selector_register.expect("a far selector register"),
                        half,
                    );
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: selector.node, res: 1 });
                (les, selector)
            }
            ISD::Load => self.expand_load(value, half),
            op if Some(op) == self.dag.config().nodes.huge_add && vt == EVT::Simple(MVT::Far) => {
                self.expand_huge_add(value, half)?
            }
            op if Some(op) == self.dag.config().nodes.window && vt == EVT::Simple(MVT::Far) => {
                self.expand_window(value, half)?
            }
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
            ISD::BitCast
                if self.tli.types.type_action(self.dag.value_type(self.dag.operand(value, 0)))
                    == TypeAction::SoftenFloat =>
            {
                // `ExpandRes_BITCAST`, `TypeSoftenFloat`: the halves of the
                // integer that stands for the float (the `getNode(BITCAST)` of
                // each half to its own type is the half).
                self.split_integer(self.softened_float(self.dag.operand(value, 0)), half)
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
                let (lo, hi) =
                    (self.dag.get_load(half, store, slot, info), self.dag.get_load(half, store, high_slot, info));
                // `hasBigEndianPartOrdering`: the word at the slot is the high
                // one
                if self.dag.config().big_endian { (hi, lo) } else { (lo, hi) }
            }
            ISD::BitCast if self.tli.types.is_type_legal(self.dag.value_type(self.dag.operand(value, 0))) => {
                // `ExpandIntRes_BITCAST` from a same-size integer: its low
                // word and the word above it.
                let src = self.dag.operand(value, 0);
                let svt = self.dag.value_type(src);
                let shift = self.shift_const(u128::from(half.size_in_bits()), svt);
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
                let shift = self.shift_const(u128::from(half.size_in_bits()), EVT::I32);
                let high = self.dag.get_node(ISD::Srl, EVT::I32, &[dword, shift]);
                (self.dag.get_node(ISD::Truncate, half, &[dword]), self.dag.get_node(ISD::Truncate, half, &[high]))
            }
            ISD::Freeze => {
                // `SplitRes_FREEZE`: each half frozen.
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                (self.dag.get_node(ISD::Freeze, half, &[lo]), self.dag.get_node(ISD::Freeze, half, &[hi]))
            }
            ISD::AssertSext | ISD::AssertZext => self.expand_assert_ext(value, half),
            ISD::Abs | ISD::AbsMinPoison => self.expand_abs(value, half),
            ISD::CtPop => match self.expand_count_by_libcall(value, ISD::CtPop, half) {
                Some(parts) => parts,
                None => self.expand_ctpop(value, half),
            },
            // `ExpandIntRes_BITREVERSE`: each half reversed, and swapped.
            ISD::BitReverse => {
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                (self.dag.get_node(ISD::BitReverse, half, &[hi]), self.dag.get_node(ISD::BitReverse, half, &[lo]))
            }
            op @ (ISD::CtLz | ISD::CtLzZeroPoison) => match self.expand_count_by_libcall(value, op, half) {
                Some(parts) => parts,
                None => self.expand_ctlz(value, op, half),
            },
            op @ (ISD::CtTz | ISD::CtTzZeroPoison) => match self.expand_count_by_libcall(value, op, half) {
                Some(parts) => parts,
                None => self.expand_cttz(value, op, half),
            },
            op @ (ISD::Add | ISD::Sub) => self.expand_add_sub(value, op, half)?,
            op @ (ISD::UAddO | ISD::USubO) => self.expand_addsub_overflow(value, op, half)?,
            op @ (ISD::UAddOCarry | ISD::USubOCarry) => self.expand_addsub_carry(value, op, half),
            op @ (ISD::SAddO | ISD::SSubO) => self.expand_signed_addsub_overflow(value, op, half),
            op @ (ISD::SAddOCarry | ISD::SSubOCarry) => self.expand_signed_addsub_carry(value, op, half),
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
            op @ (ISD::FpToSInt | ISD::FpToUInt)
                if self.tli.types.type_action(self.dag.value_type(self.dag.operand(value, 0)))
                    == TypeAction::SoftenFloat =>
            {
                // `ExpandIntRes_FP_TO_XINT` over a softened operand: the
                // routine of the original result type, its registers the
                // halves.
                let name = self
                    .fp_to_xint_name(
                        op == ISD::FpToSInt,
                        vt.size_in_bits() > 32,
                        self.dag.value_type(self.dag.operand(value, 0)),
                    )
                    .ok_or(LegalizeError::Unsupported {
                        op,
                        ty: vt,
                        what: "soften a conversion between these types",
                    })?;
                let arg = self.softened_float(self.dag.operand(value, 0));
                let call = self.soft_c_call(name, &[arg], vt, op, vt)?;
                self.split_integer(call, half)
            }
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
            ISD::Truncate => {
                // `ExpandIntRes_TRUNCATE`: the low half truncated from the
                // operand, the high half from the operand shifted down.
                let operand = self.dag.operand(value, 0);
                let operand_vt = self.dag.value_type(operand);
                let lo = self.dag.get_node(ISD::Truncate, half, &[operand]);
                let shift = self.dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
                let down = self.dag.get_node(ISD::Srl, operand_vt, &[operand, shift]);
                (lo, self.dag.get_node(ISD::Truncate, half, &[down]))
            }
            ISD::ExtractElement => {
                // `ExpandRes_EXTRACT_ELEMENT`: the half of the pair the index
                // names, split again into this type's halves.
                let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
                let index = self.dag.operand(value, 1);
                let part = if self.dag.constant(index).is_some_and(|which| which != 0) { hi } else { lo };
                let index_vt = self.dag.value_type(index);
                let (zero, one) = (self.dag.get_constant(0, index_vt), self.dag.get_constant(1, index_vt));
                (
                    self.dag.get_node(ISD::ExtractElement, half, &[part, zero]),
                    self.dag.get_node(ISD::ExtractElement, half, &[part, one]),
                )
            }
            ISD::FShl | ISD::FShr => self.expand_funnel_shift(value, half),
            // `ExpandIntRes_Rotate`: delegate to the funnel shift of the value
            // with itself.
            op @ (ISD::Rotl | ISD::Rotr) => {
                let funnel = if op == ISD::Rotl { ISD::FShl } else { ISD::FShr };
                let x = self.dag.operand(value, 0);
                let by = self.dag.operand(value, 1);
                let whole = self.dag.get_node(funnel, vt, &[x, x, by]);
                self.split_integer(whole, half)
            }
            ISD::VAArg => {
                // `ExpandRes_VAARG`: two reads of the half type, the first
                // with the type's alignment, the second with none; the chain
                // of the second replaces the node's.
                let (chain, list, source) =
                    (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
                let align = self.dag.operand(value, 3);
                let first = self.dag.get_node_multi(ISD::VAArg, &[half, EVT::OTHER], &[chain, list, source, align]);
                let none = self.dag.get_constant_with(0, EVT::I32, true, false);
                let chain = SDValue { node: first.node, res: 1 };
                let second = self.dag.get_node_multi(ISD::VAArg, &[half, EVT::OTHER], &[chain, list, source, none]);
                self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: second.node, res: 1 });
                let (first, second) = (SDValue { node: first.node, res: 0 }, SDValue { node: second.node, res: 0 });
                if self.dag.config().big_endian { (second, first) } else { (first, second) }
            }
            op => return Err(LegalizeError::Unsupported { op, ty: vt, what: "expand result" }),
        };
        self.set_expanded_integer(value, lo, hi);
        Ok(())
    }

    /// `ExpandIntRes_AssertSext`
    /// ([L4048](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4048))
    /// and `ExpandIntRes_AssertZext`
    /// ([L4069](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4069)):
    /// an assertion wider than a half goes to the high half, one inside the low
    /// half asserts the low half and the high is its sign (or zero).
    fn expand_assert_ext(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let code = self.dag.opcode(value);
        let (lo, hi) = self.expanded_integer(self.dag.operand(value, 0));
        let NodeData::ValueType(asserted) = self.dag.node(self.dag.operand(value, 1)).data else {
            unreachable!("an assertion names a value type")
        };
        let (half_bits, asserted_bits) = (half.size_in_bits(), asserted.size_in_bits());
        if half_bits < asserted_bits {
            let rest = self.dag.get_value_type_node(EVT::integer(asserted_bits - half_bits));
            return (lo, self.dag.get_node(code, half, &[hi, rest]));
        }
        let narrow = self.dag.get_value_type_node(asserted);
        let lo = self.dag.get_node(code, half, &[lo, narrow]);
        let hi = if code == ISD::AssertSext {
            // DEVIATE ASSERT-SEXT-HI (see
            // `TargetLowering::assert_sext_keeps_high`)
            if self.tli.assert_sext_keeps_high() && self.dag.opcode(hi) == ISD::CopyFromReg {
                return (lo, hi);
            }
            let sign = self.shift_const(u128::from(half_bits - 1), half);
            self.dag.get_node(ISD::Sra, half, &[lo, sign])
        } else {
            self.dag.get_constant(0, half)
        };
        (lo, hi)
    }

    /// `ExpandIntRes_FunnelShift`: the four halves `y.lo, y.hi, x.lo, x.hi`
    /// (least significant first) and the pair of half-width funnel shifts
    /// that is the result, over the window the amount's half-width bit
    /// picks: set moves it down one half for `FSHL` and up one for `FSHR`.
    fn expand_funnel_shift(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let op = self.dag.opcode(value);
        let (x, y, amount) = (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
        let ((in3, in4), (in1, in2)) = (self.expanded_integer(x), self.expanded_integer(y));
        let amount_vt = self.dag.value_type(amount);
        let bit = self.dag.get_constant(u128::from(half.size_in_bits()), amount_vt);
        let masked = self.dag.get_node(ISD::And, amount_vt, &[amount, bit]);
        let zero = self.dag.get_constant(0, amount_vt);
        let cc_vt = self.tli.types.set_cc_result_type(amount_vt);
        let cond = self.dag.get_setcc(cc_vt, masked, zero, CondCode::SetNe);
        let shift_vt = self.shift_amount_type(half);
        let amount = self.dag.get_any_ext_or_trunc(amount, shift_vt);
        // the amount's bit set moves the window up one half for FSHR and down
        // one for FSHL
        let (lower, upper) =
            if op == ISD::FShr { ([in1, in2, in3], [in2, in3, in4]) } else { ([in2, in3, in4], [in1, in2, in3]) };
        let [s1, s2, s3] = [0, 1, 2].map(|i| self.dag.get_select(half, cond, upper[i], lower[i]));
        let (lo, hi) = (self.dag.get_node(op, half, &[s2, s1, amount]), self.dag.get_node(op, half, &[s3, s2, amount]));
        (lo, hi)
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
            // DEVIATE: the pin shifts by a constant inline. A function tuned
            // for size calls the shift routine where the shifts beyond whole
            // moves cost more than the call (z80 lshl5: 60 B, the call 5).
            // Exit: none; a size policy the pin lacks.
            if self.tli.has_size_libcall_name(op, vt)
                && self.tli.calls_for_size_shift(amt, u128::from(vt.size_in_bits()))
            {
                let amount = self.dag.get_constant(amt, self.libcall_shift_amount_type(half));
                if let Some(pair) = self.expand_by_libcall(value, op, half, op == ISD::Sra, [input, amount]) {
                    return Ok(pair);
                }
            }
            return Ok(self.expand_shift_by_constant(op, input, amt, vt, half));
        }
        let parts = match op {
            ISD::Shl => ISD::ShlParts,
            ISD::Srl => ISD::SrlParts,
            _ => ISD::SraParts,
        };
        let (lo, hi) = self.expanded_integer(input);
        let shift_vt = self.shift_amount_type(half);
        let amount = match self.dag.value_type(amount).size_in_bits().cmp(&shift_vt.size_in_bits()) {
            std::cmp::Ordering::Greater => self.dag.get_node(ISD::Truncate, shift_vt, &[amount]),
            std::cmp::Ordering::Less => self.dag.get_node(ISD::ZeroExtend, shift_vt, &[amount]),
            std::cmp::Ordering::Equal => amount,
        };
        let action = self.tli.operation_action(parts, half);
        let parts_supported =
            action == crate::backend::selectiondag::Action::Custom || self.tli.is_operation_legal(parts, half);
        if !parts_supported {
            // "Otherwise, emit a libcall." (`SHL_I32` and the others); the
            // known-amount-bit forms need no call, so a target with
            // the parts op takes them first, below.
            if let Some(pair) = self.expand_shift_with_known_amount_bit(op, (lo, hi), amount, half) {
                return Ok(pair);
            }
            // the pin passes the amount as the library's `int`
            let amount = self.dag.get_zext_or_trunc(amount, self.libcall_shift_amount_type(half));
            return self
                .expand_by_libcall(value, op, half, op == ISD::Sra, [input, amount])
                .ok_or(LegalizeError::Unsupported { op, ty: vt, what: "expand shift to a library call" });
        }
        if let Some(pair) = self.expand_shift_with_known_amount_bit(op, (lo, hi), amount, half) {
            return Ok(pair);
        }
        let both = self.dag.get_node_multi(parts, &[half, half], &[lo, hi, amount]);
        Ok((both, SDValue { node: both.node, res: 1 }))
    }

    /// `ExpandShiftWithKnownAmountBit`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L3409>),
    /// which the pin runs before the parts node and the library call
    /// (`ExpandIntRes_Shift`, :5063) with no target hook. DEVIATE SD241: a
    /// target that sets `libcall_for_small_variable_shift` (z80, 6502) keeps
    /// the call for the all-clear case until its owner prices an inline
    /// policy; the base did the same for every target without a funnel
    /// shift. Exit: z80 2026-11-24, 6502 2026-12-01: each prices an inline
    /// policy. Owner sdag-b (z80, 6502 consume it). Review 2026-11-24.
    ///
    /// the bits of the amount from `log2(half)` up decide whether it is below
    /// `half`. Any of them set: one half-width shift of the other half. All of
    /// them clear: no select is needed, the bits that cross the halves come
    /// from `(x >> 1) >> (amount ^ (half - 1))`, which is defined at amount 0.
    fn expand_shift_with_known_amount_bit(
        &mut self,
        op: ISD,
        (lo, hi): (SDValue, SDValue),
        amount: SDValue,
        half: EVT,
    ) -> Option<(SDValue, SDValue)> {
        let shift_vt = self.dag.value_type(amount);
        let half_bits = half.size_in_bits();
        let high_mask = super::fold::mask(shift_vt.size_in_bits()) & !u128::from(half_bits - 1);
        let known = super::known_bits::known_bits(self.dag, amount);
        if (known.zero | known.one) & high_mask == 0 {
            return None;
        }
        if known.one & high_mask != 0 {
            let keep = self.dag.get_constant(!high_mask & super::fold::mask(shift_vt.size_in_bits()), shift_vt);
            let rest = self.dag.get_node(ISD::And, shift_vt, &[amount, keep]);
            return Some(match op {
                ISD::Shl => (self.dag.get_constant(0, half), self.dag.get_node(ISD::Shl, half, &[lo, rest])),
                ISD::Srl => (self.dag.get_node(ISD::Srl, half, &[hi, rest]), self.dag.get_constant(0, half)),
                _ => {
                    let sign = self.dag.get_constant(u128::from(half_bits - 1), shift_vt);
                    (self.dag.get_node(ISD::Sra, half, &[hi, rest]), self.dag.get_node(ISD::Sra, half, &[hi, sign]))
                }
            });
        }
        if known.zero & high_mask != high_mask || self.tli.libcall_for_small_variable_shift() {
            return None;
        }
        // 31-x rather than 32-x, which would shift by the width at x = 0; xor
        // is exact because x is below the width
        let last = self.dag.get_constant(u128::from(half_bits - 1), shift_vt);
        let inverse = self.dag.get_node(ISD::Xor, shift_vt, &[amount, last]);
        let (towards, away) = if op == ISD::Shl { (ISD::Shl, ISD::Srl) } else { (ISD::Srl, ISD::Shl) };
        // right shifts mirror the arithmetic of the halves
        let (near, far) = if op == ISD::Shl { (lo, hi) } else { (hi, lo) };
        let one = self.dag.get_constant(1, shift_vt);
        let by_one = self.dag.get_node(away, half, &[near, one]);
        let crossing = self.dag.get_node(away, half, &[by_one, inverse]);
        let near_part = self.dag.get_node(op, half, &[near, amount]);
        let moved = self.dag.get_node(towards, half, &[far, amount]);
        let far_part = self.dag.get_node(ISD::Or, half, &[moved, crossing]);
        Some(if op == ISD::Shl { (near_part, far_part) } else { (far_part, near_part) })
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
        let shift_vt = self.shift_amount_type(half);
        let by = |dag: &mut SelectionDAG, k: u128| dag.get_constant(k, shift_vt);
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

    /// `ExpandIntRes_ABS`: the low half's own abs when the high half is all
    /// sign bits; else the sra+xor+sub sequence on the halves with a borrow
    /// chain; else the negation selected by the sign of the high half.
    fn expand_abs(
        &mut self,
        value: SDValue,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let operand = self.dag.operand(value, 0);
        let (lo, hi) = self.expanded_integer(operand);
        let half_bits = half.size_in_bits();
        let sign_bits = super::known_bits::num_sign_bits(self.dag, operand);
        if sign_bits > half_bits {
            let abs = if sign_bits > half_bits + 1 { ISD::AbsMinPoison } else { ISD::Abs };
            return (self.dag.get_node(abs, half, &[lo]), self.dag.get_constant(0, half));
        }
        // `getTypeToExpandTo(NVT)`
        let mut expands_to = half;
        while !self.tli.types.is_type_legal(expands_to) {
            expands_to = self.tli.types.type_conversion(expands_to).1;
        }
        if self.tli.is_operation_legal_or_custom(ISD::USubOCarry, expands_to) {
            let sign_shift = self.shift_const(u128::from(half_bits - 1), half);
            let sign = self.dag.get_node(ISD::Sra, half, &[hi, sign_shift]);
            let overflow_vt = self.tli.types.set_cc_result_type(half);
            let lo = self.dag.get_node(ISD::Xor, half, &[lo, sign]);
            let hi = self.dag.get_node(ISD::Xor, half, &[hi, sign]);
            let lo = self.dag.get_node_multi(ISD::USubO, &[half, overflow_vt], &[lo, sign]);
            let borrow = SDValue { node: lo.node, res: 1 };
            let hi = self.dag.get_node_multi(ISD::USubOCarry, &[half, overflow_vt], &[hi, sign, borrow]);
            return (lo, hi);
        }
        let vt = self.dag.value_type(value);
        let zero = self.dag.get_constant(0, vt);
        let neg = self.dag.get_node(ISD::Sub, vt, &[zero, operand]);
        let (neg_lo, neg_hi) = self.split_integer(neg, half);
        let cc_vt = self.tli.types.set_cc_result_type(half);
        let zero_half = self.dag.get_constant(0, half);
        let negative = self.dag.get_setcc(cc_vt, hi, zero_half, CondCode::SetLt);
        (self.dag.get_select(half, negative, neg_lo, lo), self.dag.get_select(half, negative, neg_hi, hi))
    }

    /// `ExpandIntRes_ADDSUB`, the carry forms: the low halves with an overflow
    /// result, the high halves with that carry in; failing that, `ADDC` and
    /// `ADDE` (`SUBC`, `SUBE`) joined by glue, where the target has them. The
    /// `UADDO` form (`hasOVF`) is not translated.
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
        if !self.tli.is_operation_legal_or_custom(carry_op, self.tli.types.type_to_expand_to(half)) {
            let (first, second) = if op == ISD::Add { (ISD::AddC, ISD::AddE) } else { (ISD::SubC, ISD::SubE) };
            if self.tli.is_operation_legal_or_custom(first, self.tli.types.type_to_expand_to(half)) {
                let lo = self.dag.get_node_multi(first, &[half, EVT::GLUE], &[ll, rl]);
                let glue = SDValue { node: lo.node, res: 1 };
                let hi = self.dag.get_node_multi(second, &[half, EVT::GLUE], &[lh, rh, glue]);
                return Ok((SDValue { node: lo.node, res: 0 }, SDValue { node: hi.node, res: 0 }));
            }
            // Without a carry operation: the plain halves, the carry (a borrow
            // for a subtract) a compare of the low result with its operand.
            let lo = self.dag.get_node(plain, half, &[ll, rl]);
            let cc_vt = self.tli.types.set_cc_result_type(half);
            let all_ones = super::fold::mask(half.size_in_bits());
            let zero = self.dag.get_constant(0, half);
            let (cmp, minus_one) = if op == ISD::Sub {
                (self.dag.get_setcc(cc_vt, ll, rl, CondCode::SetULt), false)
            } else if self.dag.constant(rl) == Some(1) {
                // X+1 has a carry out if X+1==0
                (self.dag.get_setcc(cc_vt, lo, zero, CondCode::SetEq), false)
            } else if self.dag.constant(rl) == Some(all_ones) {
                // X-1 carries unless X is 0, and X-1 with -1 above is a borrow
                let both = self.dag.constant(rh) == Some(all_ones);
                (self.dag.get_setcc(cc_vt, ll, zero, if both { CondCode::SetEq } else { CondCode::SetNe }), both)
            } else {
                (self.dag.get_setcc(cc_vt, lo, ll, CondCode::SetULt), false)
            };
            let carry = self.dag.get_bool_as_bit(cmp, half);
            if minus_one {
                return Ok((lo, self.dag.get_node(ISD::Sub, half, &[lh, carry])));
            }
            let hi = self.dag.get_node(plain, half, &[lh, rh]);
            return Ok((lo, self.dag.get_node(plain, half, &[hi, carry])));
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
        let (lo, hi, overflow) = if self
            .tli
            .is_operation_legal_or_custom(carry_op, self.tli.types.type_to_expand_to(half))
        {
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

    /// `ExpandIntRes_UADDSUBO_CARRY`: the low halves with the carry in, the
    /// high halves with the low ones' carry out, whose overflow is the
    /// whole one.
    fn expand_addsub_carry(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs, carry) = (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let overflow_vt = self.dag.node(value).values[1];
        let lo = self.dag.get_node_multi(op, &[half, overflow_vt], &[ll, rl, carry]);
        let hi = self.dag.get_node_multi(op, &[half, overflow_vt], &[lh, rh, SDValue { node: lo.node, res: 1 }]);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: hi.node, res: 1 });
        (SDValue { node: lo.node, res: 0 }, SDValue { node: hi.node, res: 0 })
    }

    /// `ExpandIntRes_SADDSUBO_CARRY`: the low halves with the unsigned carry
    /// operation, the high halves with the signed one, which gives the
    /// overflow.
    fn expand_signed_addsub_carry(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs, carry) = (self.dag.operand(value, 0), self.dag.operand(value, 1), self.dag.operand(value, 2));
        let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
        let overflow_vt = self.dag.node(value).values[1];
        let unsigned = if op == ISD::SAddOCarry { ISD::UAddOCarry } else { ISD::USubOCarry };
        let lo = self.dag.get_node_multi(unsigned, &[half, overflow_vt], &[ll, rl, carry]);
        let hi = self.dag.get_node_multi(op, &[half, overflow_vt], &[lh, rh, SDValue { node: lo.node, res: 1 }]);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: hi.node, res: 1 });
        (SDValue { node: lo.node, res: 0 }, SDValue { node: hi.node, res: 0 })
    }

    /// `ExpandIntRes_SADDSUBO`: where the target has the signed carry operation
    /// on the half type, the low halves with `UADDO`/`USUBO` and the high
    /// halves with it, whose flag is the overflow. Without one, the plain
    /// sum or difference, and the overflow as the sign of `(~(l ^ r) & (l ^
    /// sum))` for an add, `((l ^ r) & (l ^ sum))` for a subtract.
    fn expand_signed_addsub_overflow(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> (SDValue, SDValue) {
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let carry_op = if op == ISD::SAddO { ISD::SAddOCarry } else { ISD::SSubOCarry };
        // DEVIATE SD549 (owner x86-m32, exit: a flags copy like
        // `X86FlagsCopyLowering` that tests the saved byte and rewrites the
        // reader's condition, and the selects of one flag joined): the carry
        // chain's flag is in EFLAGS once. A select or branch reading it is
        // a flag reader that needs it restored for each select the legalizer
        // makes of a wide one (two for an i64), a branch each; the sign test
        // is one `test; jns`. A flag stored as a value is read once, by a
        // `setcc`, and takes the carry chain.
        let read_by_branch = self
            .dag
            .users(SDValue { node: value.node, res: 1 })
            .any(
                |user| matches!(
                    self.dag.opcode(SDValue { node: user, res: 0 }),
                    ISD::Select | ISD::SelectCC | ISD::BrCond | ISD::BrCC
                ),
            );
        if !read_by_branch && self.tli.is_operation_legal_or_custom(carry_op, half) {
            let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
            let overflow_vt = self.dag.node(value).values[1];
            let lo_op = if op == ISD::SAddO { ISD::UAddO } else { ISD::USubO };
            let lo = self.dag.get_node_multi(lo_op, &[half, overflow_vt], &[ll, rl]);
            let hi =
                self.dag.get_node_multi(carry_op, &[half, overflow_vt], &[lh, rh, SDValue { node: lo.node, res: 1 }]);
            self.replace_value_with(SDValue { node: value.node, res: 1 }, SDValue { node: hi.node, res: 1 });
            return (SDValue { node: lo.node, res: 0 }, SDValue { node: hi.node, res: 0 });
        }
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
            let amount = self.shift_const(u128::from(scale % half_bits), half);
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
                let amount = self.shift_const(u128::from(scale), half);
                let adjusted = self.dag.get_node(ISD::Srl, half, &[result_hl, amount]);
                let any = self.dag.get_node(ISD::Or, half, &[adjusted, result_hh]);
                test(self, any, zero, CondCode::SetNe)
            } else if scale == half_bits {
                test(self, result_hh, zero, CondCode::SetNe)
            } else {
                // Overflowed if (HH >> (Scale - half)) != 0.
                let amount = self.shift_const(u128::from(scale - half_bits), half);
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
        let sign_shift = self.shift_const(u128::from(bits - 1), wide);
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
                    let sign = self.shift_const(u128::from(half.size_in_bits() - 1), half);
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
        let by = self.shift_const(u128::from(half.size_in_bits()), whole);
        let upper = self.dag.get_node(ISD::Srl, whole, &[value, by]);
        (lo, self.dag.get_node(ISD::Truncate, half, &[upper]))
    }

    /// Whether the product of `lhs` and `rhs` is known to fit `bits`: the
    /// significant bits of the two (`computeKnownBits` leading zeros) add up to
    /// no more.
    fn product_fits(
        &self,
        lhs: SDValue,
        rhs: SDValue,
        bits: u32,
    ) -> bool {
        let active = |value: SDValue| {
            let known = super::known_bits::known_bits(self.dag, value);
            let leading = (0..known.width).rev().take_while(|bit| known.zero >> bit & 1 == 1).count() as u32;
            known.width - leading
        };
        active(lhs) + active(rhs) <= bits
    }

    /// `ExpandIntRes_MUL` through `expandMUL_LOHI`: the full product of the low
    /// halves, plus the two cross products in the high half. Two operands that
    /// are both zero-extended or both sign-extended (by their known bits) need
    /// no cross products. Without a widening multiply the library call
    /// (`RTLIB::MUL_I32`) is made, else `force_expand_multiply`.
    ///
    /// DEVIATE: the call is made only where the target names a routine for
    /// functions tuned for size. The pin calls any named routine; m68k, 6502
    /// and wasm name one and expanded inline until now, and their output stays
    /// as it was until their owners measure the call. Exit: those targets
    /// measured. Nor is it made where the product fits a half: a byte times
    /// a constant is a few instructions, less than the call and the routine.
    /// Outside functions tuned for size a constant factor the combiner makes
    /// shifts and adds stays inline (compare_to_int64 `x * 3`: +8k T called).
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
        // DEVIATE: a target that names a routine for the product of two words
        // (`RTLIB`-style `UMulLoHi` name) calls it for two zero-extended
        // operands without declaring `UMUL_LOHI` legal or custom, which would
        // also turn every division by a constant into a multiply by its
        // reciprocal. Exit: none.
        if super::known_bits::masked_value_is_zero(self.dag, lhs, high_mask)
            && super::known_bits::masked_value_is_zero(self.dag, rhs, high_mask)
            && !self.product_fits(lhs, rhs, bits)
            && ![lhs, rhs].into_iter().filter_map(|one| self.dag.constant(one)).any(|c| self.tli.multiply_by_constant_stays_inline(c))
            && let Some(name) = self.tli.libcall_name(ISD::UMulLoHi, half)
        {
            let vt = self.dag.value_type(value);
            if let Some(hooks) = self.hooks.as_mut()
                && let Some(result) = hooks.make_lib_call(self.dag, name, &[ll, rl], vt, false)
            {
                return Ok(self.split_integer(result, half));
            }
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
        } else if self.tli.has_size_libcall_name(ISD::Mul, self.dag.value_type(value))
            && !self.product_fits(lhs, rhs, bits)
            && ![lhs, rhs]
                .into_iter()
                .filter_map(|one| self.dag.constant(one))
                .any(|c| self.tli.multiply_by_constant_stays_inline(c))
            && let Some(pair) = self.expand_by_libcall(value, ISD::Mul, half, false, [lhs, rhs])
        {
            return Ok(pair);
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
        let shift = self.shift_const(u128::from(half_bits), vt);
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
            let by = self.shift_const(u128::from(lhs_shift), vt);
            lhs = self.dag.get_node(ISD::Shl, vt, &[lhs, by]);
        }
        if rhs_shift != 0 {
            let by = self.shift_const(u128::from(rhs_shift), vt);
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

    /// `ExpandIntRes_CTPOP`'s `LibCall` arm for a count too wide for a
    /// register: the routine the target names for `op` of this type
    /// (`__popcountdi2`), called through `makeLibCall`, its `int` result
    /// widened and split. DEVIATE BC1 (owner softfloat, exit 2026-11-30)
    /// takes the same arm for the zero-poison clz and ctz (the pin splits those
    /// into halves). `None` where the action is not `LibCall`, no routine is
    /// named or the target makes no such call (the pin falls back to the
    /// split too).
    fn expand_count_by_libcall(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
    ) -> Option<(SDValue, SDValue)> {
        if !matches!(op, ISD::CtPop | ISD::CtLzZeroPoison | ISD::CtTzZeroPoison) {
            return None;
        }
        let vt = self.dag.value_type(value);
        if self.tli.operation_action(op, vt) != Action::LibCall {
            return None;
        }
        let name = self.tli.libcall_name(op, vt)?;
        let int = EVT::integer(self.tli.types.int_size());
        let arg = self.dag.operand(value, 0);
        let call = self.hooks.as_mut()?.make_lib_call(self.dag, name, &[arg], int, false)?;
        let widened = self.dag.get_sext_or_trunc(call, vt);
        Some(self.split_integer(widened, half))
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

    /// `ExpandIntRes_SDIV`, `ExpandIntRes_Shift` and the others, last resort:
    /// the routine the target names for `op` of this type
    /// (`RTLIB::getSDIV`, `SHL_I32`), called through `makeLibCall`, its result
    /// split. `None` where the target names none or makes no such call.
    fn expand_by_libcall(
        &mut self,
        value: SDValue,
        op: ISD,
        half: EVT,
        signed: bool,
        args: [SDValue; 2],
    ) -> Option<(SDValue, SDValue)> {
        let vt = self.dag.value_type(value);
        let name = self.tli.libcall_name(op, vt)?;
        let result = self.hooks.as_mut()?.make_lib_call(self.dag, name, &args, vt, signed)?;
        Some(self.split_integer(result, half))
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
        let signed = matches!(op, ISD::SDiv | ISD::SRem);
        let operands = [self.dag.operand(value, 0), self.dag.operand(value, 1)];
        if let Some(halves) = self.expand_by_libcall(value, op, half, signed, operands) {
            return Ok(halves);
        }
        let Some(arch) = self.target else {
            return Err(LegalizeError::Unsupported { op, ty: vt, what: "expand a division without a target" });
        };
        let wants_quotient = matches!(op, ISD::SDiv | ISD::UDiv);
        let (lhs, rhs) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let ((dividend_lo, dividend_hi), (divisor_lo, divisor_hi)) =
            (self.expanded_integer(lhs), self.expanded_integer(rhs));
        // DEVIATE DIV64-LIBCALL (owner x86-m32, exit 2026-11-20): where the
        // platform's runtime lists the routine (libgcc's `__divdi3` family:
        // the driver names it for hosted objects), the pin's call, not the
        // inline routine; its bytes are the library's.
        if let Some(name) = self.tli.libcall_name(op, vt).filter(|name| self.tli.libcall_convention(name).is_some()) {
            let env = super::libcall::CallEnv {
                arch,
                pointer: self.dag.config().pointer,
                call: super::builder::LoweringNodes::installed().call.expect("a target with a call node"),
            };
            let parts = super::libcall::c_lib_call_parts(
                self.dag,
                &env,
                &self.tli.libcall_symbol(name),
                self.tli.libcall_convention(name),
                &[dividend_lo, dividend_hi, divisor_lo, divisor_hi],
                vt,
            );
            if let Some([low, high]) = parts.as_deref() {
                return Ok((*low, *high));
            }
        }
        let wide = self.dag.constant(divisor_hi) != Some(0);
        // DEVIATE DIVREM-HELPER (owner m32, exit 2026-11-27; the pin's
        // `useDivRem` for a target whose routine makes both, e.g. ARM's
        // `__aeabi_uldivmod`; the pin's x86 has none for i64): the quotient and
        // the remainder of the same operands are one call of the routine, which
        // returns both.
        let sibling = match op {
            ISD::UDiv => ISD::URem,
            ISD::URem => ISD::UDiv,
            ISD::SDiv => ISD::SRem,
            _ => ISD::SDiv,
        };
        let sibling = self.dag.find_node(sibling, vt, &operands).filter(|other| self.dag.has_users(*other));
        let used = match (sibling, wants_quotient) {
            (Some(_), _) => llrm_target::routines::Used::Both,
            (None, true) => llrm_target::routines::Used::Quotient,
            (None, false) => llrm_target::routines::Used::Remainder,
        };
        // A dword divisor's size form always does two `div`s: -Os only, -O1
        // keeps the clocks.
        // DEVIATE DIV-HOT (owner x86-m32, exit 2026-11-24): in a block that runs more than once a
        // call the speed routine, whose bytes the module pays once.
        let hot = self.tli.fast_divide_in_loops() && self.dag.config().block_frequency > 1.0;
        let small = (self.tli.small_divide() && (wide || self.tli.size_only()) && !hot)
            .then(|| arch.divide_routine_small(signed, wide, used).ok())
            .flatten();
        let Some(routine) = small.or_else(|| arch.divide_routine_used(signed, wide, used).ok()) else {
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
        let env = CallEnv {
            arch,
            pointer,
            call: super::builder::LoweringNodes::installed().call.expect("a target with a call node"),
        };
        let (_chain, results) = lib_call(
            self.dag,
            &env,
            root,
            callee,
            &args,
            0,
            0,
            &returns,
            &super::dagisel_adapter::call_clobbered_flags(),
        );
        let (quotient, remainder) = ((results[0], results[1]), (results[2], results[3]));
        if let Some(other) = sibling {
            let (lo, hi) = if wants_quotient { remainder } else { quotient };
            self.set_expanded_integer(other, lo, hi);
        }
        Ok(if wants_quotient { quotient } else { remainder })
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
        // a `MERGE_VALUES` is the pin's list of replacement values
        let merged = self.dag.opcode(result) == ISD::MergeValues && self.dag.node(result).ops.len() == results;
        for res in 0..results as u8 {
            let new = if merged {
                self.dag.operand(result, res as usize)
            } else if results == 1 {
                result
            } else {
                SDValue { node: result.node, res }
            };
            self.replace_value_with(SDValue { node: n, res }, new);
        }
        true
    }

    /// `ExpandIntRes_LOAD`, a plain load: the low dword at the address, the
    /// high at +4 (little endian; the other way round for big endian), both on
    /// the load's chain, the chains joined.
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
                    let sign = self.shift_const(u128::from(half.size_in_bits() - 1), half);
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
        let (lo, hi) = (self.dag.get_load(half, chain, ptr, info), self.dag.get_load(half, chain, high_ptr, info));
        let chains = self.dag.get_token_factor(&[SDValue { node: lo.node, res: 1 }, SDValue { node: hi.node, res: 1 }]);
        self.replace_value_with(SDValue { node: value.node, res: 1 }, chains);
        // big endian: the high dword is the one at the address
        if self.dag.config().big_endian { (hi, lo) } else { (lo, hi) }
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
            || users.any(|user| {
                self.dag.opcode(SDValue { node: user, res: 0 }) != self.dag.config().nodes.ptrdiff.unwrap_or(ISD::Undef)
            })
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
            return self
                .dag
                .get_node_flags(ISD::BuildPair, pointer, &[moved, selector], super::dag::SDNodeFlags::PAIR_FAR);
        }
        // `getObjectPtrOffset`: the offset stays inside the object, so the add
        // cannot wrap.
        self.dag.get_object_ptr_offset(ptr, step as i64)
    }

    /// `ExpandIntOp_STORE`, a plain store: the low dword, then the high at +4
    /// (the other way round for big endian).
    fn expand_store(
        &mut self,
        store: SDValue,
    ) -> SDValue {
        let (chain, value, ptr) = (self.dag.operand(store, 0), self.dag.operand(store, 1), self.dag.operand(store, 2));
        let NodeData::Mem(info) = self.dag.node(store).data else { unreachable!("a store has a memory operand") };
        let (lo, hi) = self.expanded_integer(value);
        let (lo, hi) = if self.dag.config().big_endian { (hi, lo) } else { (lo, hi) };
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
                    op: self.dag.config().nodes.huge_add.expect("huge pointer add"),
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
        let (width, stride_shift) =
            (self.shift_const(u128::from(half.size_in_bits()), dword), self.shift_const(u128::from(shift), dword));
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
                    op: self.dag.config().nodes.window.expect("pointer window"),
                    ty: self.dag.value_type(node),
                    what: "expand result: no selector stride",
                },
            )?;
        let below = half.size_in_bits() - shift;
        let (offset, selector) = self.expanded_integer(self.dag.operand(node, 0));
        let by = self.shift_const(u128::from(below), half);
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
                    op: self.dag.config().nodes.ptrdiff.expect("pointer difference"),
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
        let (by_stride, by_offset) =
            (self.shift_const(u128::from(shift), vt), self.shift_const(u128::from(half.size_in_bits()), vt));
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
        let selector_register = self.dag.config().nodes.far_selector_register.expect("a far selector register");
        let chain = self.dag.get_copy_to_reg(self.dag.operand(access, 0), selector_register, selector);
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
        let far_call = self.dag.get_node_multi(self.dag.config().nodes.far_call.expect("a far call"), &values, &ops);
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
        // A constant operand leaves one operand's bytes live beside the
        // accumulator; two registers' worth spilled in the corpus.
        if matches!(code, CondCode::SetEq | CondCode::SetNe)
            && self.tli.byte_equality()
            && self.dag.constant(self.dag.operand(setcc, 1)).is_some()
        {
            // DEVIATE WIDE-EQ-BYTES (see `TargetLowering::byte_equality`)
            let (left, right) = (self.dag.operand(setcc, 0), self.dag.operand(setcc, 1));
            let wide = self.dag.value_type(left);
            let byte = |this: &mut Self, value: SDValue, at: u32| {
                let shifted = if at == 0 {
                    value
                } else {
                    let by = this.dag.get_constant(u128::from(8 * at), EVT::I8);
                    this.dag.get_node(ISD::Srl, wide, &[value, by])
                };
                this.dag.get_node(ISD::Truncate, EVT::I8, &[shifted])
            };
            let mut seen: Option<SDValue> = None;
            for at in 0..wide.size_in_bits() / 8 {
                let (a, b) = (byte(self, left, at), byte(self, right, at));
                let diff = self.dag.get_node(ISD::Xor, EVT::I8, &[a, b]);
                seen = Some(match seen {
                    Some(so_far) => self.dag.get_node(ISD::Or, EVT::I8, &[diff, so_far]),
                    None => diff,
                });
            }
            let zero = self.dag.get_constant(0, EVT::I8);
            return self.dag.get_setcc(vt, seen.expect("a byte"), zero, code);
        }
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
        // A comparison of the sign bit looks at the top part.
        let sign_test = match self.dag.constant(self.dag.operand(setcc, 1)) {
            Some(c) => {
                let width = self.dag.value_type(self.dag.operand(setcc, 1)).size_in_bits();
                (code == CondCode::SetLt && c == 0) || (code == CondCode::SetGt && c == super::fold::mask(width))
            }
            None => false,
        };
        if sign_test {
            return self.dag.get_setcc(vt, lh, rh, code);
        }
        // `SimplifySetCC` of each half: against an extreme value it folds.
        let half_compare = |this: &mut Self, a: SDValue, b: SDValue, code: CondCode| {
            let folded = this
                .dag
                .constant(b)
                .and_then(|c| super::setcc_extreme::fold_against_an_extreme(this.dag, vt, a, c, code));
            folded.unwrap_or_else(|| this.dag.get_setcc(vt, a, b, code))
        };
        let low = half_compare(self, ll, rl, low_code);
        let high = half_compare(self, lh, rh, code);
        // For LE / GE, a high part known false ignores the low part. For LT /
        // GT, a low part known false is the high part, and so is a high part
        // known true.
        let eq_allowed = matches!(
            code,
            CondCode::SetLe | CondCode::SetULe | CondCode::SetGe | CondCode::SetUGe
        );
        let (low_known, high_known) = (self.dag.constant(low), self.dag.constant(high));
        if (eq_allowed && high_known == Some(0)) || (!eq_allowed && (high_known == Some(1) || low_known == Some(0))) {
            return high;
        }
        // Comparing the low bits is enough.
        if lh == rh {
            return low;
        }
        // Lower with SETCCCARRY if the target supports it: the high halves
        // subtracted with the low halves' borrow; `>` and `<=` flip the
        // operands.
        // DEVIATE SC1 (owner llrm-target-6502, exit 2026-12-05): a target that
        // marks the node Custom on a half that is itself expanded gets the
        // borrow chain across every piece (`ExpandIntOp_SETCCCARRY`
        // below); the pin asks for a legal half, so a 64-bit compare on a
        // byte machine was a select of two compares of 32-bit halves.
        let half_vt = self.dag.value_type(lh);
        let chain_by_expansion = !self.tli.types.is_type_legal(half_vt)
            && self.tli.operation_action(ISD::SetCCCarry, half_vt) == Action::Custom;
        if chain_by_expansion || self.tli.is_operation_legal_or_custom(ISD::SetCCCarry, half_vt) {
            let original = code;
            let (code, flip) = match code {
                CondCode::SetGt => (CondCode::SetLt, true),
                CondCode::SetUGt => (CondCode::SetULt, true),
                CondCode::SetLe => (CondCode::SetGe, true),
                CondCode::SetULe => (CondCode::SetUGe, true),
                other => (other, false),
            };
            // DEVIATE CC1 (owner greedy-subreg, exit 2026-11-27): against a
            // constant, `x > c` is `x >= c + 1` and `x <= c` is `x < c + 1`,
            // so the operands stay in place and the constant an immediate
            // (`cmp lo, c.lo; sbb hi, c.hi`); the pin swaps them and
            // materializes the constant in registers. The pin's own form is the
            // IR canonical `ult` of `getFlippedStrictnessPredicate`.
            let wide = self.dag.value_type(self.dag.operand(setcc, 1));
            let moved = if flip {
                let bits = wide.size_in_bits();
                let top = super::fold::mask(bits);
                let max = if matches!(original, CondCode::SetUGt | CondCode::SetULe) { top } else { top >> 1 };
                self.dag.constant(self.dag.operand(setcc, 1)).filter(|&c| c != max)
            } else {
                None
            };
            if let Some(constant) = moved {
                let half_bits = half.size_in_bits();
                let next = constant.wrapping_add(1) & super::fold::mask(wide.size_in_bits());
                let (rl, rh) = (
                    self.dag.get_constant(next & super::fold::mask(half_bits), half),
                    self.dag.get_constant(next >> half_bits, half),
                );
                let overflow_vt = self.tli.types.set_cc_result_type(half);
                let borrow = self.dag.get_node_multi(ISD::USubO, &[half, overflow_vt], &[ll, rl]);
                let borrow = SDValue { node: borrow.node, res: 1 };
                let moved_code = match original {
                    CondCode::SetUGt => CondCode::SetUGe,
                    CondCode::SetULe => CondCode::SetULt,
                    CondCode::SetGt => CondCode::SetGe,
                    _ => CondCode::SetLt,
                };
                let cc = self.dag.get_cond_code(moved_code);
                return self.dag.get_node(ISD::SetCCCarry, vt, &[lh, rh, borrow, cc]);
            }
            let ((ll, rl), (lh, rh)) = if flip { ((rl, ll), (rh, lh)) } else { ((ll, rl), (lh, rh)) };
            let overflow_vt = self.tli.types.set_cc_result_type(half);
            let borrow = self.dag.get_node_multi(ISD::USubO, &[half, overflow_vt], &[ll, rl]);
            let borrow = SDValue { node: borrow.node, res: 1 };
            let cc = self.dag.get_cond_code(code);
            return self.dag.get_node(ISD::SetCCCarry, vt, &[lh, rh, borrow, cc]);
        }
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
                let dword =
                    self.dag.get_node_flags(ISD::BuildPair, EVT::I32, &[lo, hi], super::dag::SDNodeFlags::PAIR_FAR);
                let mut ops = self.dag.node(node).ops.clone();
                ops[2] = dword;
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
            ISD::Load if at == 1 => self.expand_far_address(node, at),
            op if at == 1
                && (Some(op) == self.dag.config().nodes.far_load_es
                    || Some(op) == self.dag.config().nodes.far_load_ds) =>
            {
                self.expand_far_address(node, at)
            }
            ISD::Store if at == 2 => self.expand_far_address(node, at),
            op if at == 1 && Some(op) == self.dag.config().nodes.call => self.expand_far_call(node),
            op if Some(op) == self.dag.config().nodes.ptrdiff => self.expand_ptrdiff(node)?,
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
                let vt = self.dag.value_type(node);
                let kind =
                    if vt == EVT::Simple(MVT::Far) { super::dag::SDNodeFlags::PAIR_FAR } else { Default::default() };
                self.dag.get_node_flags(ISD::BuildPair, vt, &[lo, hi], kind)
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
            ISD::SetCCCarry if at < 2 => {
                // `ExpandIntOp_SETCCCARRY`: the low halves' subtract takes the
                // carry in, and the high halves compare with its borrow.
                let (lhs, rhs, carry, cc) = (
                    self.dag.operand(node, 0),
                    self.dag.operand(node, 1),
                    self.dag.operand(node, 2),
                    self.dag.operand(node, 3),
                );
                let ((ll, lh), (rl, rh)) = (self.expanded_integer(lhs), self.expanded_integer(rhs));
                let half = self.dag.value_type(ll);
                let carry_vt = self.dag.value_type(carry);
                let low = self.dag.get_node_multi(ISD::USubOCarry, &[half, carry_vt], &[ll, rl, carry]);
                let vt = self.dag.value_type(node);
                self.dag.get_node(ISD::SetCCCarry, vt, &[lh, rh, SDValue { node: low.node, res: 1 }, cc])
            }
            ISD::SelectCC if at < 2 && self.setcc_code(node).is_some() => {
                // `ExpandIntOp_SELECT_CC`: the compare expanded to a boolean,
                // and the select on that boolean against zero.
                let code = self.setcc_code(node).expect("checked");
                let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let bool_vt = self.tli.types.set_cc_result_type(self.dag.value_type(lhs));
                let compare = self.dag.get_setcc(bool_vt, lhs, rhs, code);
                let flag = self.expand_setcc(compare, code);
                let zero = self.dag.get_constant(0, self.dag.value_type(flag));
                let not_equal = self.dag.get_cond_code(CondCode::SetNe);
                let vt = self.dag.value_type(node);
                let (tval, fval) = (self.dag.operand(node, 2), self.dag.operand(node, 3));
                self.dag.get_node(ISD::SelectCC, vt, &[flag, zero, tval, fval, not_equal])
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
            // The scale of a fixed-point multiply is an `i32`
            // constant whatever the operands' width. `PromoteIntOp_FIX` zero
            // extends it; where `i32` is expanded the low half carries it.
            // DEVIATE SD602: the pin has no expand case, so a 16-bit target
            // dies on `llvm.smul.fix.i16`. Exit: none, the pin's gap. Owner
            // z80. Review 2026-11-27.
            ISD::SMulFix | ISD::UMulFix | ISD::SMulFixSat | ISD::UMulFixSat if at == 2 => {
                let (scale, _) = self.expanded_integer(self.dag.operand(node, 2));
                let ops = [self.dag.operand(node, 0), self.dag.operand(node, 1), scale];
                let updated = self.dag.update_node_operands(n, &ops);
                SDValue { node: updated, res: 0 }
            }
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

/// A memory type that is not a power of two wide, as the powers of two it is
/// the sum of, widest first (`i48`: `i32` and `i16`; `i56`: `i32`, `i16` and
/// `i8`); none if it is a power of two or has a bit left over a byte.
pub(super) fn odd_memory(mem: EVT) -> Option<Vec<EVT>> {
    let bits = mem.size_in_bits();
    if bits.is_power_of_two() || bits % 8 != 0 {
        return None;
    }
    Some((3..32).rev().map(|bit| 1u32 << bit).filter(|part| bits & part != 0).map(EVT::integer).collect())
}
