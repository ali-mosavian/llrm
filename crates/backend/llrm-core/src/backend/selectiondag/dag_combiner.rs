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
//! single-use simple load only, before operations are legal: no
//! `ExtendUsesToFormExtLoad` (a load with other users) and no
//! `isLoadExtLegal` query (the combiner has no action table). Exit: pass the
//! table in and port `ExtendUsesToFormExtLoad`. Owner sdag-b. Review
//! 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::Run`
//! ([L1845](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1845)),
//! `combine`
//! ([L2157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2157)),
//! `visitMUL`
//! ([L4908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4908)).

use std::collections::HashSet;

use super::actions::{LoadExt, TargetLowering};
use super::dag::{NodeData, Rauw, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;

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
}

/// A target with no combines.
pub struct NoCombines;

impl CombineHooks for NoCombines {}

pub struct DagCombiner<'a, H: CombineHooks> {
    pub(super) dag: &'a mut SelectionDAG,
    hooks: &'a mut H,
    pub(super) level: CombineLevel,
    worklist: Vec<u32>,
    queued: HashSet<u32>,
    /// The action table, for the folds that ask what the target can do.
    pub(super) tli: Option<&'a TargetLowering>,
}

impl<'a, H: CombineHooks> DagCombiner<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        hooks: &'a mut H,
        level: CombineLevel,
    ) -> DagCombiner<'a, H> {
        DagCombiner { dag, hooks, level, worklist: Vec::new(), queued: HashSet::new(), tli: None }
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
        // Oldest first, so the newest is taken first.
        for node in self.dag.node_ids() {
            self.add_to_worklist(node);
        }
        while let Some(n) = self.next() {
            if self.is_dead(n) {
                continue;
            }
            let ops: Vec<u32> = self.dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|op| op.node).collect();
            for op in ops {
                self.add_to_worklist(op);
            }
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
        result
    }

    fn visit(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        match self.dag.opcode(node) {
            ISD::Mul => self.visit_mul(node),
            ISD::And => self.visit_and(node),
            ISD::SDiv => self.visit_sdiv(node),
            ISD::UDiv => self.visit_udiv(node),
            ISD::SRem | ISD::URem => self.visit_rem(node),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => self.visit_extend(node),
            _ => None,
        }
    }

    /// The users of `old` read `new`, and are looked at again.
    fn replace_value(
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

    /// `visitSIGN_EXTEND`, `visitZERO_EXTEND` and `visitANY_EXTEND` for a
    /// loaded operand (`tryToFoldExtOfLoad` and the load case after it): the
    /// extension is the load's own. `(zext (load i8 p))` is `zextload i8 p`;
    /// an already extending load is re-extended to the wider type.
    fn visit_extend(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let load = self.dag.operand(node, 0);
        if self.level >= CombineLevel::AfterLegalizeVectorOps
            || self.dag.opcode(load) != ISD::Load
            || self.dag.users(load).count() != 1
        {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        if info.volatile {
            return None;
        }
        let wanted = match self.dag.opcode(node) {
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
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let info = super::dag::MemInfo { ext: None, space: 0, ..info };
        let wide = self.dag.value_type(node);
        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, info);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        Some(extended)
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
        let all = super::fold::mask(vt.size_in_bits());
        for (other, constant) in [(n0, n1), (n1, n0)] {
            if let Some(c) = self.dag.constant(constant) {
                if self.dag.is_opaque(constant) {
                    continue;
                }
                let known = super::known_bits::known_bits(self.dag, other);
                if all & !(known.zero | c) == 0 {
                    return Some(other);
                }
            }
        }
        None
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
