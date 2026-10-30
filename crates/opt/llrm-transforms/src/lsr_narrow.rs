//! How LLVM's loop strength reduction thins the formulae when there are too
//! many for the solver (`NarrowSearchSpaceUsingHeuristics` and the heuristics
//! it runs; LoopStrengthReduce.cpp at `d1106deb71cc`, l.4968-5560).
//!
//! # How it works
//! The search space is the product of the number of formulae of each use. Past
//! the complexity limit (65535, LLVM's), the heuristics run in order, each
//! only if the space is still too big: drop formulae that use a superset of
//! the registers of another, merge uses a constant offset apart (unrolled
//! code), filter again, keep the best formula per scaled register and scale,
//! merge uses outside the loop into those inside, and last pick the register
//! most uses share and drop what does not use it, until the space fits.
//!
//! # Example
//! Four uses of twenty formulae each (160000 combinations) with a register all
//! share: the formulae without it go, one is left per use.
//!
//! # Runs
//! Nothing runs it yet: stage 4 of the translation of LSR.
//!
//! # Limits
//! DEVIATE: `NarrowSearchSpaceByFilterPostInc` (post-indexed addressing, x86
//! prefers none) and `NarrowSearchSpaceByDeletingCostlyFormulas`
//! (`-lsr-exp-narrow`, off by default) are not ported.

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::expr::Scev;
use llrm_support::hash::{HashMap, HashSet};

use crate::lsr_collect::Collection;
use crate::lsr_formula::{Formula, Immediate};
use crate::lsr_use::{Cost, Kind, is_legal_use};

/// `ComplexityLimit` (`-lsr-complexity-limit`).
const COMPLEXITY_LIMIT: usize = u16::MAX as usize;

impl Collection<'_, '_> {
    /// `EstimateSearchSpaceComplexity`: the worst-case number of solutions the
    /// solver might consider, capped at the limit.
    pub fn estimate_search_space_complexity(&self) -> usize {
        let mut power = 1usize;
        for lu in &self.uses {
            let size = lu.formulae.len();
            if size >= COMPLEXITY_LIMIT {
                return COMPLEXITY_LIMIT;
            }
            power *= size;
            if power >= COMPLEXITY_LIMIT {
                break;
            }
        }
        power
    }

    fn too_complex(&self) -> bool {
        self.estimate_search_space_complexity() >= COMPLEXITY_LIMIT
    }

