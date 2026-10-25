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

use std::collections::HashSet;

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

pub struct DagCombiner<'a, H: CombineHooks> {
    pub(super) dag: &'a mut SelectionDAG,
    pub(super) hooks: &'a mut H,
    pub(super) level: CombineLevel,
    worklist: Vec<u32>,
    queued: HashSet<u32>,
    /// `CombinedNodes`: the nodes already visited, whose operands are not
    /// queued again by a visit of their user.
    combined: HashSet<u32>,
    /// The action table, for the folds that ask what the target can do.
    pub(super) tli: Option<&'a TargetLowering>,
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
            queued: HashSet::new(),
            combined: HashSet::new(),
            tli: None,
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
        self.queued.remove(&node);
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
        (0..results).all(|res| self.dag.users(SDValue { node, res }).next().is_none())
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
                if !self.combined.contains(&op) {
                    self.add_to_worklist(op);
                }
            }
            self.combined.insert(n);
            let before = self.dag.node_count();
            let result = self.combine(SDValue { node: n, res: 0 });
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
            ISD::Select => self.visit_select(node),
            ISD::Store => self.visit_store(node),
            ISD::Freeze => self.visit_freeze(node),
            ISD::SetCC => self.visit_setcc(node),
            ISD::Xor => self.visit_xor(node).or_else(|| self.simplify_demanded(node)),
            ISD::Mul => self.visit_mul(node),
            ISD::And => self.visit_and(node).or_else(|| self.simplify_demanded(node)),
            ISD::Or => self.visit_or(node).or_else(|| self.simplify_demanded(node)),
            ISD::Add => self.visit_add(node),
            ISD::Sub => self.visit_sub(node),
            ISD::Shl | ISD::Srl => self.visit_shift(node).or_else(|| self.simplify_demanded(node)),
            ISD::Sra => self.visit_sra(node).or_else(|| self.visit_shift(node)),
            ISD::SDiv => self.visit_sdiv(node),
            ISD::UDiv => self.visit_udiv(node),
            ISD::SRem | ISD::URem => self.visit_rem(node),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => {
                self.fold_extend_of_cast(node).or_else(|| self.visit_extend(node))
            }
            ISD::Truncate => self.visit_truncate(node).or_else(|| self.simplify_demanded(node)),
            _ => None,
        }
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
        // `(x + c0) == c1` is `x == c1 - c0`, `(x ^ c0) == c1` is `x == c0 ^
        // c1` and `(c0 - x) == c1` is `x == c0 - c1`, for a single-use
        // operand.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && matches!(self.dag.opcode(n0), ISD::Add | ISD::Xor | ISD::Sub)
            && self.dag.use_count(n0.node) == 1
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
        if !matches!(cond, CondCode::SetEq | CondCode::SetNe)
            || self.level != CombineLevel::BeforeLegalizeTypes
            || self.dag.use_count(n0.node) != 1
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
        let arm_vt = self.dag.value_type(n1);
        // `select c0, (select c1, x, y), y` is `select (and c0, c1), x, y`.
        if self.dag.opcode(n1) == ISD::Select && self.dag.use_count(n1.node) == 1 {
            let (c1, x, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1), self.dag.operand(n1, 2));
            if y == n2 && self.dag.value_type(c1) == cond_vt {
                let and = self.dag.get_node(ISD::And, cond_vt, &[n0, c1]);
                return Some(self.dag.get_select(arm_vt, and, x, n2));
            }
        }
        // `select c0, x, (select c1, x, y)` is `select (or c0, c1), x, y`.
        if self.dag.opcode(n2) == ISD::Select && self.dag.use_count(n2.node) == 1 {
            let (c1, x, y) = (self.dag.operand(n2, 0), self.dag.operand(n2, 1), self.dag.operand(n2, 2));
            if x == n1 && self.dag.value_type(c1) == cond_vt {
                let or = self.dag.get_node(ISD::Or, cond_vt, &[n0, c1]);
                return Some(self.dag.get_select(arm_vt, or, n1, y));
            }
        }
        self.select_single_bit_test(n0, n1, n2)
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
        let up = self.dag.get_constant(u128::from(width - 1 - mask.trailing_zeros()), EVT::I8);
        let shl = self.dag.get_node(ISD::Shl, vt, &[x, up]);
        let down = self.dag.get_constant(u128::from(width - 1), EVT::I8);
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
            && self.dag.use_count(n0.node) == 1
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let one_use_setcc =
                |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::SetCC && dag.use_count(v.node) == 1;
            if one_use_setcc(self.dag, a) || one_use_setcc(self.dag, b) {
                let flipped = if self.dag.opcode(n0) == ISD::And { ISD::Or } else { ISD::And };
                let (not_a, not_b) =
                    (self.dag.get_node(ISD::Xor, vt, &[a, n1]), self.dag.get_node(ISD::Xor, vt, &[b, n1]));
                self.add_to_worklist(not_a.node);
                self.add_to_worklist(not_b.node);
                return Some(self.dag.get_node(flipped, vt, &[not_a, not_b]));
            }
        }
        if self.dag.constant(n1) != Some(1) || self.dag.opcode(n0) != ISD::SetCC {
            return None;
        }
        let (lhs, rhs) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        if !self.dag.value_type(lhs).is_integer() {
            return None;
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(n0, 2)).data else { return None };
        Some(self.dag.get_setcc(vt, lhs, rhs, cc.inverse_integer()))
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
        match self.dag.opcode(n0) {
            ISD::Truncate => Some(self.dag.get_node(ISD::Truncate, vt, &[self.dag.operand(n0, 0)])),
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
        if let (Some(c1), true) = (self.dag.constant(n1), !self.dag.is_opaque(n1)) {
            // `reassociateOps`: `(and (and x, c0), c1)` is `(and x, c0 & c1)`.
            if self.dag.opcode(n0) == ISD::And {
                let (x, inner) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                if self.dag.constant(inner).is_some() && !self.dag.is_opaque(inner) {
                    let both = self.dag.get_node(ISD::And, vt, &[inner, n1]);
                    return Some(self.dag.get_node(ISD::And, vt, &[x, both]));
                }
            }
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

    /// `visitADD` through `visitADDLike`, the folds that need no target
    /// query: a constant moves right, `x + ~x` is -1, constants merge through
    /// a `sub` or an `add`, and an `add` of a negation or of a `sub` of the
    /// other operand is a `sub` or the operand itself.
    ///
    /// DEVIATE SD210: an `add` of constants is not reassociated when it feeds
    /// a memory access (`reassociationCanBreakAddressingModePattern` is not
    /// translated; the matcher folds the displacement there). Exit: that
    /// query. Owner sdag-b. Review 2026-12-10.
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
            // `(add (add x, c0), c1)` is `(add x, c0 + c1)`.
            if self.dag.opcode(n0) == ISD::Add && plain(self.dag, self.dag.operand(n0, 1)) {
                let feeds_memory = self
                    .dag
                    .users(node)
                    .any(|user| matches!(
                        self.dag.opcode(SDValue { node: user, res: 0 }),
                        ISD::Load | ISD::Store
                    ));
                if !feeds_memory {
                    let sum = self.dag.get_node(ISD::Add, vt, &[n1, self.dag.operand(n0, 1)]);
                    return Some(self.dag.get_node(ISD::Add, vt, &[self.dag.operand(n0, 0), sum]));
                }
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
        if self.dag.use_count(trunc.node) == 1
            && self.dag.use_count(inner.node) == 1
            && c1 + c2 < inner_size
            && c2 < op_size
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
            return None;
        }
        // The users now read `x` directly: their users may fold with it too
        // (`not (or (cmp), freeze (cmp))` was visited with the freeze there).
        let users: Vec<u32> = self.dag.users(node).collect();
        for user in users {
            self.add_to_worklist_with_users(user);
        }
        Some(x)
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
        let c = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1))?;
        let negated = self.dag.get_constant(c.wrapping_neg() & super::fold::mask(vt.size_in_bits()), vt);
        Some(self.dag.get_node(ISD::Add, vt, &[n0, negated]))
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
        if self.dag.opcode(value) != ISD::ConstantFP || info.volatile || info.mem_vt != self.dag.value_type(value) {
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
        let shift_vt = EVT::I8;
        if let (Some(c), false) = (c1, opaque) {
            if c.is_power_of_two() {
                let amount = self.dag.get_constant(u128::from(c.trailing_zeros()), shift_vt);
                return Some(self.dag.get_node(ISD::Shl, vt, &[n0, amount]));
            }
            // A negated power of two: `0 - (x << log2(-c))`.
            let negated = c.wrapping_neg() & ones;
            if negated.is_power_of_two() && negated != c {
                let amount = self.dag.get_constant(u128::from(negated.trailing_zeros()), shift_vt);
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
        None
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
