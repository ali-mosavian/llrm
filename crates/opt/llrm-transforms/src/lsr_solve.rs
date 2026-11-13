//! The solver of LLVM's loop strength reduction: one formula per use, by a
//! depth-first search pruned by the best cost found (`SolveRecurse` and
//! `Solve`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5573-5710).
//!
//! # How it works
//! Uses are decided in order. A formula of a use must reference every register
//! already chosen that the use could also reference (up to its register count),
//! which prunes most of the search; its cost is rated on top of the cost so
//! far, and the branch is dropped when it is no better than the best solution.
//! A formula of the first use that is a single register is remembered as
//! visited: formulae later in the search that name it lose.
//!
//! # Example
//! The array walk and its exit test: the store keeps `{%p,+,4}`, the compare
//! keeps `{(-1 + %n),+,-1}`, one register each, as `opt -passes=loop-reduce`
//! has them.
//!
//! # Runs
//! Nothing runs it yet: stage 5 of the translation of LSR.
//!
//! # Limits
//! DEVIATE: `AMK_PreIndexed` is not ported, and so a loop that wants `-(An)`
//! (a pointer stepped down before each access) stays a step and an access.
//! The pin sites: `RateRegister`'s `CanPreIndex`
//! ([L1468](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1468))
//! and the offset-minus-step formulae of `GenerateConstantOffsetsImpl`
//! ([L4230](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4230)).
//! No target of ours prefers it (`getPreferredAddressingMode` is `PostIndexed`
//! or `None`), and the overlay has no `-(An)` fold either. Exit: with the first
//! target that prefers pre-indexed addressing. The
//! `-lsr-preferred-addressing-mode` option (L170) is not either: the target's
//! hook decides. Ported, for `AMK_PostIndexed`: L1445, L1461-1471, L3780,
//! L3988, L4735, L5196, L5609 and `getFixupInsertPos` (L6190,
//! `lsr_implement.rs`; a store keeps the default position, as the pin's
//! `I->getType()` is `void` for it). DEVIATE LU3: see `solve`, the drop of a
//! rewrite that spills (real mode).
//!
//! DEVIATE LU3b: the registers the loop keeps live beside its recurrences (the
//! values its body defines that SCEV does not make a recurrence, in registers
//! of the target's width, `Tti::gpr_bits`) are taken off the budget the solver
//! and the spill check count against; the pin counts none. Without it shellsort
//! bas -Os spent a third formula register to save an instruction and the
//! allocator spilled the `i32` chain (+6 B). No pin run can decide it: the pin
//! has no m16 TTI. Where no solution fits the registers left, the cut is
//! dropped and the search runs again without it: every one spills and the count
//! no longer tells them apart (stft_fixed c -Os, +11 B). Exit: a pressure model
//! on the pin's MachineLICM/RegisterPressure terms, by 2026-11-20, or when the
//! allocator's spill cost is priced in the comparator.

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::expr::Scev;
use llrm_support::hash::HashSet;

use crate::lsr_collect::Collection;
use crate::lsr_use::{Cost, Kind};

impl Collection<'_, '_> {
    /// `SolveRecurse`: decide use `workspace.len()`, then the rest.
    #[allow(clippy::too_many_arguments)]
    fn solve_recurse(
        &self,
        se: &mut ScalarEvolution,
        solution: &mut Vec<usize>,
        solution_cost: &mut Cost,
        workspace: &mut Vec<usize>,
        cur_cost: &Cost,
        cur_regs: &HashSet<Scev>,
        visited: &mut HashSet<Scev>,
        order: &[usize],
    ) {
        let lu_idx = order[workspace.len()];
        let lu = &self.uses[lu_idx];
        // A register of the solution so far that this use could reference is
        // required of its formula, which prunes the search.
        let req_regs: Vec<Scev> = cur_regs.iter().copied().filter(|reg| lu.regs.contains(reg)).collect();
        for (f_idx, f) in lu.formulae.iter().enumerate() {
            // Formulae that do not use the required registers before new ones
            // are ignored. If no formula satisfied them LLVM could clear the
            // set and try again; it gives up.
            // Favoring post-increment, an address use is left to the cost
            // model: the required registers can mislead it.
            // DEVIATE LU24: so is an address use that runs every trip where
            // the target says its base add costs more than a recurrence (`gep
            // p, n` against a pointer that steps): a pointer that steps costs
            // an add on trips that do not use it.
            if !(self.tti.preferred_addressing_mode().has_post_indexed()
                || self.tti.lsr_address_left_to_cost_model() && lu.all_fixups_unconditional)
                || lu.kind != Kind::Address
            {
                let mut to_find = f.num_regs().min(req_regs.len());
                for reg in &req_regs {
                    if f.scaled_reg == Some(*reg) || f.base_regs.contains(reg) {
                        to_find -= 1;
                        if to_find == 0 {
                            break;
                        }
                    }
                }
                if to_find != 0 {
                    continue;
                }
            }
            // The cost with this formula: a branch no better than the best is
            // pruned.
            let mut new_cost = cur_cost.clone();
            let mut new_regs = cur_regs.clone();
            self.rate(se, &mut new_cost, f, &mut new_regs, visited, lu_idx, None);
            if new_cost.is_less(self.tti, solution_cost) {
                workspace.push(f_idx);
                if workspace.len() != self.uses.len() {
                    self.solve_recurse(se, solution, solution_cost, workspace, &new_cost, &new_regs, visited, order);
                    if self.tti.lsr_visited_registers_lose() && f.num_regs() == 1 && workspace.len() == 1 {
                        visited.insert(f.scaled_reg.unwrap_or_else(|| f.base_regs[0]));
                    }
                } else {
                    *solution_cost = new_cost.clone();
                    *solution = workspace.clone();
                }
                workspace.pop();
            }
        }
    }