    /// `NarrowSearchSpaceByDetectingSupersets`: a formula with a constant or a
    /// global in a register goes where the use has the same formula with the
    /// value in the immediate or address field.
    pub fn narrow_search_space_by_detecting_supersets(
        &mut self,
        se: &ScalarEvolution,
    ) {
        if !self.too_complex() {
            return;
        }
        for lu_idx in 0..self.uses.len() {
            let mut any = false;
            let mut i = 0;
            while i < self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                let mut deleted = false;
                for (at, &reg) in f.base_regs.iter().enumerate() {
                    let mut new_f = f.clone();
                    if let Some(c) = se.as_constant(reg) {
                        // Formulas do not carry their width (PR41034).
                        new_f.base_offset = Immediate(new_f.base_offset.0.wrapping_add(c.sext_value() as i64));
                    } else if let Some(global) = self.global_of(se, reg) {
                        if f.base_gv.is_some() {
                            continue;
                        }
                        new_f.base_gv = Some(global);
                    } else {
                        continue;
                    }
                    new_f.base_regs.remove(at);
                    if self.uses[lu_idx].has_formula_with_same_regs(&new_f) {
                        self.uses[lu_idx].delete_formula(i);
                        deleted = true;
                        any = true;
                        break;
                    }
                }
                if !deleted {
                    i += 1;
                }
            }
            if any {
                self.uses[lu_idx].recompute_regs(lu_idx, &mut self.reg_uses);
            }
        }
    }

    /// `NarrowSearchSpaceByCollapsingUnrolledCode`: uses a constant offset
    /// apart use the same registers: the fixups of one move into the other.
    pub fn narrow_search_space_by_collapsing_unrolled_code(&mut self) {
        if !self.too_complex() {
            return;
        }
        let mut lu_idx = 0;
        while lu_idx < self.uses.len() {
            let formulae = self.uses[lu_idx].formulae.clone();
            let mut collapsed = false;
            for f in &formulae {
                if f.base_offset.is_zero() || (f.scale != 0 && f.scale != 1) {
                    continue;
                }
                assert!(
                    matches!(self.uses[lu_idx].kind, Kind::Address | Kind::ICmpZero),
                    "Only address and cmp uses expected to have nonzero BaseOffset"
                );
                let Some(that) = self.find_use_with_similar_formula(f, lu_idx) else { continue };
                let (kind, access_ty) = (self.uses[lu_idx].kind, self.uses[lu_idx].access_ty);
                if !self.reconcile_new_offset(that, f.base_offset, false, kind, access_ty) {
                    continue;
                }
                let (outside, unconditional) =
                    (self.uses[lu_idx].all_fixups_outside_loop, self.uses[lu_idx].all_fixups_unconditional);
                self.uses[that].all_fixups_outside_loop &= outside;
                self.uses[that].all_fixups_unconditional &= unconditional;
                // The fixups of this use go to the other.
                for mut fixup in std::mem::take(&mut self.uses[lu_idx].fixups) {
                    fixup.offset = Immediate(fixup.offset.0.wrapping_add(f.base_offset.0));
                    self.uses[that].push_fixup(fixup);
                }
                // The formulae of the other that are no longer legal go.
                let mut any = false;
                let mut i = 0;
                while i < self.uses[that].formulae.len() {
                    let lu = &self.uses[that];
                    if is_legal_use(
                        self.tti,
                        lu.min_offset,
                        lu.max_offset,
                        lu.kind,
                        lu.access_ty.addr_space,
                        lu.formulae[i].base_gv.is_some(),
                        lu.formulae[i].base_offset,
                        lu.formulae[i].has_base_reg,
                        lu.formulae[i].scale,
                    ) {
                        i += 1;
                    } else {
                        self.uses[that].delete_formula(i);
                        any = true;
                    }
                }
                if any {
                    self.uses[that].recompute_regs(that, &mut self.reg_uses);
                }
                // The old use goes.
                self.delete_use(lu_idx);
                collapsed = true;
                break;
            }
            if !collapsed {
                lu_idx += 1;
            }
        }
    }

    /// `NarrowSearchSpaceByRefilteringUndesirableDedicatedRegisters`.
    pub fn narrow_search_space_by_refiltering_undesirable_dedicated_registers(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        if self.too_complex() {
            self.filter_out_undesirable_dedicated_registers(se);
        }
    }

    /// `NarrowSearchSpaceByFilterFormulaWithSameScaledReg`: of the formulae
    /// with one scaled register and scale, the best, so as many pairs as
    /// possible survive.
    pub fn narrow_search_space_by_filter_formula_with_same_scaled_reg(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        if !self.too_complex() {
            return;
        }
        let visited: HashSet<Scev> = HashSet::default();
        let num_uses = self.uses.len();
        for lu_idx in 0..self.uses.len() {
            let mut best_formulae: HashMap<(Scev, i64), usize> = HashMap::default();
            let mut any = false;
            let mut f_idx = 0;
            while f_idx < self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[f_idx].clone();
                let Some(scaled) = f.scaled_reg else {
                    f_idx += 1;
                    continue;
                };
                let Some(&best_idx) = best_formulae.get(&(scaled, f.scale)) else {
                    best_formulae.insert((scaled, f.scale), f_idx);
                    f_idx += 1;
                    continue;
                };
                let best = self.uses[lu_idx].formulae[best_idx].clone();
                // The formula with fewer new registers: one shared among uses
                // adds less to the count.
                let new_regs = |this: &Self, f: &Formula| -> usize {
                    f.base_regs.iter().map(|&reg| num_uses - this.reg_uses.used_by_indices(reg).count() + 1).sum()
                };
                let (fa, fb) = (new_regs(self, &f), new_regs(self, &best));
                let better = if fa != fb {
                    fa < fb
                } else {
                    // The same: the one that costs less.
                    let (mut cost_a, mut cost_b) = (Cost::new(), Cost::new());
                    self.rate(se, &mut cost_a, &f, &mut HashSet::default(), &visited, lu_idx, None);
                    self.rate(se, &mut cost_b, &best, &mut HashSet::default(), &visited, lu_idx, None);
                    cost_a.is_less(self.tti, &cost_b)
                };
                if better {
                    self.uses[lu_idx].formulae.swap(f_idx, best_idx);
                }
                self.uses[lu_idx].delete_formula(f_idx);
                any = true;
            }
            if any {
                self.uses[lu_idx].recompute_regs(lu_idx, &mut self.reg_uses);
            }
        }
    }

    /// `NarrowSearchSpaceByMergingUsesOutsideLoop`: a use with every fixup
    /// outside the loop joins one inside it that has the same first formula.
    pub fn narrow_search_space_by_merging_uses_outside_loop(&mut self) {
        if !self.too_complex() {
            return;
        }
        let mut lu_idx = 0;
        while lu_idx < self.uses.len() {
            let lu = &self.uses[lu_idx];
            // A compare outside the loop is handled specially when expanded.
            if !lu.all_fixups_outside_loop || lu.formulae.is_empty() || lu.kind == Kind::ICmpZero {
                lu_idx += 1;
                continue;
            }
            let this_f = lu.formulae[0].clone();
            let (min, max) = (lu.min_offset, lu.max_offset);
            // A compatible use inside the loop to merge with.
            let target = self
                .uses
                .iter()
                .position(
                    |other| {
                        if other.all_fixups_outside_loop || other.kind == Kind::ICmpZero || other.formulae.is_empty() {
                            return false;
                        }
                        // This use's offsets must be legal for all the other's
                        // formulae.
                        if other.formulae.iter().any(|f| {
                            !is_legal_use(
                                self.tti,
                                min,
                                max,
                                other.kind,
                                other.access_ty.addr_space,
                                f.base_gv.is_some(),
                                f.base_offset,
                                f.has_base_reg,
                                f.scale,
                            )
                        }) {
                            return false;
                        }
                        // The same initial formula; the kind and access type
                        // may differ, which makes the cost inexact but is
                        // what ignoring the uses outside the loop
                        // gives.
                        let other_f = &other.formulae[0];
                        this_f.base_regs == other_f.base_regs
                            && this_f.scaled_reg == other_f.scaled_reg
                            && this_f.base_gv == other_f.base_gv
                            && this_f.scale == other_f.scale
                            && this_f.unfolded_offset == other_f.unfolded_offset
                            && this_f.base_offset == other_f.base_offset
                    },
                );
            let Some(target) = target else {
                lu_idx += 1;
                continue;
            };
            for fixup in std::mem::take(&mut self.uses[lu_idx].fixups) {
                self.uses[target].push_fixup(fixup);
            }
            self.delete_use(lu_idx);
        }
    }

    /// `IsSimplerBaseSCEVForTarget`: whether `best` and `reg` are a constant
    /// apart, and the target takes `+C` in an address where it does not take
    /// `-C`.
    fn is_simpler_base_scev_for_target(
        &self,
        se: &mut ScalarEvolution,
        best: Scev,
        reg: Scev,
        addr_space: u32,
    ) -> bool {
        if se.type_of(best) != se.type_of(reg) {
            return false;
        }
        if let (Some(a), Some(b)) = (se.addrec_header(best), se.addrec_header(reg))
            && a != b
        {
            return false;
        }
        let Some(diff) = se.compute_constant_difference(self.cx, best, reg) else { return false };
        let diff = diff.sext_value() as i64;
        self.tti.is_legal_addressing_mode_in(addr_space, false, diff, true, 0)
            && !self.tti.is_legal_addressing_mode_in(addr_space, false, diff.wrapping_neg(), true, 0)
    }

    /// `NarrowSearchSpaceByPickingWinnerRegs`: with the rest tried, pick the
    /// register most uses share and drop, from each use that has it, the
    /// formulae that do not, until the space fits.
    pub fn narrow_search_space_by_picking_winner_regs(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        let mut taken: HashSet<Scev> = HashSet::default();
        while self.too_complex() {
            // The register used by the most uses is likely a good one to reuse.
            let mut best: Option<(Scev, usize)> = None;
            for &reg in self.reg_uses.registers() {
                if taken.contains(&reg) {
                    continue;
                }
                let count = self.reg_uses.used_by_indices(reg).count();
                let Some((mut current, mut best_count)) = best else {
                    best = Some((reg, count));
                    continue;
                };
                if count > best_count {
                    (current, best_count) = (reg, count);
                }
                // Equal scores: the simpler for the target (`{x,+,1}` over
                // `{x+C,+,1}`, where +C is taken and -C not).
                if count == best_count
                    && let Some(lu_idx) = self.reg_uses.used_by_indices(reg).first()
                    && self.uses[lu_idx].kind == Kind::Address
                    && self.is_simpler_base_scev_for_target(se, current, reg, self.uses[lu_idx].access_ty.addr_space)
                {
                    (current, best_count) = (reg, count);
                }
                best = Some((current, best_count));
            }
            let (best, _) = best.expect("Failed to find best LSRUse candidate");
            taken.insert(best);
            // In each use with a formula that references it, the formulae that
            // do not go.
            for lu_idx in 0..self.uses.len() {
                if !self.uses[lu_idx].regs.contains(&best) {
                    continue;
                }
                let mut any = false;
                let mut i = 0;
                while i < self.uses[lu_idx].formulae.len() {
                    if self.uses[lu_idx].formulae[i].references_reg(best) {
                        i += 1;
                    } else {
                        self.uses[lu_idx].delete_formula(i);
                        any = true;
                        assert!(
                            !self.uses[lu_idx].formulae.is_empty(),
                            "Use has no formulae left! Is Regs inconsistent?"
                        );
                    }
                }
                if any {
                    self.uses[lu_idx].recompute_regs(lu_idx, &mut self.reg_uses);
                }
            }
        }
    }

    /// `NarrowSearchSpaceUsingHeuristics`: thin the formulae with LLVM's rough
    /// heuristics, in its order, so the solver's worst case stays bounded.
    pub fn narrow_search_space_using_heuristics(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        self.narrow_search_space_by_detecting_supersets(se);
        self.narrow_search_space_by_collapsing_unrolled_code();
        self.narrow_search_space_by_refiltering_undesirable_dedicated_registers(se);
        self.narrow_search_space_by_filter_formula_with_same_scaled_reg(se);
        self.narrow_search_space_by_merging_uses_outside_loop();
        self.narrow_search_space_by_picking_winner_regs(se);
    }
}
