//! The DAG combiner: peephole rewrites of the DAG to a fixed point (the pin's
//! `DAGCombiner`).
//!
//! # How it works
//! A worklist holds the nodes to look at, newest last. Each pass of
//! [`DagCombiner::run`] takes the last, skips it if nothing reads it, puts
//! its operands back on the worklist, and calls [`DagCombiner::combine`]:
//! the generic `visit` for its opcode, else the target's
//! [`CombineHooks::perform_dag_combine`]. A result other than the node
//! replaces it in every user, and the result and its users go back on the
//! worklist. Nodes made on the way are put on the worklist too. The walk ends
//! when the worklist is empty.
//!
//! # Runs
//! At four levels (`CombineLevel`): before and after type legalization, after
//! vector legalization, and after DAG legalization. Not wired in yet. Only
//! `visitMUL`'s first half is translated here; the other visitors are added
//! in the order the corpus counts them.
//!
//! DEVIATE SD201: no `HandleSDNode` for the root, no `DAGUpdateListener`: the
//! arena keeps replaced nodes, `run` reads [`Rauw`] events and the node count
//! to find what changed. Exit: none. Owner sdag-b. Review 2026-11-13.
//!
//! DEVIATE SD202: no `computeKnownBits`/`MaskedValueIsZero` (SD12): a fold that
//! asks for known bits sees only constants. Exit: a known-bits port. Owner
//! sdag-b. Review 2026-11-13.
//!
//! DEVIATE SD205: `visitSIGN_EXTEND`/`ZERO_EXTEND`/`ANY_EXTEND` fold a
//! simple, non-extending load before operations are legal, with the table
//! given by `with_tli` (no table: only a load nothing else reads); every
//! integer truncate is taken as free (`isTruncateFree`). Exit: the target's
//! truncate query. Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::Run`
//! ([L1845](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1845)),
//! `combine`
//! ([L2157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2157)),
//! `visitMUL`
//! ([L4908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4908)).

use super::actions::{Action, LoadExt, TargetLowering};
use super::dag::{MemInfo, NodeData, Rauw, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::mvt::{EVT, MVT};

/// `CombineLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombineLevel {
    BeforeLegalizeTypes,
    AfterLegalizeTypes,
    AfterLegalizeVectorOps,
    AfterLegalizeDag,
}

/// `TLI.hasTargetDAGCombine` and `TLI.PerformDAGCombine`.
pub trait CombineHooks {
    /// `TLI.convertSelectOfConstantsToMath`: a select of two constants is an
    /// extend of the condition plus a constant. The pin's default is false;
    /// DEVIATE: true here because this combiner was written for x86, which
    /// asks for it. Exit: x86 returns true and the default is the pin's.
    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        true
    }

    /// `visitXOR`'s `(xor (and x, y), y)` is `(and (not x), y)`. DEVIATE: off
    /// by default so x86's rows stay as they are until the judge has seen
    /// them. Exit: x86 judged.
    fn fold_xor_of_and_with_mask(&self) -> bool {
        false
    }

    /// `visitSIGN_EXTEND_INREG`'s `(sext_inreg (extload x))` is `(sextload x)`.
    /// DEVIATE: off by default, as above. Exit: x86 judged.
    fn fold_sext_in_reg_of_load(&self) -> bool {
        false
    }

    /// `visitFREEZE`'s `freeze(op(x, ...))` is `op(freeze(x), ...)`, so a
    /// frozen compare tree is a compare tree of frozen leaves. DEVIATE: off by
    /// default, as above; and only for an integer `setcc` or bitwise/add
    /// node, with no `ReplaceAllUsesOfValueWith` of the frozen operand.
    /// Exit: x86 judged.
    fn push_freeze_through_operands(&self) -> bool {
        false
    }

    /// `tryStoreMergeOfConstants`' "check whether a truncstore is legal": a run
    /// of narrow constant stores whose joined type is illegal is one
    /// truncating store of the promoted type (AArch64: two `strb` are a
    /// `strh`). DEVIATE: off by default, as above. Exit: x86 judged.
    fn merge_stores_via_trunc_store(&self) -> bool {
        false
    }

    /// `ReduceLoadWidth` looks through a `freeze` between an `and` mask and its
    /// load, and puts the freeze back on the narrowed load. Default false
    /// until x86 is judged.
    fn reduce_load_width_through_freeze(&self) -> bool {
        false
    }

    /// `visitMUL`'s `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)`.
    /// Default false until x86 is judged.
    fn fold_mul_of_add_with_constant(&self) -> bool {
        false
    }

    /// `TargetLowering::isMulAddWithConstProfitable` for a single-use add, on
    /// the constants `c1` and `c2` of the type `vt`.
    fn is_mul_add_with_const_profitable(
        &self,
        _vt: EVT,
        _c1: u128,
        _c2: u128,
    ) -> bool {
        true
    }

    /// `visitSRA`'s `(sra (trunc (srl x, c1)), c2)` is `(trunc (sra x, c1 +
    /// c2))` when the truncate removes `c1` bits. Default false until x86
    /// is judged.
    fn fold_sra_of_trunc_of_shift(&self) -> bool {
        false
    }

    /// `TargetLowering::shouldTransformSignedTruncationCheck`: `(add x,
    /// 1<<(k-1)) u< 1<<k` is `sext_inreg x, k == x`. The pin's default is
    /// false. Default false until x86 is judged.
    fn should_transform_signed_truncation_check(
        &self,
        _vt: EVT,
        _kept_bits: u32,
    ) -> bool {
        false
    }

    /// `visitORCommutative`'s absorption folds, `(or (and x, y), x)` is `x`.
    /// Default false until x86 is judged.
    fn fold_or_absorption(&self) -> bool {
        false
    }

    /// `convertSetCCLogicToBitwiseLogic`: `and (seteq a, b), (seteq c, d)` as
    /// one compare of `or (xor a, b), (xor c, d)`. Default true, as X86's
    /// (the pin's default is false).
    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        true
    }

    /// `getShiftAmountTy` for a shift of `shifted`: x86's is `i8`. DEVIATE:
    /// the pin asks the target; AArch64 shifts by a value of the shifted type.
    /// Exit: x86 judged.
    fn shift_amount_type(
        &self,
        _shifted: EVT,
    ) -> EVT {
        EVT::I8
    }

    /// `replaceStoreOfFPConstant` stores a `double` constant as one `i64` when
    /// that type is legal. Default false until x86 is judged.
    fn store_fp_constant_as_i64(&self) -> bool {
        false
    }

    /// DEVIATE SD446: `store (fp_to_sint x), ptr` is one `fist` into `ptr`
    /// (`X86ISD::FP_TO_INT_IN_MEM`) when this returns true for the float and
    /// the integer type; the pin converts through a frame temporary. Exit: the
    /// pin has no such fold (it matches main's output).
    fn store_fp_to_int_in_memory(
        &self,
        _float: EVT,
        _int: EVT,
    ) -> bool {
        false
    }

    /// `reassociateOpsCommutative`'s pairing of `and`/`or` operands that are
    /// compares of one predicate. DEVIATE: off by default (the pin runs it
    /// for every target). Exit: x86 judged.
    fn reassociate_compare_pairs(&self) -> bool {
        false
    }

    /// `visitSELECT`'s `select (setcc x, y, cc), a, b` to `select_cc x, y, a,
    /// b, cc`, which the pin makes before legalizing when `SELECT_CC` is
    /// legal or custom for `vt`. x86 has no `SELECT_CC`.
    fn select_to_select_cc(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }

    /// `SimplifyDemandedBits`'s `ADD`/`SUB`/`MUL` case. DEVIATE: off by default
    /// (the pin runs it for every target). Exit: x86 judged.
    fn simplify_demanded_add_sub_mul(&self) -> bool {
        false
    }

    /// `isIndexedLoadLegal`/`isIndexedStoreLegal` of `PRE_INC`/`POST_INC` (the
    /// only modes made) for an access of `vt`. x86 has no indexed forms.
    fn is_indexed_legal(
        &self,
        _is_load: bool,
        _vt: EVT,
        _post: bool,
    ) -> bool {
        false
    }

    /// `getPreIndexedAddressParts`/`getPostIndexedAddressParts`: the base and
    /// the signed offset of `access` (a load or store) when `address` (an
    /// add or subtract) can be its update; the offset is made a constant of
    /// the type of the address's second operand.
    fn indexed_address_parts(
        &self,
        _dag: &super::dag::SelectionDAG,
        _access: SDValue,
        _address: SDValue,
        _post: bool,
    ) -> Option<(SDValue, i64)> {
        None
    }

    /// `isLegalAddressingMode` of `[reg + base_offs]` (scale 0) or `[reg +
    /// reg]` (scale 1) for an access of `vt`.
    fn is_legal_addressing_mode(
        &self,
        _vt: EVT,
        _base_offs: i64,
        _scale: i64,
    ) -> bool {
        false
    }

    /// `BuildSDIV`/`BuildUDIV`'s "a legal multiply of the wide type" for any
    /// width, not only `i16` and below. DEVIATE: off by default, as above.
    /// Exit: x86 judged.
    fn widen_multiply_high_any_width(&self) -> bool {
        false
    }

    /// `SimplifySetCC`'s "Canonicalize GE/LE comparisons to use GT/LT" (`x >=
    /// c` is `x > c - 1`). DEVIATE: off by default. x86's rows are the old
    /// selector's, which keeps a range check as `cmp n; jae`; the rewrite
    /// turns it into `cmp n - 1; ja` and moved 35 of its 424 judged rows
    /// (`switch` in every language). Exit: re-bless the x86 rows.
    fn canonicalize_ge_le(&self) -> bool {
        false
    }

    fn has_target_combine(
        &self,
        _op: ISD,
    ) -> bool {
        false
    }

    /// The replacement for `node`, or `None` for no change.
    fn perform_dag_combine(
        &mut self,
        _dag: &mut SelectionDAG,
        _node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        None
    }

    /// `targetShrinkDemandedConstant`: `None` is "not handled" (the generic
    /// shrink runs); `Some(None)` keeps the node as it is; `Some(Some(new))`
    /// replaces it.
    fn target_shrink_demanded_constant(
        &mut self,
        _dag: &mut SelectionDAG,
        _op: SDValue,
        _demanded: u128,
    ) -> Option<Option<SDValue>> {
        None
    }

    /// `IsDesirableToPromoteOp`, with `isTypeDesirableForOp` before it: the
    /// type a legal operation is better done in, or `None`.
    fn promote_type(
        &self,
        _dag: &SelectionDAG,
        _tli: &TargetLowering,
        _op: SDValue,
    ) -> Option<EVT> {
        None
    }
}

/// A target with no combines.
pub struct NoCombines;

impl CombineHooks for NoCombines {}

/// A set of node ids, one flag each (ids are dense).
#[derive(Default)]
struct NodeSet(Vec<bool>);

impl NodeSet {
    /// Whether `node` was not in the set.
    fn insert(
        &mut self,
        node: u32,
    ) -> bool {
        let at = node as usize;
        if self.0.len() <= at {
            self.0.resize(at + 1, false);
        }
        !std::mem::replace(&mut self.0[at], true)
    }

    fn remove(
        &mut self,
        node: u32,
    ) {
        if let Some(flag) = self.0.get_mut(node as usize) {
            *flag = false;
        }
    }

    fn contains(
        &self,
        node: u32,
    ) -> bool {
        self.0.get(node as usize).copied().unwrap_or(false)
    }
}

pub struct DagCombiner<'a, H: CombineHooks> {
    pub(super) dag: &'a mut SelectionDAG,
    pub(super) hooks: &'a mut H,
    pub(super) level: CombineLevel,
    worklist: Vec<u32>,
    queued: NodeSet,
    /// `CombinedNodes`: the nodes already visited, whose operands are not
    /// queued again by a visit of their user.
    combined: NodeSet,
    /// The action table, for the folds that ask what the target can do.
    pub(super) tli: Option<&'a TargetLowering>,
    /// `OptLevel == CodeGenOptLevel::None`: the chain, store and load combines
    /// the pin skips at `-O0` are skipped.
    pub(super) opt_none: bool,
}

fn op_is_srl(op: ISD) -> bool {
    op == ISD::Srl
}