    /// Every complete choice of one formula per use, rated as `solve_recurse`
    /// does but never pruned by the best: the candidates `lsr-all` prints,
    /// cheapest registers first. Debugging only.
    fn trace_candidates(
        &self,
        se: &mut ScalarEvolution,
    ) -> Vec<(Cost, Vec<usize>)> {
        #[allow(clippy::too_many_arguments)]
        fn walk(
            this: &Collection<'_, '_>,
            se: &mut ScalarEvolution,
            workspace: &mut Vec<usize>,
            cur_cost: &Cost,
            cur_regs: &HashSet<Scev>,
            visited: &mut HashSet<Scev>,
            found: &mut Vec<(Cost, Vec<usize>)>,
        ) {
            if found.len() >= 20_000 {
                return;
            }
            let lu_idx = workspace.len();
            let lu = &this.uses[lu_idx];
            let req_regs: Vec<Scev> = cur_regs.iter().copied().filter(|reg| lu.regs.contains(reg)).collect();
            for (f_idx, f) in lu.formulae.iter().enumerate() {
                let mut to_find = f.num_regs().min(req_regs.len());
                for reg in &req_regs {
                    if f.scaled_reg == Some(*reg) || f.base_regs.contains(reg) {
                        to_find -= 1;
                        if to_find == 0 {
                            break;
                        }
                    }
                }
                if to_find != 0
                    && !(lu.kind == Kind::Address
                        && lu.all_fixups_unconditional
                        && this.tti.lsr_address_left_to_cost_model())
                {
                    continue;
                }
                let mut new_cost = cur_cost.clone();
                let mut new_regs = cur_regs.clone();
                this.rate(se, &mut new_cost, f, &mut new_regs, visited, lu_idx, None);
                if new_cost.is_loser() {
                    continue;
                }
                workspace.push(f_idx);
                if workspace.len() == this.uses.len() {
                    found.push((new_cost, workspace.clone()));
                } else {
                    walk(this, se, workspace, &new_cost, &new_regs, visited, found);
                }
                workspace.pop();
            }
        }
        let mut found = Vec::new();
        walk(self, se, &mut Vec::new(), &Cost::new(), &HashSet::default(), &mut HashSet::default(), &mut found);
        found.sort_by_key(|(cost, _)| (cost.c.num_regs, cost.c.insns, cost.c.setup_cost));
        llrm_support::debug!("lsr-all", "{} candidates, baseline {:?}", found.len(), self.baseline_cost.c);
        let mut shown = 0;
        let mut last = None;
        for (cost, choice) in &found {
            if last.as_ref() == Some(&cost.c) {
                continue;
            }
            last = Some(cost.c.clone());
            if shown == 12 {
                break;
            }
            shown += 1;
            llrm_support::debug!("lsr-all", "  {:?}", cost.c);
            for (at, &chosen) in choice.iter().enumerate() {
                llrm_support::debug!(
                    "lsr-all",
                    "    use {at} {:?}: {:?}",
                    self.uses[at].kind,
                    self.uses[at].formulae[chosen]
                );
            }
        }
        found
    }

    /// `Solve`: the index of the formula chosen for each use, none if there is
    /// no satisfactory solution or the baseline is better and the target would
    /// drop the solution.
    pub fn solve(
        &self,
        se: &mut ScalarEvolution,
    ) -> Vec<usize> {
        self.solve_with_cost(se).0
    }

    /// One `SolveRecurse` from the empty solution with the uses taken in
    /// `order`: the formula chosen for each use (by use, empty if none) and
    /// what it costs.
    fn search(
        &self,
        se: &mut ScalarEvolution,
        order: &[usize],
    ) -> (Vec<usize>, Cost) {
        let mut by_position = Vec::new();
        let mut solution_cost = Cost::new();
        solution_cost.lose();
        let mut workspace = Vec::with_capacity(self.uses.len());
        let mut visited = HashSet::default();
        self.solve_recurse(
            se,
            &mut by_position,
            &mut solution_cost,
            &mut workspace,
            &Cost::new(),
            &HashSet::default(),
            &mut visited,
            order,
        );
        let mut by_use = vec![0; by_position.len()];
        for (position, &formula) in by_position.iter().enumerate() {
            by_use[order[position]] = formula;
        }
        (by_use, solution_cost)
    }

    /// `solve`, and what the solution it returns costs (nothing to compare for
    /// the empty one).
    pub fn solve_with_cost(
        &self,
        se: &mut ScalarEvolution,
    ) -> (Vec<usize>, Cost) {
        if llrm_support::debug::enabled("lsr-all") {
            self.trace_candidates(se);
        }
        for (at, lu) in self.uses.iter().enumerate() {
            for formula in &lu.formulae {
                llrm_support::debug!("lsr", "  candidate use {at} {:?}: {:?}", lu.kind, formula);
            }
        }
        let forward: Vec<usize> = (0..self.uses.len()).collect();
        let reversed: Vec<usize> = forward.iter().rev().copied().collect();
        let mut solution = Vec::new();
        let mut solution_cost;
        loop {
            (solution, solution_cost) = self.search(se, &forward);
            // DEVIATE LU14: `SolveRecurse` requires of a use the registers the
            // uses before it chose, so a cheaper combination that starts with
            // a later use is never rated (rcflip's RAMP loop: 4 instructions
            // against 2 with the uses the other way round). The uses are also taken
            // in the opposite order, and its solution kept where it costs less
            // by the target's comparator; a tie keeps the pin's order. A
            // target opts in (`lsr_solves_both_use_orders`). Cost: one more
            // search per instance.
            if self.uses.len() > 1 && !solution.is_empty() && self.tti.lsr_solves_both_use_orders() {
                let (other, other_cost) = self.search(se, &reversed);
                // It must save an instruction over the first order's solution
                // and over the loop as written: a tie in instructions is
                // settled by the cost's later terms (`AddRecCost`, immediates),
                // which rate a rewrite better that the code is not (m16 recpow
                // +10 B, matmul +64 instructions).
                if !other.is_empty()
                    && other_cost.is_less(self.tti, &solution_cost)
                    && other_cost.c.insns < solution_cost.c.insns
                    && other_cost.c.insns < self.baseline_cost.c.insns
                {
                    solution = other;
                    solution_cost = other_cost;
                }
            }
            // DEVIATE LU3b: where the loop's other values leave less than a
            // base and an index, the cut tells no solution from
            // another; search again without it.
            let budget = self.tti.number_of_gpr_registers().saturating_sub(1).saturating_sub(self.pressure_units());
            if solution.is_empty() || budget >= 2 || solution_cost.c.num_regs <= budget || self.pressure_units() == 0 {
                break;
            }
            self.ignore_pressure.set(true);
        }
        if solution.is_empty() {
            return (solution, solution_cost);
        }
        assert_eq!(solution.len(), self.uses.len(), "Malformed solution!");
        llrm_support::debug!("lsr", "chosen {:?} against {:?} as it stands", solution_cost.c, self.baseline_cost.c);
        for (at, &chosen) in solution.iter().enumerate() {
            llrm_support::debug!("lsr", "  use {at} {:?}: {:?}", self.uses[at].kind, self.uses[at].formulae[chosen]);
        }
        // DEVIATE LU3: a rewrite that needs more registers than the loop and
        // than the target has, and costs two instructions more, is dropped.
        let budget = self.tti.number_of_gpr_registers().saturating_sub(1).saturating_sub(self.pressure_units());
        let spills = solution_cost.c.num_regs > budget
            && solution_cost.c.num_regs > self.baseline_cost.c.num_regs
            && solution_cost.c.insns >= self.baseline_cost.c.insns + 2;
        // DEVIATE LU15: a loop with an address keeps the solution that
        // reduces it, where the target says so.
        let spared = self.tti.lsr_dropped_solution_spares_addresses() && self.uses.iter().any(|u| u.kind == Kind::Address);
        if self.baseline_cost.is_less(self.tti, &solution_cost)
            && (self.tti.should_drop_lsr_solution_if_less_profitable() && !spared
                || spills && self.tti.lsr_drops_spilling_solution())
        {
            solution.clear();
        }
        (solution, solution_cost)
    }
}