impl<'a, H: CombineHooks> DagCombiner<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        hooks: &'a mut H,
        level: CombineLevel,
    ) -> DagCombiner<'a, H> {
        DagCombiner {
            dag,
            hooks,
            level,
            worklist: Vec::new(),
            queued: NodeSet::default(),
            combined: NodeSet::default(),
            tli: None,
            opt_none: false,
        }
    }

    /// The action table the folds query (`TLI`).
    pub fn with_tli(
        mut self,
        tli: &'a TargetLowering,
    ) -> DagCombiner<'a, H> {
        self.tli = Some(tli);
        self
    }

    /// The pin's `-O0` combiner (`OptLevel == None`).
    pub fn with_opt_none(mut self) -> DagCombiner<'a, H> {
        self.opt_none = true;
        self
    }

    pub(super) fn add_to_worklist(
        &mut self,
        node: u32,
    ) {
        if self.queued.insert(node) {
            self.worklist.push(node);
        }
    }

    pub(super) fn add_to_worklist_with_users(
        &mut self,
        node: u32,
    ) {
        let results = self.dag.node(SDValue { node, res: 0 }).values.len() as u8;
        for res in 0..results {
            let users: Vec<u32> = self.dag.users(SDValue { node, res }).collect();
            for user in users {
                self.add_to_worklist(user);
            }
        }
        self.add_to_worklist(node);
    }

    fn next(&mut self) -> Option<u32> {
        let node = self.worklist.pop()?;
        self.queued.remove(node);
        Some(node)
    }

    /// A node nothing reads that is not the root
    /// (`recursivelyDeleteUnusedNodes`).
    fn is_dead(
        &self,
        node: u32,
    ) -> bool {
        if node == self.dag.get_root().node {
            return false;
        }
        let results = self.dag.node(SDValue { node, res: 0 }).values.len() as u8;
        !(0..results).any(|res| self.dag.has_users(SDValue { node, res }))
    }

    /// `Run`.
    pub fn run(&mut self) {
        // `RemoveDeadNodes`: what an earlier phase replaced no longer counts as
        // a user. Newest first, so a user goes before the nodes it reads.
        for node in self.dag.node_ids().rev() {
            self.dag.delete_dead(node);
        }
        // Oldest first, so the newest is taken first.
        for node in self.dag.node_ids() {
            self.add_to_worklist(node);
        }
        while let Some(n) = self.next() {
            if self.is_dead(n) {
                continue;
            }
            // "Add any operands of the new node which have not yet been
            // combined to the worklist as well" (`CombinedNodes`): without the
            // test a DAG whose nodes share operands is walked once per path.
            let ops: Vec<u32> = self.dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|op| op.node).collect();
            for op in ops {
                if !self.combined.contains(op) {
                    self.add_to_worklist(op);
                }
            }
            self.combined.insert(n);
            // `SDLoc(N)`: what the combine makes takes the node's order.
            self.dag.cur_order = self.dag.node(SDValue { node: n, res: 0 }).order;
            let before = self.dag.node_count();
            let result = self.combine(SDValue { node: n, res: 0 });
            for (old, new) in self.dag.take_replacements() {
                self.replace_value(old, new);
                self.add_to_worklist_with_users(new.node);
                self.dag.delete_dead(old.node);
            }
            for made in before..self.dag.node_count() {
                self.add_to_worklist(made as u32);
            }
            let Some(result) = result else { continue };
            if result.node == n && result.res == 0 {
                continue;
            }
            let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
            let replacement = self.dag.node(result).values.len();
            let count = if results == replacement { results } else { 1 };
            for res in 0..count as u8 {
                let new = if results == replacement { SDValue { node: result.node, res } } else { result };
                for event in self.dag.replace_all_uses_with(SDValue { node: n, res }, new) {
                    match event {
                        Rauw::Updated(user) => self.add_to_worklist(user),
                        Rauw::Merged { into, .. } => self.add_to_worklist_with_users(into),
                    }
                }
            }
            if self.dag.opcode(result) != ISD::EntryToken {
                self.add_to_worklist_with_users(result.node);
            }
            self.dag.delete_dead(n);
            self.dag.delete_replaced(n);
        }
        // `RemoveDeadNodes` before selection: a node a fold left behind must
        // not count as a user of what it read (a load folds into its one user).
        for node in self.dag.node_ids().rev() {
            self.dag.delete_dead(node);
        }
    }

    /// `combine`: the generic fold, else the target's.
    fn combine(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let mut result = self.visit(node);
        if result.is_none() && (matches!(op, ISD::Target(_)) || self.hooks.has_target_combine(op)) {
            result = self.hooks.perform_dag_combine(self.dag, node, self.level);
        }
        // "If nothing happened still, try promoting the operation."
        if result.is_none() && self.level >= CombineLevel::AfterLegalizeVectorOps {
            result = self.promote_int_op(node);
        }
        result
    }

    fn visit(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        match self.dag.opcode(node) {
            ISD::MergeValues => self.visit_merge_values(node),
            ISD::Load => self.visit_load(node).or_else(|| self.indexed(node)),
            ISD::TokenFactor => self.visit_token_factor(node),
            ISD::Select => self.visit_select(node),
            ISD::SelectCC => self.visit_select_cc(node),
            ISD::Store => self.visit_store(node).or_else(|| self.indexed(node)),
            ISD::Freeze => self.visit_freeze(node),
            ISD::SetCC => self.visit_setcc(node),
            ISD::Xor => self.visit_xor(node).or_else(|| self.simplify_demanded(node)),
            ISD::Mul => self.visit_mul(node),
            ISD::And => self
                .visit_and(node)
                .or_else(|| self.reduce_load_width_of(node))
                .or_else(|| self.simplify_demanded(node)),
            ISD::Or => self.visit_or(node).or_else(|| self.simplify_demanded(node)),
            ISD::Add => self.visit_add(node),
            ISD::Sub => self.visit_sub(node),
            ISD::FAdd => self.visit_fadd(node),
            ISD::FSub => self.visit_fsub(node),
            ISD::FMul => self.visit_fmul(node),
            ISD::FNeg => self.visit_fneg(node),
            ISD::FpRound => self.visit_fp_round(node),
            ISD::FpExtend => self.visit_fp_extend(node),
            ISD::SIntToFp | ISD::FpToSInt => self.visit_fp_int_convert(node),
            ISD::FAbs => self.visit_fabs(node),
            ISD::FDiv => self.visit_fdiv(node),
            ISD::Shl | ISD::Srl => self
                .visit_shift(node)
                .or_else(|| if op_is_srl(self.dag.opcode(node)) { self.reduce_load_width_of(node) } else { None })
                .or_else(|| self.simplify_demanded(node)),
            ISD::Sra => {
                self.visit_sra(node).or_else(|| self.visit_shift(node)).or_else(|| self.reduce_load_width_of(node))
            }
            // `visitFunnelShift`, closing with `SimplifyDemandedBits(N)`.
            ISD::FShl | ISD::FShr => self.visit_funnel_shift(node).or_else(|| self.simplify_demanded(node)),
            ISD::BSwap => self.visit_bswap(node),
            ISD::Abs | ISD::AbsMinPoison => self.visit_abs(node),
            ISD::CtLz | ISD::CtTz => self.visit_ctlz_cttz(node),
            ISD::CtPop => self.visit_ctpop(node),
            ISD::Rotl | ISD::Rotr => self.visit_rotate(node),
            ISD::SMin | ISD::SMax | ISD::UMin | ISD::UMax => self.visit_iminmax(node),
            ISD::BitReverse => self.visit_bitreverse(node),
            ISD::SDiv => self.visit_sdiv(node),
            ISD::UDiv => self.visit_udiv(node),
            ISD::SRem | ISD::URem => self.visit_rem(node),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => {
                self.fold_extend_of_cast(node).or_else(|| self.visit_extend(node))
            }
            ISD::BitCast => self.visit_bitcast(node),
            ISD::Truncate => self.visit_truncate(node).or_else(|| self.simplify_demanded(node)),
            ISD::SignExtendInReg => self.visit_sext_in_reg(node),
            _ => None,
        }
    }

    /// `CombineToPreIndexedLoadStore || CombineToPostIndexedLoadStore`.
    fn indexed(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        self.combine_to_pre_indexed_load_store(node).or_else(|| self.combine_to_post_indexed_load_store(node))
    }

    /// `if (SimplifyDemandedBits(SDValue(N, 0))) return SDValue(N, 0)`.
    fn simplify_demanded(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        self.simplify_demanded_bits(node).then_some(node)
    }

    /// `visitMERGE_VALUES` ([L2446][pin]): each result is its operand, so a
    /// lowering that returns several values as one `MERGE_VALUES` leaves no
    /// such node for selection. Returns the node itself so it is not looked
    /// at again.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2446
    fn visit_merge_values(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let count = self.dag.node(node).values.len();
        for res in 0..count {
            let operand = self.dag.operand(node, res);
            self.replace_value(SDValue { node: node.node, res: res as u8 }, operand);
        }
        Some(SDValue { node: node.node, res: 0 })
    }

    /// `visitSETCC` through `SimplifySetCC` ([TargetLowering.cpp L4831][pin]),
    /// the integer folds after `FoldSetCC` (which `get_node` already ran): a
    /// constant on the left moves right, and an equality of an extension (or
    /// of an `and` with a low mask) with a constant that fits the narrow type
    /// compares the narrow value.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L4831
    fn visit_setcc(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let NodeData::CondCode(cond) = self.dag.node(self.dag.operand(node, 2)).data else { return None };
        let op_type = self.dag.value_type(n0);
        if !op_type.is_integer() {
            return None;
        }
        let vt = self.dag.value_type(node);
        let (c0, c1) = (self.dag.constant(n0), self.dag.constant(n1));
        if c0.is_some() && c1.is_none() {
            return Some(self.dag.get_setcc(vt, n1, n0, cond.swapped_operands()));
        }
        let c1 = c1?;
        // `ExpandSetCCOperands`: a pair equals a constant when both halves do
        // (`(lo ^ c.lo) | (hi ^ c.hi)` is zero), so a long a call returned in
        // dx:ax is compared by halves, not joined into one dword first.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe) && self.dag.opcode(n0) == ISD::BuildPair {
            let (low, high) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let half = self.dag.value_type(low);
            let bits = half.size_in_bits();
            let part = |this: &mut Self, value: SDValue, constant: u128| {
                if constant == 0 {
                    value
                } else {
                    let constant = this.dag.get_constant(constant, half);
                    this.dag.get_node(ISD::Xor, half, &[value, constant])
                }
            };
            let low = part(self, low, c1 & super::fold::mask(bits));
            let high = part(self, high, (c1 >> bits) & super::fold::mask(bits));
            let either = self.dag.get_node(ISD::Or, half, &[low, high]);
            let zero = self.dag.get_constant(0, half);
            return Some(self.dag.get_setcc(vt, either, zero, cond));
        }
        // `(x + c0) == c1` is `x == c1 - c0`, `(x ^ c0) == c1` is `x == c0 ^
        // c1` and `(c0 - x) == c1` is `x == c0 - c1`, for a single-use
        // operand.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && matches!(self.dag.opcode(n0), ISD::Add | ISD::Xor | ISD::Sub)
            && self.dag.has_one_use(n0.node)
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let width = op_type.size_in_bits();
            let folded = match (self.dag.opcode(n0), self.dag.constant(a), self.dag.constant(b)) {
                (ISD::Add, _, Some(c0)) => Some((a, c1.wrapping_sub(c0))),
                (ISD::Xor, _, Some(c0)) => Some((a, c0 ^ c1)),
                (ISD::Sub, Some(c0), _) => Some((b, c0.wrapping_sub(c1))),
                _ => None,
            };
            if let Some((x, value)) = folded {
                let value = self.dag.get_constant(value & super::fold::mask(width), op_type);
                return Some(self.dag.get_setcc(vt, x, value, cond));
            }
        }
        if let Some(folded) = self.signed_truncation_check(vt, n0, c1, cond) {
            return Some(folded);
        }
        // `SimplifySetCC`, "Canonicalize GE/LE comparisons to use GT/LT": `x >=
        // c` is `x > c - 1` and `x <= c` is `x < c + 1`, with the
        // always-true extremes folded.
        if self.level < CombineLevel::AfterLegalizeVectorOps && self.hooks.canonicalize_ge_le() {
            let bits = op_type.size_in_bits();
            let (unsigned, ge) = match cond {
                CondCode::SetGe => (false, true),
                CondCode::SetUGe => (true, true),
                CondCode::SetLe => (false, false),
                CondCode::SetULe => (true, false),
                _ => (false, matches!(cond, CondCode::SetEq)),
            };
            if matches!(
                cond,
                CondCode::SetGe | CondCode::SetUGe | CondCode::SetLe | CondCode::SetULe
            ) {
                let mask = super::fold::mask(bits);
                let (min, max) = if unsigned { (0, mask) } else { (1u128 << (bits - 1), mask >> 1) };
                let edge = if ge { min } else { max };
                if c1 & mask == edge {
                    return Some(self.dag.get_bool_constant(true, vt));
                }
                let moved = if ge { c1.wrapping_sub(1) } else { c1.wrapping_add(1) } & mask;
                let new_cond = match (ge, unsigned) {
                    (true, false) => CondCode::SetGt,
                    (true, true) => CondCode::SetUGt,
                    (false, false) => CondCode::SetLt,
                    (false, true) => CondCode::SetULt,
                };
                let rhs = self.dag.get_constant(moved, op_type);
                return Some(self.dag.get_setcc(vt, n0, rhs, new_cond));
            }
        }
        if !matches!(cond, CondCode::SetEq | CondCode::SetNe)
            || self.level != CombineLevel::BeforeLegalizeTypes
            || !self.dag.has_one_use(n0.node)
        {
            return None;
        }
        let width = op_type.size_in_bits();
        let (min_bits, pre_ext, signed) = match self.dag.opcode(n0) {
            ISD::ZeroExtend => (self.dag.width(self.dag.operand(n0, 0)), self.dag.operand(n0, 0), false),
            ISD::SignExtend => (self.dag.width(self.dag.operand(n0, 0)), self.dag.operand(n0, 0), true),
            ISD::And => {
                let mask = self.dag.constant(self.dag.operand(n0, 1))?;
                if (mask.wrapping_add(1) & mask) != 0 {
                    return None;
                }
                (mask.trailing_ones(), self.dag.operand(n0, 0), false)
            }
            _ => return None,
        };
        // Bits the constant needs: `getSignificantBits` or `getActiveBits`.
        let reqd = if signed {
            let s = (c1 << (128 - width)) as i128 >> (128 - width);
            129 - (s ^ (s >> 127)).leading_zeros()
        } else {
            128 - c1.leading_zeros()
        };
        if min_bits == 0 || min_bits >= width || min_bits < reqd {
            return None;
        }
        let min_vt = EVT::integer(min_bits);
        // `isTypeDesirableForOp`: x86 does not narrow to an illegal type.
        if self.tli.is_some_and(|tli| !tli.types.is_type_legal(min_vt)) {
            return None;
        }
        let trunc = self.dag.get_node(ISD::Truncate, min_vt, &[pre_ext]);
        let (rhs, cond) = if min_bits == 1 && c1 == 1 {
            let flipped = if cond == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
            (self.dag.get_constant(0, min_vt), flipped)
        } else {
            (self.dag.get_constant(c1, min_vt), cond)
        };
        Some(self.dag.get_setcc(vt, trunc, rhs, cond))
    }

    /// `optimizeSetCCOfSignedTruncationCheck`: `(add x, 1<<(k-1)) u< 1<<k`, or
    /// its inverse form `(add x, -(1<<(k-1))) u>= -(1<<k)`, is `(sext_inreg
    /// x, k) == x` (`!=` for the inverse).
    fn signed_truncation_check(
        &mut self,
        vt: EVT,
        n0: SDValue,
        c1: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        if self.dag.opcode(n0) != ISD::Add {
            return None;
        }
        let (x, add) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        let c01 = self.dag.constant(add).filter(|_| !self.dag.is_opaque(add))?;
        let xvt = self.dag.value_type(x);
        let width = xvt.size_in_bits();
        let mask = super::fold::mask(width);
        let (mut i1, mut i01) = (c1 & mask, c01 & mask);
        let mut equal = match cond {
            CondCode::SetULt => true,
            CondCode::SetULe => {
                i1 = i1.wrapping_add(1) & mask;
                true
            }
            CondCode::SetUGt => {
                i1 = i1.wrapping_add(1) & mask;
                false
            }
            CondCode::SetUGe => false,
            _ => return None,
        };
        let fits = |i1: u128, i01: u128| i1 > i01 && i1.is_power_of_two() && i01.is_power_of_two();
        if !fits(i1, i01) {
            i1 = i1.wrapping_neg() & mask;
            i01 = i01.wrapping_neg() & mask;
            equal = !equal;
            if !fits(i1, i01) {
                return None;
            }
        }
        let (kept, kept_minus_one) = (i1.trailing_zeros(), i01.trailing_zeros());
        if kept != kept_minus_one + 1 || kept == 0 || kept >= width {
            return None;
        }
        if !self.hooks.should_transform_signed_truncation_check(xvt, kept) {
            return None;
        }
        let ty = self.dag.get_value_type_node(EVT::integer(kept));
        let extended = self.dag.get_node(ISD::SignExtendInReg, xvt, &[x, ty]);
        let new_cond = if equal { CondCode::SetEq } else { CondCode::SetNe };
        Some(self.dag.get_setcc(vt, extended, x, new_cond))
    }

    /// `visitSELECT` ([L13507][pin]) up to the setcc folds: a flipped
    /// condition swaps the arms; an `i1` select is logic
    /// (`foldBoolSelectToLogic`); a select of a select on the same arm
    /// merges into one with an `and`/`or` condition (X86 does not normalize
    /// to a select sequence). The split of an `and`/`or` condition into a
    /// sequence, taken only when the inner select already has users, is not
    /// translated: the arena cannot tell a live user from a dead one.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13507
    /// `visitSELECT_CC`, the `SimplifySetCC` of its compare: a simpler compare
    /// is a new `select_cc`.
    fn visit_select_cc(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let ops: Vec<SDValue> = (0..5).map(|i| self.dag.operand(node, i)).collect();
        if ops[2] == ops[3] {
            return Some(ops[2]);
        }
        let vt = self.dag.value_type(node);
        let NodeData::CondCode(cond) = self.dag.node(ops[4]).data else { return None };
        let compare = self.dag.get_setcc(EVT::I1, ops[0], ops[1], cond);
        let simpler = self.visit_setcc(compare)?;
        if self.dag.opcode(simpler) != ISD::SetCC {
            return None;
        }
        let (lhs, rhs, code) =
            (self.dag.operand(simpler, 0), self.dag.operand(simpler, 1), self.dag.operand(simpler, 2));
        Some(self.dag.get_node(ISD::SelectCC, vt, &[lhs, rhs, ops[2], ops[3], code]))
    }

    fn visit_select(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1, n2) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let vt = self.dag.value_type(node);
        let cond_vt = self.dag.value_type(n0);
        if vt == cond_vt && vt == EVT::I1 {
            if let Some(logic) = self.bool_select_to_logic(n0, n1, n2, vt) {
                return Some(logic);
            }
        }
        // `extractBooleanFlip`: `select (not c), a, b` is `select c, b, a`.
        if self.dag.opcode(n0) == ISD::Xor && self.dag.bool_constant(self.dag.operand(n0, 1)) == Some(true) {
            let flipped = self.dag.operand(n0, 0);
            return Some(self.dag.get_select(vt, flipped, n2, n1));
        }
        if cond_vt != EVT::I1 {
            return None;
        }
        // The pin reaches the sign mask through `or (sext (setle x, -1)), 1`:
        // `SimplifySetCC` makes the compare `x < 0` and `visitSIGN_EXTEND`
        // folds `sext (setlt x, 0)` to `sra`. Neither is translated, so the
        // sign folds run first, with the same result.
        if let Some(mask) = self.select_sign_folds(n0, n1, n2) {
            return Some(mask);
        }
        if let Some(math) = self.fold_select_of_constants(n0, n1, n2, vt) {
            return Some(math);
        }
        let arm_vt = self.dag.value_type(n1);
        // `select c0, (select c1, x, y), y` is `select (and c0, c1), x, y`.
        if self.dag.opcode(n1) == ISD::Select && self.dag.has_one_use(n1.node) {
            let (c1, x, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1), self.dag.operand(n1, 2));
            if y == n2 && self.dag.value_type(c1) == cond_vt {
                let and = self.dag.get_node(ISD::And, cond_vt, &[n0, c1]);
                return Some(self.dag.get_select(arm_vt, and, x, n2));
            }
        }
        // `select c0, x, (select c1, x, y)` is `select (or c0, c1), x, y`.
        if self.dag.opcode(n2) == ISD::Select && self.dag.has_one_use(n2.node) {
            let (c1, x, y) = (self.dag.operand(n2, 0), self.dag.operand(n2, 1), self.dag.operand(n2, 2));
            if x == n1 && self.dag.value_type(c1) == cond_vt {
                let or = self.dag.get_node(ISD::Or, cond_vt, &[n0, c1]);
                return Some(self.dag.get_select(arm_vt, or, n1, y));
            }
        }
        if let Some(folded) = self.select_sign_folds(n0, n1, n2).or_else(|| self.select_single_bit_test(n0, n1, n2)) {
            return Some(folded);
        }
        if self.dag.opcode(n0) == ISD::SetCC && !self.legal_operations() && self.hooks.select_to_select_cc(vt) {
            let (lhs, rhs, code) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1), self.dag.operand(n0, 2));
            return Some(self.dag.get_node(ISD::SelectCC, vt, &[lhs, rhs, n1, n2, code]));
        }
        None
    }

    /// `foldSelectOfConstants` ([L13011][pin]) for an `i1` condition before
    /// operations are legal: a select of two constants is an extend of the
    /// condition, plus or or-ed with a constant, instead of a branch. X86's
    /// `convertSelectOfConstantsToMath` is true for scalars and it has no
    /// `SELECT_CC`, so `shouldConvertSelectOfConstantsToMath` always holds. The
    /// non-`i1` condition arm and `foldSelectOfConstantsUsingSra` are not
    /// translated (the latter is covered by [`Self::select_sign_folds`]).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13011
    fn fold_select_of_constants(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        if !vt.is_integer()
            || self.level >= CombineLevel::AfterLegalizeVectorOps
            || !self.hooks.convert_select_of_constants_to_math(vt)
        {
            return None;
        }
        let (c1, c2) = (self.dag.constant(then)?, self.dag.constant(other)?);
        let mask = super::fold::mask(vt.size_in_bits());
        let (c1, c2) = (c1 & mask, c2 & mask);
        let amount_vt = self.hooks.shift_amount_type(vt);
        let dag = &mut *self.dag;
        if c1 == 1 && c2 == 0 {
            return Some(dag.get_zext_or_trunc(cond, vt));
        }
        if c1 == mask && c2 == 0 {
            return Some(dag.get_sext_or_trunc(cond, vt));
        }
        if c1 == 0 && (c2 == 1 || c2 == mask) {
            let not = dag.get_not(cond, EVT::I1);
            return Some(if c2 == 1 { dag.get_zext_or_trunc(not, vt) } else { dag.get_sext_or_trunc(not, vt) });
        }
        if c1.wrapping_sub(1) & mask == c2 {
            let cond = dag.get_zext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Add, vt, &[cond, other]));
        }
        if c1.wrapping_add(1) & mask == c2 {
            let cond = dag.get_sext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Add, vt, &[cond, other]));
        }
        if c1.is_power_of_two() && c2 == 0 {
            let cond = dag.get_zext_or_trunc(cond, vt);
            let amount = dag.get_constant(u128::from(c1.trailing_zeros()), amount_vt);
            return Some(dag.get_node(ISD::Shl, vt, &[cond, amount]));
        }
        if c1 == mask {
            let cond = dag.get_sext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Or, vt, &[cond, other]));
        }
        if c2 == mask {
            let not = dag.get_not(cond, EVT::I1);
            let not = dag.get_sext_or_trunc(not, vt);
            return Some(dag.get_node(ISD::Or, vt, &[not, then]));
        }
        None
    }

    /// `SimplifySelectCC`, the sign folds: `select (x > -1), C, ~C` is
    /// `(x >>s 31) ^ C`, `select (x < 0), C, ~C` is `(x >>s 31) ^ ~C`, and the
    /// sign `select (x > -1), 1, -1` is `(x >>s 31) | 1`: a mask from the sign
    /// bit instead of a branch (an `i486` has no `cmov`).
    fn select_sign_folds(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(cond) != ISD::SetCC {
            return None;
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(cond, 2)).data else { return None };
        let (x, bound) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
        let x_vt = self.dag.value_type(x);
        let vt = self.dag.value_type(then);
        if !x_vt.is_integer() || !vt.is_integer() {
            return None;
        }
        let (bound, c2, c3) = (self.dag.constant(bound)?, self.dag.constant(then)?, self.dag.constant(other)?);
        let all = super::fold::mask(x_vt.size_in_bits());
        let arm_mask = super::fold::mask(vt.size_in_bits());
        let sign = match cc {
            CondCode::SetGt if bound == all => false,
            CondCode::SetLt if bound == 0 => true,
            _ => return None,
        };
        let shift = self.shift_amount(x_vt, x_vt.size_in_bits() - 1);
        let sra = self.dag.get_node(ISD::Sra, x_vt, &[x, shift]);
        let mask = self.dag.get_sext_or_trunc(sra, vt);
        if c2 == !c3 & arm_mask {
            let keep = self.dag.get_constant(if sign { c3 } else { c2 }, vt);
            return Some(self.dag.get_node(ISD::Xor, vt, &[mask, keep]));
        }
        if !sign && c2 == 1 && c3 == arm_mask {
            let one = self.dag.get_constant(1, vt);
            return Some(self.dag.get_node(ISD::Or, vt, &[mask, one]));
        }
        None
    }

    /// `SimplifySelectCC`, the single-bit test: `select ((x & 2^k) == 0), 0, a`
    /// is `a & ((x << (31-k)) >>s 31)`, a mask of the bit instead of a branch
    /// or a `cmov` (an `i486` has none; the pin's x86 does not avoid it).
    fn select_single_bit_test(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(cond) != ISD::SetCC || self.dag.constant(then) != Some(0) {
            return None;
        }
        let NodeData::CondCode(CondCode::SetEq) = self.dag.node(self.dag.operand(cond, 2)).data else { return None };
        let (and, zero) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
        let vt = self.dag.value_type(then);
        if self.dag.opcode(and) != ISD::And || self.dag.value_type(and) != vt || self.dag.constant(zero) != Some(0) {
            return None;
        }
        let (x, mask) = (self.dag.operand(and, 0), self.dag.constant(self.dag.operand(and, 1))?);
        if !mask.is_power_of_two() {
            return None;
        }
        let width = vt.size_in_bits();
        let up = self.shift_amount(vt, width - 1 - mask.trailing_zeros());
        let shl = self.dag.get_node(ISD::Shl, vt, &[x, up]);
        let down = self.shift_amount(vt, width - 1);
        let sra = self.dag.get_node(ISD::Sra, vt, &[shl, down]);
        Some(self.dag.get_node(ISD::And, vt, &[sra, other]))
    }

    /// `foldBoolSelectToLogic`: a select of `i1`s is an `or`/`and` of the
    /// condition, with the other arm frozen.
    fn bool_select_to_logic(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        let is = |dag: &SelectionDAG, v: SDValue, bit: u128| dag.constant(v) == Some(bit);
        if cond == then || is(self.dag, then, 1) {
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[other]);
            return Some(self.dag.get_node(ISD::Or, vt, &[cond, frozen]));
        }
        if cond == other || is(self.dag, other, 0) {
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[then]);
            return Some(self.dag.get_node(ISD::And, vt, &[cond, frozen]));
        }
        let ones = self.dag.get_all_ones_constant(vt);
        if is(self.dag, other, 1) {
            let not = self.dag.get_node(ISD::Xor, vt, &[cond, ones]);
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[then]);
            return Some(self.dag.get_node(ISD::Or, vt, &[not, frozen]));
        }
        if is(self.dag, then, 0) {
            let not = self.dag.get_node(ISD::Xor, vt, &[cond, ones]);
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[other]);
            return Some(self.dag.get_node(ISD::And, vt, &[not, frozen]));
        }
        None
    }

    /// `visitXOR`'s `not (setcc x, y, cc)` fold ([L10560][pin]): `xor (setcc x,
    /// y, cc), true` is `setcc x, y, !cc`, so a branch on the inverted
    /// condition tests the compare, not a materialized and flipped boolean.
    /// Booleans are zero-or-one here, so `true` is 1 (`-1` of an
    /// `i1`). Only the integer compare is translated; the other `visitXOR`
    /// folds are not (SD208).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10560
    fn visit_xor(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        // `not (or|and a, b)` of `i1`s, when one is a one-use compare, is the
        // and/or of the nots (De Morgan), each of which flips its compare:
        // `xor (or (setcc), (setcc)), 1` was `or; xor 1; test 1; je`.
        if self.dag.constant(n1) == Some(1)
            && vt == EVT::I1
            && matches!(self.dag.opcode(n0), ISD::Or | ISD::And)
            && self.dag.has_one_use(n0.node)
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let one_use_setcc = |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::SetCC && dag.has_one_use(v.node);
            if one_use_setcc(self.dag, a) || one_use_setcc(self.dag, b) {
                let flipped = if self.dag.opcode(n0) == ISD::And { ISD::Or } else { ISD::And };
                let (not_a, not_b) =
                    (self.dag.get_node(ISD::Xor, vt, &[a, n1]), self.dag.get_node(ISD::Xor, vt, &[b, n1]));
                self.add_to_worklist(not_a.node);
                self.add_to_worklist(not_b.node);
                return Some(self.dag.get_node(flipped, vt, &[not_a, not_b]));
            }
        }
        // fold (xor (and x, y), y) -> (and (not x), y)
        if self.hooks.fold_xor_of_and_with_mask()
            && self.dag.opcode(n0) == ISD::And
            && self.dag.has_one_use(n0.node)
            && self.dag.operand(n0, 1) == n1
        {
            let x = self.dag.operand(n0, 0);
            let not_x = self.dag.get_not(x, vt);
            self.add_to_worklist(not_x.node);
            return Some(self.dag.get_node(ISD::And, vt, &[not_x, n1]));
        }
        // fold (xor (shl 1, x), -1) -> (rotl ~1, x): a zero placed in a value
        // of all ones, which the target clears a bit with (`btr`) when it
        // meets an `and` (DAGCombiner::visitXOR).
        let ones = match vt.size_in_bits() {
            128.. => u128::MAX,
            bits => (1u128 << bits) - 1,
        };
        if vt.is_integer()
            && self.dag.opcode(n0) == ISD::Shl
            && self.dag.constant(self.dag.operand(n0, 0)) == Some(1)
            && self.dag.constant(n1) == Some(ones)
            && self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(ISD::Rotl, vt))
        {
            let amount = self.dag.operand(n0, 1);
            let not_one = self.dag.get_constant(ones - 1, vt);
            return Some(self.dag.get_node(ISD::Rotl, vt, &[not_one, amount]));
        }
        if self.dag.constant(n1) != Some(1) || self.dag.opcode(n0) != ISD::SetCC {
            return self.reassociate_ops(ISD::Xor, n0, n1);
        }
        let (lhs, rhs) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(n0, 2)).data else { return None };
        // A far pointer is an integer to the pin (`i32`), not a float.
        let is_float = self.dag.value_type(lhs).is_floating_point();
        let not_cc = if is_float { cc.inverse_float() } else { cc.inverse_integer() };
        // `isCondCodeLegal` once operations are legal: x86 expands `oeq` and
        // `une` (two flags), wasm the unordered ones, so the fold must
        // not make one the target expands.
        if self.level >= CombineLevel::AfterLegalizeVectorOps
            && let (Some(tli), EVT::Simple(operand)) = (self.tli, self.dag.value_type(lhs))
            && tli.cond_code_action(not_cc, operand) != Action::Legal
        {
            return None;
        }
        Some(self.dag.get_setcc(vt, lhs, rhs, not_cc))
    }

    /// The users of `old` read `new`, and are looked at again.
    pub(super) fn replace_value(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        for event in self.dag.replace_all_uses_with(old, new) {
            match event {
                Rauw::Updated(user) => self.add_to_worklist(user),
                Rauw::Merged { into, .. } => self.add_to_worklist_with_users(into),
            }
        }
    }

    /// `getAnyExtOrTrunc`.
    fn any_ext_or_trunc(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let from = self.dag.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.dag.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.dag.get_node(ISD::AnyExtend, vt, &[value])
        }
    }

    /// `visitTRUNCATE`, the cast folds: `trunc (trunc x)` is `trunc x`;
    /// `trunc (ext x)` is `x` of the same type, else the narrower of the two
    /// casts.
    fn visit_truncate(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, n0) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.value_type(n0) == vt {
            return Some(n0);
        }
        if self.dag.constant(n0).is_some() || self.dag.is_undef(n0) {
            let folded = self.dag.get_node(ISD::Truncate, vt, &[n0]);
            return (folded != node).then_some(folded);
        }
        if self.dag.opcode(n0) == ISD::Sub && vt.is_integer() {
            if let Some(saturated) = self.fold_sub_to_usub_sat(vt, n0) {
                return Some(saturated);
            }
        }
        match self.dag.opcode(n0) {
            // `reduceLoadWidth`: the low bytes of a loaded value are a narrower
            // load of the same address (little-endian), which then folds into
            // its user (`test byte [m], 1` where `mov eax, [m]; test al, 1`).
            ISD::Load if n0.res == 0 => self.reduce_load_width(node),
            ISD::Truncate => Some(self.dag.get_node(ISD::Truncate, vt, &[self.dag.operand(n0, 0)])),
            // Narrow a binary operation with a constant operand ahead of the
            // truncate: `trunc (and y, 31)` is `and (trunc y), 31`, which the
            // shift patterns then drop as an unneeded mask. Before
            // legalization only, as the pin does.
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor
                if self.level < CombineLevel::AfterLegalizeVectorOps
                    && self.dag.has_one_use(n0.node)
                    && vt.is_integer()
                    && {
                        let (l, r) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                        l == r || self.dag.constant(l).is_some() || self.dag.constant(r).is_some()
                    } =>
            {
                let (l, r) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                let narrow_l = self.dag.get_node(ISD::Truncate, vt, &[l]);
                let narrow_r = self.dag.get_node(ISD::Truncate, vt, &[r]);
                Some(self.dag.get_node(self.dag.opcode(n0), vt, &[narrow_l, narrow_r]))
            }
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => {
                let (op, x) = (self.dag.opcode(n0), self.dag.operand(n0, 0));
                let from = self.dag.value_type(x);
                if from == vt {
                    Some(x)
                } else if from.bits_lt(vt) {
                    Some(self.dag.get_node(op, vt, &[x]))
                } else {
                    Some(self.dag.get_node(ISD::Truncate, vt, &[x]))
                }
            }
            _ => None,
        }
    }

    /// `reduceLoadWidth` for `trunc (load p)` of a load nothing else reads.
    fn reduce_load_width(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        let NodeData::Mem(info) = self.dag.node(load).data.clone() else { return None };
        let wide = self.dag.value_type(load);
        if info.volatile
            || info.ext.is_some()
            || !self.dag.has_one_use(load.node)
            || !vt.is_integer()
            || vt.size_in_bits() % 8 != 0
            || vt.size_in_bits() >= wide.size_in_bits()
        {
            return None;
        }
        // The load must be legal at the narrower type once types are legal.
        if self.level >= CombineLevel::AfterLegalizeTypes && self.tli.is_some_and(|tli| !tli.types.is_type_legal(vt)) {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        // `AdjustBigEndianShift`: the low bytes of a big-endian value are its
        // last bytes
        let offset =
            if self.dag.config().big_endian { i64::from((wide.size_in_bits() - vt.size_in_bits()) / 8) } else { 0 };
        let ptr = if offset == 0 { ptr } else { self.dag.get_object_ptr_offset(ptr, offset) };
        let base = match info.base {
            super::dag::MemBase::Stack(at) => super::dag::MemBase::Stack(at + offset as i32),
            super::dag::MemBase::CallArgument(at) => super::dag::MemBase::CallArgument(at + offset as i32),
            other => other,
        };
        let align = if offset == 0 { info.align } else { info.align.min(1 << (offset as u32).trailing_zeros()) };
        let narrow = super::dag::MemInfo { mem_vt: vt, align, base, ..info };
        let new = self.dag.get_load(vt, chain, ptr, narrow);
        if self.dag.invariant_loads.contains(&load.node) {
            self.dag.invariant_loads.insert(new.node);
        }
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: new.node, res: 1 });
        Some(new)
    }

    /// `visitANY_EXTEND`, `visitZERO_EXTEND` and `visitSIGN_EXTEND`, the folds
    /// of an extension of another cast: `aext (zext x)` is `zext x`, `zext
    /// (aext x)` is `zext x`, `aext (trunc x)` is `x` made the right width,
    /// `zext (trunc x)` its low bits masked, `sext (trunc x)` sign-extended in
    /// register where that is legal.
    fn fold_extend_of_cast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (op, vt, n0) = (self.dag.opcode(node), self.dag.value_type(node), self.dag.operand(node, 0));
        // An operand a fold made a constant (or undef) since the node was
        // built: `getNode` folds the cast.
        if self.dag.constant(n0).is_some() || self.dag.is_undef(n0) {
            let folded = self.dag.get_node(op, vt, &[n0]);
            return (folded != node).then_some(folded);
        }
        let inner = self.dag.opcode(n0);
        if !matches!(
            inner,
            ISD::AnyExtend | ISD::ZeroExtend | ISD::SignExtend | ISD::Truncate
        ) {
            return None;
        }
        let x = self.dag.operand(n0, 0);
        match (op, inner) {
            (ISD::AnyExtend, ISD::AnyExtend | ISD::ZeroExtend | ISD::SignExtend) => {
                Some(self.dag.get_node(inner, vt, &[x]))
            }
            (ISD::ZeroExtend, ISD::ZeroExtend | ISD::AnyExtend) => Some(self.dag.get_node(ISD::ZeroExtend, vt, &[x])),
            (ISD::SignExtend, ISD::SignExtend | ISD::AnyExtend) => Some(self.dag.get_node(ISD::SignExtend, vt, &[x])),
            (ISD::AnyExtend, ISD::Truncate) => Some(self.any_ext_or_trunc(x, vt)),
            (ISD::ZeroExtend, ISD::Truncate) => {
                let wide = self.any_ext_or_trunc(x, vt);
                let narrow = self.dag.value_type(n0);
                let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), vt);
                Some(self.dag.get_node(ISD::And, vt, &[wide, mask]))
            }
            (ISD::SignExtend, ISD::Truncate)
                if self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::SignExtendInReg, vt)) =>
            {
                let wide = self.any_ext_or_trunc(x, vt);
                let narrow = self.dag.get_value_type_node(self.dag.value_type(n0));
                Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[wide, narrow]))
            }
            _ => None,
        }
    }

    /// `visitSIGN_EXTEND`, `visitZERO_EXTEND` and `visitANY_EXTEND` for a
    /// loaded operand (`tryToFoldExtOfLoad` and the load case after it): the
    /// extension is the load's own. `(zext (load i8 p))` is `zextload i8 p`;
    /// an already extending load is re-extended to the wider type.
    fn visit_extend(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let load = self.dag.operand(node, 0);
        if self.level >= CombineLevel::AfterLegalizeVectorOps || self.dag.opcode(load) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        if info.volatile {
            return None;
        }
        let op = self.dag.opcode(node);
        let wanted = match op {
            ISD::ZeroExtend => LoadExt::ZExtLoad,
            ISD::SignExtend => LoadExt::SExtLoad,
            _ => LoadExt::ExtLoad,
        };
        let ext = match info.ext {
            None => wanted,
            Some(have) if have == wanted => have,
            Some(LoadExt::ExtLoad) => wanted,
            Some(have) if wanted == LoadExt::ExtLoad => have,
            Some(_) => return None,
        };
        let wide = self.dag.value_type(node);
        let others: Vec<u32> = self.dag.users(load).filter(|&user| user != node.node).collect();
        let mut compares = Vec::new();
        if !others.is_empty() {
            // `isLoadExtLegal`, and `ExtendUsesToFormExtLoad`: the other
            // readers are compares against a constant, which can be
            // widened, or take the value back through a (free)
            // truncate.
            let EVT::Simple(wide_mvt) = wide else { return None };
            let EVT::Simple(mem_mvt) = info.mem_vt else { return None };
            let legal = self.tli.is_some_and(|tli| tli.load_ext_action(ext, wide_mvt, mem_mvt) == Action::Legal);
            if info.ext.is_some() || !legal || !self.extend_uses(node, load, op, &others, &mut compares) {
                return None;
            }
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let info = super::dag::MemInfo { ext: None, ..info };
        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, info);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        if !others.is_empty() {
            // The narrow value is the extending load, truncated.
            let narrow = self.dag.value_type(load);
            let truncated = self.dag.get_node(ISD::Truncate, narrow, &[extended]);
            for compare in compares {
                self.extend_compare(compare, load, extended, op);
            }
            self.replace_value(load, truncated);
        }
        Some(extended)
    }

    /// `ExtendUsesToFormExtLoad`: whether the other users of `load` can live
    /// with its extension; the compares to widen are collected.
    fn extend_uses(
        &self,
        node: SDValue,
        load: SDValue,
        op: ISD,
        others: &[u32],
        compares: &mut Vec<u32>,
    ) -> bool {
        let mut copy_to_reg = false;
        for &user in others {
            let user_value = SDValue { node: user, res: 0 };
            if op != ISD::AnyExtend && self.dag.opcode(user_value) == ISD::SetCC {
                let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(user_value, 2)).data else { return false };
                if op == ISD::ZeroExtend && cc.is_signed_int() {
                    return false;
                }
                let mut widens = false;
                for at in 0..2 {
                    let operand = self.dag.operand(user_value, at);
                    if operand == load {
                        continue;
                    }
                    if self.dag.constant(operand).is_none() {
                        return false;
                    }
                    widens = true;
                }
                if widens {
                    compares.push(user);
                }
                continue;
            }
            // Every integer truncate between these widths is free on x86.
            if self.dag.opcode(user_value) == ISD::CopyToReg {
                copy_to_reg = true;
            }
        }
        if copy_to_reg {
            // Both the narrow and the wide value are live out: only worth it if
            // a compare is widened.
            let both =
                self.dag.users(node).any(|user| self.dag.opcode(SDValue { node: user, res: 0 }) == ISD::CopyToReg);
            if both {
                return !compares.is_empty();
            }
        }
        true
    }

    /// `ExtendSetCCUses`: the compare of the load against a constant is a
    /// compare of the extending load against the extended constant.
    fn extend_compare(
        &mut self,
        compare: u32,
        load: SDValue,
        extended: SDValue,
        op: ISD,
    ) {
        let compare_value = SDValue { node: compare, res: 0 };
        let wide = self.dag.value_type(extended);
        let mut operands = [self.dag.operand(compare_value, 0), self.dag.operand(compare_value, 1)];
        for operand in &mut operands {
            *operand = if *operand == load { extended } else { self.dag.get_node(op, wide, &[*operand]) };
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(compare_value, 2)).data else { return };
        let vt = self.dag.value_type(compare_value);
        let replacement = self.dag.get_setcc(vt, operands[0], operands[1], cc);
        self.replace_value(compare_value, replacement);
    }

    /// `areBitwiseNotOfEachother`: one is `(xor other, -1)`.
    fn are_bitwise_not_of_each_other(
        &self,
        a: SDValue,
        b: SDValue,
    ) -> bool {
        let is_not = |dag: &SelectionDAG, v: SDValue, of: SDValue| {
            dag.opcode(v) == ISD::Xor
                && dag.operand(v, 0) == of
                && dag.constant(dag.operand(v, 1)) == Some(super::fold::mask(dag.width(v)))
        };
        is_not(self.dag, a, b) || is_not(self.dag, b, a)
    }

    /// `visitAND`, the part `SimplifyDemandedBits` does for it: `(and x, c)` is
    /// `x` when every bit is either set in `c` or known zero in `x`, so
    /// `and (setcc), 1` is the `setcc` (it is 0 or 1 already).
    fn visit_and(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        if n0 == n1 {
            return Some(n0);
        }
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(ISD::And, vt, &[n1, n0]));
        }
        if self.are_bitwise_not_of_each_other(n0, n1) {
            return Some(self.dag.get_constant(0, vt));
        }
        // `(and x, c)` is zero when `x` is known zero wherever `c` is set.
        if let Some(c) = self.dag.constant(n1) {
            if c & !super::known_bits::known_bits(self.dag, n0).zero & super::fold::mask(vt.size_in_bits()) == 0 {
                return Some(self.dag.get_constant(0, vt));
            }
        }
        if let Some(folded) = self.fold_logic_of_setccs(true, n0, n1) {
            return Some(folded);
        }
        if let Some(reassociated) = self.reassociate_ops(ISD::And, n0, n1) {
            return Some(reassociated);
        }
        if let (Some(c1), true) = (self.dag.constant(n1), !self.dag.is_opaque(n1)) {
            // `(and (or x, C), D)` is `D` when `D` is a subset of `C`.
            if self.dag.opcode(n0) == ISD::Or {
                if let Some(c) = self.dag.constant(self.dag.operand(n0, 1)) {
                    if c1 & !c == 0 {
                        return Some(n1);
                    }
                }
            }
        }
        let all = super::fold::mask(vt.size_in_bits());
        for (other, constant) in [(n0, n1), (n1, n0)] {
            if let Some(c) = self.dag.constant(constant) {
                if self.dag.is_opaque(constant) {
                    continue;
                }
                // `(and (anyext x), c)` is `(zext x)` when `c` keeps no bit
                // above `x` and every bit `x` may set (`SimplifyDemandedBits`
                // drops the `and` of a promoted boolean).
                if self.dag.opcode(other) == ISD::AnyExtend {
                    let source = self.dag.operand(other, 0);
                    let width = self.dag.value_type(source).size_in_bits();
                    let narrow = super::fold::mask(width);
                    let known = super::known_bits::known_bits(self.dag, source);
                    if c & !narrow == 0 && narrow & !(known.zero | c) == 0 {
                        return Some(self.dag.get_node(ISD::ZeroExtend, vt, &[source]));
                    }
                }
                let known = super::known_bits::known_bits(self.dag, other);
                if all & !(known.zero | c) == 0 {
                    return Some(other);
                }
            }
        }
        None
    }

    /// `visitSRA`, by way of `SimplifyDemandedBits`: `(sra (shl x, c), c)` is
    /// `sign_extend_inreg x` from the low `width - c` bits, before operations
    /// are legal or when the target has that extension.
    fn visit_sra(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.hooks.fold_sra_of_trunc_of_shift() && self.dag.opcode(n0) == ISD::Truncate {
            if let Some(folded) = self.sra_of_trunc_of_shift(node) {
                return Some(folded);
            }
        }
        if self.dag.opcode(n0) != ISD::Shl {
            return None;
        }
        let (c, up) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(n0, 1))?);
        if c != up || c == 0 || c >= u128::from(vt.size_in_bits()) {
            return None;
        }
        let low = EVT::integer(vt.size_in_bits() - c as u32);
        let legal = self.level == CombineLevel::BeforeLegalizeTypes
            || self.tli.is_some_and(|tli| tli.operation_action(ISD::SignExtendInReg, low) == Action::Legal);
        if !legal {
            return None;
        }
        let ty = self.dag.get_value_type_node(low);
        let x = self.dag.operand(n0, 0);
        Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[x, ty]))
    }

    /// `(sra (trunc (srl x, c1)), c2)` and the `sra` form are `(trunc (sra x,
    /// c1 + c2))` when `c1` is the number of bits the truncate removes
    /// ([pin][pin]).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11664
    fn sra_of_trunc_of_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let inner = self.dag.operand(n0, 0);
        if !matches!(self.dag.opcode(inner), ISD::Srl | ISD::Sra) || !self.dag.has_one_use(inner.node) {
            return None;
        }
        let amount = self.dag.operand(inner, 1);
        if !self.dag.has_one_use(amount.node) {
            return None;
        }
        let (large_shift, c2) = (self.dag.constant(amount)?, self.dag.constant(n1)?);
        let large = self.dag.value_type(inner);
        let removed = large.size_in_bits() - vt.size_in_bits();
        if large_shift != u128::from(removed) {
            return None;
        }
        let total = self.shift_amount(large, removed + c2 as u32);
        let sra = self.dag.get_node(ISD::Sra, large, &[self.dag.operand(inner, 0), total]);
        Some(self.dag.get_node(ISD::Truncate, vt, &[sra]))
    }

    /// `visitADD` through `visitADDLike`, the folds that need no target
    /// query: a constant moves right, `x + ~x` is -1, constants merge through
    /// a `sub` or an `add`, and an `add` of a negation or of a `sub` of the
    /// other operand is a `sub` or the operand itself.
    fn visit_add(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(ISD::Add, vt, &[n1, n0]));
        }
        let ones = super::fold::mask(vt.size_in_bits());
        let is_not = |dag: &SelectionDAG, v: SDValue, of: SDValue| {
            dag.opcode(v) == ISD::Xor && dag.operand(v, 0) == of && dag.constant(dag.operand(v, 1)) == Some(ones)
        };
        if is_not(self.dag, n0, n1) || is_not(self.dag, n1, n0) {
            return Some(self.dag.get_constant(ones, vt));
        }
        let plain = |dag: &SelectionDAG, v: SDValue| dag.constant(v).filter(|_| !dag.is_opaque(v)).is_some();
        if plain(self.dag, n1) {
            if self.dag.opcode(n0) == ISD::Sub {
                let (n00, n01) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                // `(add (sub x, c0), c1)` is `(add x, c1 - c0)`.
                if plain(self.dag, n01) {
                    let diff = self.dag.get_node(ISD::Sub, vt, &[n1, n01]);
                    return Some(self.dag.get_node(ISD::Add, vt, &[n00, diff]));
                }
                // `(add (sub c0, x), c1)` is `(sub c0 + c1, x)`.
                if plain(self.dag, n00) {
                    let sum = self.dag.get_node(ISD::Add, vt, &[n1, n00]);
                    return Some(self.dag.get_node(ISD::Sub, vt, &[sum, n01]));
                }
            }
            // `(add (add x, c0), c1)` is `(add x, c0 + c1)` (`reassociateOps`).
            if self.dag.opcode(n0) == ISD::Add
                && plain(self.dag, self.dag.operand(n0, 1))
                && !self.reassociation_can_break_addressing_mode_pattern(node, n0, n1)
            {
                let sum = self.dag.get_node(ISD::Add, vt, &[n1, self.dag.operand(n0, 1)]);
                return Some(self.dag.get_node(ISD::Add, vt, &[self.dag.operand(n0, 0), sum]));
            }
        }
        let is_neg =
            |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::Sub && dag.constant(dag.operand(v, 0)) == Some(0);
        // `(add (sub 0, a), b)` is `(sub b, a)`; likewise on the right.
        if is_neg(self.dag, n0) {
            return Some(self.dag.get_node(ISD::Sub, vt, &[n1, self.dag.operand(n0, 1)]));
        }
        if is_neg(self.dag, n1) {
            return Some(self.dag.get_node(ISD::Sub, vt, &[n0, self.dag.operand(n1, 1)]));
        }
        // `x + (b - x)` and `(b - y) + y` are `b`.
        if self.dag.opcode(n1) == ISD::Sub && self.dag.operand(n1, 1) == n0 {
            return Some(self.dag.operand(n1, 0));
        }
        if self.dag.opcode(n0) == ISD::Sub && self.dag.operand(n0, 1) == n1 {
            return Some(self.dag.operand(n0, 0));
        }
        if self.dag.opcode(n0) == ISD::Sub && self.dag.opcode(n1) == ISD::Sub {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let (c, d) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            // `(a - b) + (c - a)` is `c - b`; `(a - b) + (b - d)` is `a - d`.
            if d == a {
                return Some(self.dag.get_node(ISD::Sub, vt, &[c, b]));
            }
            if c == b {
                return Some(self.dag.get_node(ISD::Sub, vt, &[a, d]));
            }
        }
        if !self.reassociation_can_break_addressing_mode_pattern(node, n0, n1) {
            return self.reassociate_ops(ISD::Add, n0, n1);
        }
        None
    }

    /// `visitSHL`, `visitSRL` and `visitSRA`, the constant-amount folds: the
    /// result is 0 when every bit is known zero; a shift of a shift of the
    /// same kind is one shift by the sum (0 once the sum reaches the width,
    /// for `sra` the sign fill).
    fn visit_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        // `getNode`'s `simplifyShift`, for an operand a fold made undef since
        // the shift was built: `undef << x` is 0.
        if self.dag.is_undef(n0) {
            return Some(self.dag.get_constant(0, vt));
        }
        if op != ISD::Sra {
            let all = super::fold::mask(vt.size_in_bits());
            let known = super::known_bits::known_bits(self.dag, node);
            if all & !known.zero == 0 {
                return Some(self.dag.get_constant(0, vt));
            }
        }
        if op == ISD::Srl && self.dag.opcode(n0) == ISD::Truncate {
            if let Some(folded) = self.srl_of_trunc_of_srl(node) {
                return Some(folded);
            }
        }
        if op == ISD::Shl && self.dag.opcode(n0) == ISD::Srl {
            if let Some(folded) = self.shl_of_srl(node) {
                return Some(folded);
            }
        }
        // `shl (sra x, c), c` is `and x, (-1 << c)` ([L11245][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11245
        if op == ISD::Shl && self.dag.opcode(n0) == ISD::Sra && self.dag.operand(n0, 1) == n1 {
            if let Some(c) = self.dag.constant(n1).filter(|&c| c < width) {
                let mask = self
                    .dag
                    .get_constant(
                        (super::fold::mask(vt.size_in_bits()) << c) & super::fold::mask(vt.size_in_bits()),
                        vt,
                    );
                return Some(self.dag.get_node(ISD::And, vt, &[self.dag.operand(n0, 0), mask]));
            }
        }
        // `srl (sra x, y), w-1` is `srl x, w-1`: the sign bit is the one `sra`
        // leaves alone ([L11886][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11886
        if op == ISD::Srl && self.dag.opcode(n0) == ISD::Sra && self.dag.constant(n1) == Some(width - 1) {
            return Some(self.dag.get_node(ISD::Srl, vt, &[self.dag.operand(n0, 0), n1]));
        }
        if self.dag.opcode(n0) != op {
            return None;
        }
        let (c2, inner) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(n0, 1))?);
        let (x, sum) = (self.dag.operand(n0, 0), c2 + inner);
        if sum < width {
            let amount = self.dag.get_constant(sum, self.dag.value_type(n1));
            return Some(self.dag.get_node(op, vt, &[x, amount]));
        }
        if op == ISD::Sra {
            let amount = self.dag.get_constant(width - 1, self.dag.value_type(n1));
            return Some(self.dag.get_node(op, vt, &[x, amount]));
        }
        Some(self.dag.get_constant(0, vt))
    }

    /// `visitSHL`: `shl (srl x, c1), c2` is `and (srl x, c1 - c2), (-1 << c1
    /// >> (c1 - c2))` when `c2 <= c1`, else `and (shl x, c2 - c1), (-1 <<
    /// c2)`; only when the inner shift has no other use or the amounts are
    /// equal (`shouldFoldConstantShiftPairToMask`: true on these CPUs).
    fn shl_of_srl(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (inner, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let (c2, c1) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(inner, 1))?);
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        if c1 >= width || c2 >= width || (c1 != c2 && !self.dag.has_one_use(inner.node)) {
            return None;
        }
        let x = self.dag.operand(inner, 0);
        let amount_vt = self.dag.value_type(n1);
        let all = super::fold::mask(vt.size_in_bits());
        if c2 <= c1 {
            let diff = c1 - c2;
            let mask = ((all << c1) & all) >> diff;
            let shifted = if diff == 0 {
                x
            } else {
                let by = self.dag.get_constant(diff, amount_vt);
                self.dag.get_node(ISD::Srl, vt, &[x, by])
            };
            let mask = self.dag.get_constant(mask, vt);
            return Some(self.dag.get_node(ISD::And, vt, &[shifted, mask]));
        }
        let diff = c2 - c1;
        let by = self.dag.get_constant(diff, amount_vt);
        let shifted = self.dag.get_node(ISD::Shl, vt, &[x, by]);
        let mask = self.dag.get_constant((all << c2) & all, vt);
        Some(self.dag.get_node(ISD::And, vt, &[shifted, mask]))
    }

    /// `visitSRL`: `srl (trunc (srl x, c1)), c2` is 0 or `trunc (srl x,
    /// c1+c2)` when the truncated width plus `c1` is the inner width, else
    /// `trunc (and (srl x, c1+c2), mask)` when both nodes have one use.
    fn srl_of_trunc_of_srl(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (trunc, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let inner = self.dag.operand(trunc, 0);
        if self.dag.opcode(inner) != ISD::Srl {
            return None;
        }
        let (c1, c2) = (self.dag.constant(self.dag.operand(inner, 1))?, self.dag.constant(n1)?);
        let (vt, inner_vt) = (self.dag.value_type(node), self.dag.value_type(inner));
        let (op_size, inner_size) = (u128::from(vt.size_in_bits()), u128::from(inner_vt.size_in_bits()));
        let amount_vt = self.dag.value_type(self.dag.operand(inner, 1));
        if c1 + op_size == inner_size {
            if c1 + c2 >= inner_size {
                return Some(self.dag.get_constant(0, vt));
            }
            let amount = self.dag.get_constant(c1 + c2, amount_vt);
            let shifted = self.dag.get_node(ISD::Srl, inner_vt, &[self.dag.operand(inner, 0), amount]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[shifted]));
        }
        if self.dag.has_one_use(trunc.node) && self.dag.has_one_use(inner.node) && c1 + c2 < inner_size && c2 < op_size
        {
            let amount = self.dag.get_constant(c1 + c2, amount_vt);
            let shifted = self.dag.get_node(ISD::Srl, inner_vt, &[self.dag.operand(inner, 0), amount]);
            let mask = self.dag.get_constant(super::fold::mask((op_size - c2) as u32), inner_vt);
            let masked = self.dag.get_node(ISD::And, inner_vt, &[shifted, mask]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[masked]));
        }
        None
    }

    /// `visitFREEZE`, the first fold: `freeze x` is `x` when `x` cannot be
    /// undef or poison. (The pin's second fold, hoisting the freeze over a
    /// multi-use operand, is not translated.)
    fn visit_freeze(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let x = self.dag.operand(node, 0);
        if !self.guaranteed_not_undef_or_poison(x, 0) {
            return self.push_freeze_through(x);
        }
        // The users now read `x` directly: their users may fold with it too
        // (`not (or (cmp), freeze (cmp))` was visited with the freeze there).
        let users: Vec<u32> = self.dag.users(node).collect();
        for user in users {
            self.add_to_worklist_with_users(user);
        }
        Some(x)
    }

    /// `freeze(op(x, ...))` to `op(freeze(x), ...)`: the operation cannot make
    /// undef or poison itself (flags ignored), has one use, and has at most one
    /// operand that may be undef or poison (a `setcc` may have two).
    fn push_freeze_through(
        &mut self,
        op: SDValue,
    ) -> Option<SDValue> {
        let opcode = self.dag.opcode(op);
        let is_setcc = opcode == ISD::SetCC;
        let integer_setcc = is_setcc && self.dag.value_type(self.dag.operand(op, 0)).is_integer();
        if !self.hooks.push_freeze_through_operands()
            || !(integer_setcc || matches!(opcode, ISD::And | ISD::Or | ISD::Xor | ISD::Add | ISD::Sub))
            || !self.dag.has_one_use(op.node)
        {
            return None;
        }
        let mut maybe_poison: Vec<usize> = Vec::new();
        // A `setcc`'s third operand is its condition code, not a value.
        let values = if is_setcc { 2 } else { self.dag.node(op).ops.len() };
        for at in 0..values {
            let operand = self.dag.operand(op, at);
            if self.guaranteed_not_undef_or_poison(operand, 0) {
                continue;
            }
            let seen = maybe_poison.iter().any(|&other| self.dag.operand(op, other) == operand);
            if !maybe_poison.is_empty() && !seen && !is_setcc {
                return None;
            }
            if !seen {
                maybe_poison.push(at);
            }
        }
        let mut ops: Vec<SDValue> = (0..self.dag.node(op).ops.len()).map(|at| self.dag.operand(op, at)).collect();
        for at in maybe_poison {
            if self.dag.is_undef(ops[at]) {
                continue;
            }
            let frozen = self.dag.get_node(ISD::Freeze, self.dag.value_type(ops[at]), &[ops[at]]);
            for other in ops.iter_mut().filter(|o| **o == self.dag.operand(op, at)) {
                *other = frozen;
            }
        }
        let vt = self.dag.value_type(op);
        // The node is rebuilt without its poison-generating flags.
        Some(self.dag.get_node(opcode, vt, &ops))
    }

    /// `isGuaranteedNotToBeUndefOrPoison`, for the nodes the combiner builds:
    /// constants, registers, frame indexes and `freeze`; a load through such a
    /// pointer; and an integer operation without a poison-generating flag whose
    /// operands are.
    ///
    /// DEVIATE SD213: no vector, `AssertSext`/`AssertZext`, overflow-flag or
    /// shift-amount range reasoning beyond a constant amount below the width.
    /// Exit: those. Owner sdag-b. Review 2026-12-10.
    fn guaranteed_not_undef_or_poison(
        &self,
        value: SDValue,
        depth: u32,
    ) -> bool {
        if self.dag.opcode(value) == ISD::Freeze {
            return true;
        }
        if depth >= 6 {
            return false;
        }
        if self.dag.constant(value).is_some() {
            return true;
        }
        let poison_flags = SDNodeFlags::NO_UNSIGNED_WRAP.0 | SDNodeFlags::NO_SIGNED_WRAP.0 | SDNodeFlags::EXACT.0;
        let flagged = self.dag.node(value).flags.0 & poison_flags != 0;
        let all = |ops: &[usize]| {
            ops.iter().all(|&i| self.guaranteed_not_undef_or_poison(self.dag.operand(value, i), depth + 1))
        };
        match self.dag.opcode(value) {
            ISD::Register | ISD::FrameIndex | ISD::ValueType | ISD::CopyFromReg => true,
            ISD::Load => all(&[1]),
            ISD::SetCC => all(&[0, 1]),
            ISD::And | ISD::Or | ISD::Xor | ISD::Add | ISD::Sub | ISD::Mul if !flagged => all(&[0, 1]),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend | ISD::Truncate if !flagged => all(&[0]),
            ISD::Shl | ISD::Srl | ISD::Sra if !flagged => {
                let width = u128::from(self.dag.value_type(value).size_in_bits());
                self.dag.constant(self.dag.operand(value, 1)).is_some_and(|by| by < width) && all(&[0])
            }
            _ => false,
        }
    }

    /// `PromoteIntBinOp`, `PromoteIntShiftOp` and `PromoteExtend`: an operation
    /// the target wants in a wider type is done there, and truncated.
    ///
    /// DEVIATE SD211: `AssertSext`/`AssertZext` are not looked through, and a
    /// volatile load is any-extended rather than widened. Exit: those. Owner
    /// sdag-b. Review 2026-12-10.
    fn promote_int_op(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let class = match op {
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => 0,
            ISD::Shl | ISD::Sra | ISD::Srl => 1,
            ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend => 2,
            _ => return None,
        };
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        let tli = self.tli?;
        let wide = self.hooks.promote_type(self.dag, tli, node)?;
        match class {
            2 => {
                let x = self.dag.operand(node, 0);
                Some(self.dag.get_node(op, vt, &[x]))
            }
            0 => {
                let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let mut loads = Vec::new();
                let (w0, w1) =
                    (self.promote_operand(n0, wide, &mut loads)?, self.promote_operand(n1, wide, &mut loads)?);
                let wide_op = self.dag.get_node(op, wide, &[w0, w1]);
                self.replace_loads_with_promoted(loads);
                Some(self.dag.get_node(ISD::Truncate, vt, &[wide_op]))
            }
            _ => {
                let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let flags = self.dag.node(node).flags;
                let mut loads = Vec::new();
                let w0 = match op {
                    ISD::Sra => self.sext_promote_operand(n0, wide, tli, &mut loads)?,
                    ISD::Srl => self.zext_promote_operand(n0, wide, &mut loads)?,
                    _ if flags.0 & SDNodeFlags::NO_UNSIGNED_WRAP.0 != 0 => {
                        self.zext_promote_operand(n0, wide, &mut loads)?
                    }
                    _ if flags.0 & SDNodeFlags::NO_SIGNED_WRAP.0 != 0 => {
                        self.sext_promote_operand(n0, wide, tli, &mut loads)?
                    }
                    _ => self.promote_operand(n0, wide, &mut loads)?,
                };
                let wide_op = self.dag.get_node(op, wide, &[w0, n1]);
                self.replace_loads_with_promoted(loads);
                Some(self.dag.get_node(ISD::Truncate, vt, &[wide_op]))
            }
        }
    }

    /// `PromoteOperand`: a load becomes an extending load of its memory type
    /// (recorded in `loads` for `replace_loads_with_promoted`), a constant is
    /// extended, anything else any-extended.
    fn promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        if self.dag.opcode(value) == ISD::Load && value.res == 0 {
            if let NodeData::Mem(info) = self.dag.node(value).data.clone() {
                let ext = info.ext.unwrap_or(LoadExt::ExtLoad);
                if !info.volatile
                    && let (EVT::Simple(wide_mvt), EVT::Simple(mem_mvt)) = (wide, info.mem_vt)
                {
                    let legal =
                        self.tli.is_some_and(|tli| tli.load_ext_action(ext, wide_mvt, mem_mvt) == Action::Legal);
                    if legal {
                        let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                        let plain = super::dag::MemInfo { ext: None, ..info };
                        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, plain);
                        loads.push((value, extended));
                        return Some(extended);
                    }
                }
            }
        }
        if self.dag.constant(value).is_some() {
            let byte_sized = self.dag.value_type(value).size_in_bits() % 8 == 0;
            let ext = if byte_sized { ISD::SignExtend } else { ISD::ZeroExtend };
            return Some(self.dag.get_node(ext, wide, &[value]));
        }
        if !self.tli?.is_operation_legal(ISD::AnyExtend, wide) {
            return None;
        }
        Some(self.dag.get_node(ISD::AnyExtend, wide, &[value]))
    }

    /// `ReplaceLoadWithPromotedLoad`: the other users of a promoted load read
    /// the extending load truncated, and its chain users the new chain.
    fn replace_loads_with_promoted(
        &mut self,
        loads: Vec<(SDValue, SDValue)>,
    ) {
        for (load, extended) in loads {
            let narrow = self.dag.value_type(load);
            let truncated = self.dag.get_node(ISD::Truncate, narrow, &[extended]);
            self.replace_value(load, truncated);
            self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        }
    }

    fn sext_promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        tli: &TargetLowering,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        if !tli.is_operation_legal(ISD::SignExtendInReg, wide) {
            return None;
        }
        let old = self.dag.value_type(value);
        let promoted = self.promote_operand(value, wide, loads)?;
        let ty = self.dag.get_value_type_node(old);
        Some(self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty]))
    }

    /// `ZExtPromoteOperand`: `getZeroExtendInReg` is an `and` with the old
    /// type's mask.
    fn zext_promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        let old = self.dag.value_type(value);
        let promoted = self.promote_operand(value, wide, loads)?;
        let mask = self.dag.get_constant(super::fold::mask(old.size_in_bits()), wide);
        Some(self.dag.get_node(ISD::And, wide, &[promoted, mask]))
    }

    /// `visitSUB`, the first folds: `x - x` is 0, and `x - C` is `x + -C`, so
    /// the add-shaped folds and the matcher's `dec`/`lea` patterns see one
    /// form. (The pin's `umin`, select, `ctlz` and negation folds are not
    /// translated.)
    fn visit_sub(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        if n0 == n1 {
            return Some(self.dag.get_constant(0, vt));
        }
        if let Some(saturated) = self.fold_sub_to_usub_sat(vt, node) {
            return Some(saturated);
        }
        let c = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1))?;
        let negated = self.dag.get_constant(c.wrapping_neg() & super::fold::mask(vt.size_in_bits()), vt);
        Some(self.dag.get_node(ISD::Add, vt, &[n0, negated]))
    }

    /// `getTruncatedUSUBSAT`
    /// ([L4070](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4070)):
    /// a `usubsat` of `lhs` and
    /// `rhs` in `dst`, the operands clamped and truncated from `src`.
    fn truncated_usub_sat(
        &mut self,
        dst: EVT,
        src: EVT,
        lhs: SDValue,
        rhs: SDValue,
    ) -> Option<SDValue> {
        if dst == src {
            return Some(self.dag.get_node(ISD::USubSat, dst, &[lhs, rhs]));
        }
        // "If the LHS is zero-extended then we can perform the USUBSAT as
        // DstVT by clamping RHS."
        let (src_bits, dst_bits) = (src.size_in_bits(), dst.size_in_bits());
        let upper = super::fold::mask(src_bits) & !super::fold::mask(dst_bits);
        if !super::known_bits::masked_value_is_zero(self.dag, lhs, upper) {
            return None;
        }
        let limit = self.dag.get_constant(super::fold::mask(dst_bits), src);
        let clamped = self.dag.get_node(ISD::UMin, src, &[rhs, limit]);
        let rhs = self.dag.get_node(ISD::Truncate, dst, &[clamped]);
        let lhs = self.dag.get_node(ISD::Truncate, dst, &[lhs]);
        Some(self.dag.get_node(ISD::USubSat, dst, &[lhs, rhs]))
    }

    /// `foldSubToUSubSat`
    /// ([L4100](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4100),
    /// called from `visitSUB`
    /// [L4501](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4501)
    /// and `visitTRUNCATE`
    /// [L17910](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17910)):
    /// `umax(a, b) - b` and `a - umin(a, b)` are
    /// `usubsat(a, b)`, optionally as a truncated type. `sub` is a `SUB` node
    /// and `dst` the type the result is wanted in. O(1) per node.
    fn fold_sub_to_usub_sat(
        &mut self,
        dst: EVT,
        sub: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(sub) != ISD::Sub {
            return None;
        }
        // `!LegalOperations || hasOperation(USUBSAT, DstVT)`
        if self.legal_operations() && !self.tli?.is_operation_legal_or_custom(ISD::USubSat, dst) {
            return None;
        }
        let sub_vt = self.dag.value_type(sub);
        let (op0, op1) = (self.dag.operand(sub, 0), self.dag.operand(sub, 1));
        // "Try to find umax(a,b) - b or a - umin(a,b) patterns."
        if self.dag.opcode(op0) == ISD::UMax && self.dag.has_one_use(op0.node) {
            let (max_lhs, max_rhs) = (self.dag.operand(op0, 0), self.dag.operand(op0, 1));
            if max_lhs == op1 {
                return self.truncated_usub_sat(dst, sub_vt, max_rhs, op1);
            }
            if max_rhs == op1 {
                return self.truncated_usub_sat(dst, sub_vt, max_lhs, op1);
            }
        }
        if self.dag.opcode(op1) == ISD::UMin && self.dag.has_one_use(op1.node) {
            let (min_lhs, min_rhs) = (self.dag.operand(op1, 0), self.dag.operand(op1, 1));
            if min_lhs == op0 {
                return self.truncated_usub_sat(dst, sub_vt, op0, min_rhs);
            }
            if min_rhs == op0 {
                return self.truncated_usub_sat(dst, sub_vt, op0, min_lhs);
            }
        }
        // `sub(a, trunc(umin(zext(a), b)))` is `usubsat(a, trunc(umin(b,
        // SatLimit)))`.
        if self.dag.opcode(op1) == ISD::Truncate {
            let min = self.dag.operand(op1, 0);
            if self.dag.opcode(min) == ISD::UMin && self.dag.has_one_use(min.node) {
                let (min_lhs, min_rhs) = (self.dag.operand(min, 0), self.dag.operand(min, 1));
                for (zext, other) in [(min_lhs, min_rhs), (min_rhs, min_lhs)] {
                    if self.dag.opcode(zext) == ISD::ZeroExtend && self.dag.operand(zext, 0) == op0 {
                        return self.truncated_usub_sat(dst, self.dag.value_type(zext), zext, other);
                    }
                }
            }
        }
        None
    }

    /// `visitSTORE`, `replaceStoreOfFPConstant`: `store float 1.5, p` is
    /// `store i32 0x3fc00000, p`, and a double constant the FPU cannot load
    /// directly is two `i32` stores, so the constant is never loaded onto the
    /// x87 stack to be stored. (The pin's `f16`, `f80` and `f128` stay.)
    fn visit_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        // DEVIATE SD256: the call argument the builder cast to its bits stays
        // a float store when the load can never become an integer load
        // (`visit_bitcast`: volatile or extending): the cast alone would cost
        // a temporary.
        if matches!(info.base, super::dag::MemBase::CallArgument(_))
            && self.dag.opcode(value) == ISD::BitCast
            && self.dag.value_type(value) == EVT::I32
        {
            let source = self.dag.operand(value, 0);
            if self.dag.value_type(source) == EVT::Simple(MVT::F32)
                && self.dag.opcode(source) == ISD::Load
                && matches!(
                    self.dag.node(source).data,
                    NodeData::Mem(load) if load.volatile || load.ext.is_some()
                )
            {
                let float = MemInfo { mem_vt: EVT::Simple(MVT::F32), ..info };
                return Some(self.dag.get_store(chain, source, ptr, float));
            }
        }
        // Walk up the chain skipping the memory nodes this store does not
        // alias, on it and on the stores chained below it.
        if let Some(better) = self.find_better_neighbor_chains(node) {
            return Some(better);
        }
        if let Some(split) = self.split_pair_store(node) {
            return Some(split);
        }
        // "Keep trying to merge store sequences until we are unable to do so":
        // the merged stores are dead, the node is returned as handled.
        if self.merge_consecutive_stores(node) {
            return Some(node);
        }
        // DEVIATE SD446: the conversion's only use is this plain store.
        if self.dag.opcode(value) == ISD::FpToSInt
            && !info.volatile
            && info.ordering.is_none()
            && info.ext.is_none()
            && info.mem_vt == self.dag.value_type(value)
            && self.dag.value_type(ptr) != MVT::Far.into()
            && self.dag.has_one_use(value.node)
        {
            let source = self.dag.operand(value, 0);
            if self.hooks.store_fp_to_int_in_memory(self.dag.value_type(source), info.mem_vt) {
                return Some(self.dag.intern(
                    super::x86_lowering::x86isd::FP_TO_INT_IN_MEM,
                    &[EVT::OTHER],
                    &[chain, source, ptr],
                    SDNodeFlags::default(),
                    NodeData::Mem(info),
                ));
            }
        }
        // `replaceStoreOfFPConstant` takes a volatile store too
        // (`isNormalStore`); the hook asks for that.
        let volatile_blocks = info.volatile && !self.hooks.store_fp_constant_as_i64();
        if self.dag.opcode(value) != ISD::ConstantFP || volatile_blocks || info.mem_vt != self.dag.value_type(value) {
            return None;
        }
        let NodeData::ConstFp { bits } = self.dag.node(value).data else { return None };
        let tli = self.tli?;
        let int_info = |mem_vt: EVT, align: u32, base| MemInfo { mem_vt, align, base, ..info };
        match self.dag.value_type(value) {
            EVT::Simple(MVT::F32) => {
                let constant = self.dag.get_constant(bits & 0xFFFF_FFFF, EVT::I32);
                Some(self.dag.get_store(chain, constant, ptr, int_info(EVT::I32, info.align, info.base)))
            }
            EVT::Simple(MVT::F64)
                if self.hooks.store_fp_constant_as_i64()
                    && ((tli.types.is_type_legal(EVT::I64) && !self.legal_operations() && !info.volatile)
                        || tli.is_operation_legal_or_custom(ISD::Store, EVT::I64)) =>
            {
                // `replaceStoreOfFPConstant`: "store double 1.0" is "store i64
                // 0x3FF0...".
                let constant = self.dag.get_constant(bits & 0xFFFF_FFFF_FFFF_FFFF, EVT::I64);
                Some(self.dag.get_store(chain, constant, ptr, int_info(EVT::I64, info.align, info.base)))
            }
            EVT::Simple(MVT::F64) if !tli.is_fp_imm_legal(MVT::F64, bits) => {
                let lo = self.dag.get_constant(bits & 0xFFFF_FFFF, EVT::I32);
                let hi = self.dag.get_constant((bits >> 32) & 0xFFFF_FFFF, EVT::I32);
                let first = self.dag.get_store(chain, lo, ptr, int_info(EVT::I32, info.align, info.base));
                // A far pointer is no integer: its offset word moves, by 4.
                let next = if self.dag.value_type(ptr) == MVT::Far.into() {
                    self.dag.get_object_ptr_offset(ptr, 4)
                } else {
                    let four = self.dag.get_constant(4, self.dag.value_type(ptr));
                    self.dag.get_mem_base_plus_offset(ptr, four, SDNodeFlags::default())
                };
                let second = self
                    .dag
                    .get_store(chain, hi, next, int_info(EVT::I32, info.align.min(4), super::dag::MemBase::Unknown));
                Some(self.dag.get_token_factor(&[first, second]))
            }
            _ => None,
        }
    }

    /// `splitMergedValStore`, for the `BUILD_PAIR` the builder makes before
    /// legalization expands it into the `or`/`shl` the pin matches: a store of
    /// a pair is a store of each half, when the target says that is cheaper
    /// than merging the halves into one register.
    fn split_pair_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.opt_none {
            return None;
        }
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        if self.dag.opcode(value) != ISD::BuildPair
            || info.volatile
            || info.mem_vt != self.dag.value_type(value)
            || !self.dag.has_one_use(value.node)
        {
            return None;
        }
        let (lo, hi) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let (lo_vt, hi_vt) = (self.dag.value_type(lo), self.dag.value_type(hi));
        if !self.tli?.is_multi_stores_cheaper_than_bits_merge(lo_vt, hi_vt) || lo_vt != hi_vt || !lo_vt.is_integer() {
            return None;
        }
        let half = lo_vt.size_in_bits() / 8;
        let part = |mem_vt, align, base| MemInfo { mem_vt, align, base, ..info };
        let first = self.dag.get_store(chain, lo, ptr, part(lo_vt, info.align, info.base));
        let next = if self.dag.value_type(ptr) == MVT::Far.into() {
            self.dag.get_object_ptr_offset(ptr, i64::from(half))
        } else {
            let offset = self.dag.get_constant(u128::from(half), self.dag.value_type(ptr));
            self.dag.get_mem_base_plus_offset(ptr, offset, SDNodeFlags::default())
        };
        Some(self.dag.get_store(first, hi, next, part(hi_vt, info.align.min(half), super::dag::MemBase::Unknown)))
    }

    /// `visitMUL`, up to the multiply-by-power-of-two folds.
    fn visit_mul(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        if self.dag.is_undef(n0) || self.dag.is_undef(n1) {
            return Some(self.dag.get_constant(0, vt));
        }
        let is_const = |dag: &SelectionDAG, v: SDValue| dag.constant(v).is_some() && !dag.is_opaque(v);
        if is_const(self.dag, n0) && !is_const(self.dag, n1) {
            return Some(self.dag.get_node(ISD::Mul, vt, &[n1, n0]));
        }
        let c1 = self.dag.constant(n1);
        let opaque = self.dag.is_opaque(n1);
        let width = vt.size_in_bits();
        let ones = super::fold::mask(width);
        match c1 {
            Some(0) => return Some(n1),
            Some(1) => return Some(n0),
            Some(c) if c == ones => return Some(self.dag.get_negative(n0, vt)),
            _ => {}
        }
        if let (Some(c), false) = (c1, opaque) {
            if c.is_power_of_two() {
                let amount = self.shift_amount(vt, c.trailing_zeros());
                return Some(self.dag.get_node(ISD::Shl, vt, &[n0, amount]));
            }
            // A negated power of two: `0 - (x << log2(-c))`.
            let negated = c.wrapping_neg() & ones;
            if negated.is_power_of_two() && negated != c {
                let amount = self.shift_amount(vt, negated.trailing_zeros());
                let shifted = self.dag.get_node(ISD::Shl, vt, &[n0, amount]);
                let zero = self.dag.get_constant(0, vt);
                return Some(self.dag.get_node(ISD::Sub, vt, &[zero, shifted]));
            }
        }
        // `(mul (shl x, c1), c2)` is `(mul x, c2 << c1)`.
        if self.dag.opcode(n0) == ISD::Shl {
            let (x, c1_shift) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            if let (Some(shift), Some(c)) = (self.dag.constant(c1_shift), c1) {
                if shift < u128::from(width) {
                    let folded = self.dag.get_constant((c << shift) & ones, vt);
                    return Some(self.dag.get_node(ISD::Mul, vt, &[x, folded]));
                }
            }
        }
        // `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)`.
        if self.hooks.fold_mul_of_add_with_constant() && self.dag.opcode(n0) == ISD::Add && !opaque {
            if let (Some(c2), Some(c1)) = (c1, self.dag.constant(self.dag.operand(n0, 1))) {
                if self.mul_add_with_const_profitable(node, n0, vt, c1, c2) {
                    let x = self.dag.operand(n0, 0);
                    let product = self.dag.get_node(ISD::Mul, vt, &[x, n1]);
                    let folded = self.dag.get_constant(c1.wrapping_mul(c2) & ones, vt);
                    return Some(self.dag.get_node(ISD::Add, vt, &[product, folded]));
                }
            }
        }
        self.reassociate_ops(ISD::Mul, n0, n1)
    }

    /// `isMulAddWithConstProfitable`: a one-use add the target accepts, or
    /// another multiply by the same constant that shares `x * c2` or will
    /// after its own fold.
    fn mul_add_with_const_profitable(
        &self,
        mul: SDValue,
        add: SDValue,
        vt: EVT,
        c1: u128,
        c2: u128,
    ) -> bool {
        if self.dag.has_one_use(add.node) && self.hooks.is_mul_add_with_const_profitable(vt, c1, c2) {
            return true;
        }
        let constant = self.dag.operand(mul, 1);
        let var = self.dag.operand(add, 0);
        let users: Vec<u32> = self.dag.users(constant).collect();
        users
            .into_iter()
            .filter(|&user| user != mul.node)
            .any(
                |user| {
                    let user = SDValue { node: user, res: 0 };
                    if self.dag.opcode(user) != ISD::Mul {
                        return false;
                    }
                    let other = if self.dag.operand(user, 0) == constant {
                        self.dag.operand(user, 1)
                    } else {
                        self.dag.operand(user, 0)
                    };
                    other == var
                        || (self.dag.opcode(other) == ISD::Add
                            && self.dag.constant(self.dag.operand(other, 1)).is_some()
                            && self.dag.operand(other, 0) == var)
                },
            )
    }
    /// `visitSIGN_EXTEND_INREG` ([pin][pin]): an operand that is already
    /// sign-extended, nested in-regs, a `sext`/`aext`/`zext` of a value no
    /// wider than the type, a clear sign bit; then the demanded bits.
    /// SD230: not the vector in-reg forms, nor `reduceLoadWidth`.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17400
    fn visit_sext_in_reg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let super::dag::NodeData::ValueType(ext) = self.dag.node(n1).data else { return None };
        let (bits, ext_bits) = (vt.size_in_bits(), ext.size_in_bits());
        if self.dag.is_undef(n0) {
            return Some(self.dag.get_constant(0, vt));
        }
        // already sign extended
        if bits - super::known_bits::num_sign_bits(self.dag, n0) + 1 <= ext_bits {
            return Some(n0);
        }
        if self.dag.opcode(n0) == ISD::SignExtendInReg {
            if let super::dag::NodeData::ValueType(inner) = self.dag.node(self.dag.operand(n0, 1)).data {
                if ext_bits < inner.size_in_bits() {
                    let x = self.dag.operand(n0, 0);
                    return Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[x, n1]));
                }
            }
        }
        let sext_legal = !self.legal_operations() || self.fp_op_legal(ISD::SignExtend, vt);
        let n00 = match self.dag.opcode(n0) {
            ISD::SignExtend | ISD::AnyExtend | ISD::ZeroExtend => self.dag.operand(n0, 0),
            _ => n0,
        };
        match self.dag.opcode(n0) {
            ISD::SignExtend | ISD::AnyExtend if sext_legal => {
                let small = self.dag.value_type(n00).size_in_bits();
                if small <= ext_bits
                    || self.dag.value_type(n00).size_in_bits() - super::known_bits::num_sign_bits(self.dag, n00) + 1
                        <= ext_bits
                {
                    return Some(self.dag.get_node(ISD::SignExtend, vt, &[n00]));
                }
            }
            ISD::ZeroExtend if sext_legal && self.dag.value_type(n00).size_in_bits() == ext_bits => {
                return Some(self.dag.get_node(ISD::SignExtend, vt, &[n00]));
            }
            _ => {}
        }
        // the sign bit is known clear: a zero-extend in register
        let sign = 1u128 << (ext_bits - 1);
        if super::known_bits::known_bits(self.dag, n0).zero & sign != 0 {
            let mask = self.dag.get_constant(super::fold::mask(ext_bits), vt);
            return Some(self.dag.get_node(ISD::And, vt, &[n0, mask]));
        }
        if let Some(simplified) = self.simplify_demanded(node) {
            return Some(simplified);
        }
        self.sext_in_reg_of_load(node, n0, vt, ext)
    }

    /// `visitSIGN_EXTEND_INREG`: `(sext_inreg (extload x))` and
    /// `(sext_inreg (zextload x))` are `(sextload x)`.
    fn sext_in_reg_of_load(
        &mut self,
        node: SDValue,
        load: SDValue,
        vt: EVT,
        ext: EVT,
    ) -> Option<SDValue> {
        if !self.hooks.fold_sext_in_reg_of_load() || self.dag.opcode(load) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        if info.mem_vt != ext || info.volatile {
            return None;
        }
        let legal = match (vt, ext) {
            (EVT::Simple(wide), EVT::Simple(mem)) => {
                self.tli.is_some_and(|tli| tli.load_ext_action(LoadExt::SExtLoad, wide, mem) == Action::Legal)
            }
            _ => false,
        };
        let one_use = self.dag.has_one_use(load.node);
        let fold = match info.ext {
            Some(LoadExt::ExtLoad) => (!self.legal_operations() && one_use) || legal,
            Some(LoadExt::ZExtLoad) => one_use && !self.legal_operations() && legal,
            _ => false,
        };
        if !fold {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let info = super::dag::MemInfo { ext: None, ..info };
        let extended = self.dag.get_ext_load(LoadExt::SExtLoad, vt, chain, ptr, info);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        let _ = node;
        Some(extended)
    }
}

/// One run of the combiner at `level` (`SelectionDAG::Combine`).
pub fn combine<H: CombineHooks>(
    dag: &mut SelectionDAG,
    hooks: &mut H,
    level: CombineLevel,
) {
    DagCombiner::new(dag, hooks, level).run();
}
