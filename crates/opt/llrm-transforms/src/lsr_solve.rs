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
//! DEVIATE: `AMK_PostIndexed` (x86 prefers none) is not ported.
//! DEVIATE LU3: see `solve`, the drop of a rewrite that spills (real mode).

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::expr::Scev;
use llrm_support::hash::HashSet;

use crate::lsr_collect::Collection;
use crate::lsr_use::Cost;

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
    ) {
        let lu_idx = workspace.len();
        let lu = &self.uses[lu_idx];
        // A register of the solution so far that this use could reference is
        // required of its formula, which prunes the search.
        let req_regs: Vec<Scev> = cur_regs.iter().copied().filter(|reg| lu.regs.contains(reg)).collect();
        for (f_idx, f) in lu.formulae.iter().enumerate() {
            // Formulae that do not use the required registers before new ones
            // are ignored. If no formula satisfied them LLVM could clear the
            // set and try again; it gives up.
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
            // The cost with this formula: a branch no better than the best is
            // pruned.
            let mut new_cost = cur_cost.clone();
            let mut new_regs = cur_regs.clone();
            self.rate(se, &mut new_cost, f, &mut new_regs, visited, lu_idx, None);
            if new_cost.is_less(self.tti, solution_cost) {
                workspace.push(f_idx);
                if workspace.len() != self.uses.len() {
                    self.solve_recurse(se, solution, solution_cost, workspace, &new_cost, &new_regs, visited);
                    if f.num_regs() == 1 && workspace.len() == 1 {
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
                if to_find != 0 {
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
                llrm_support::debug!("lsr-all", "    use {at} {:?}: {:?}", self.uses[at].kind, self.uses[at].formulae[chosen]);
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
        if llrm_support::debug::enabled("lsr-all") {
            self.trace_candidates(se);
        }
        let mut solution = Vec::new();
        let mut solution_cost = Cost::new();
        solution_cost.lose();
        let mut workspace = Vec::with_capacity(self.uses.len());
        let mut visited = HashSet::default();
        self.solve_recurse(
            se,
            &mut solution,
            &mut solution_cost,
            &mut workspace,
            &Cost::new(),
            &HashSet::default(),
            &mut visited,
        );
        if solution.is_empty() {
            return solution;
        }
        assert_eq!(solution.len(), self.uses.len(), "Malformed solution!");
        llrm_support::debug!("lsr", "chosen {:?} against {:?} as it stands", solution_cost.c, self.baseline_cost.c);
        for (at, &chosen) in solution.iter().enumerate() {
            llrm_support::debug!("lsr", "  use {at} {:?}: {:?}", self.uses[at].kind, self.uses[at].formulae[chosen]);
        }
        // DEVIATE LU3: a rewrite that needs more registers than the loop and
        // than the target has, and costs two instructions more, is dropped.
        let budget = self.tti.number_of_gpr_registers().saturating_sub(1);
        let spills = solution_cost.c.num_regs > budget
            && solution_cost.c.num_regs > self.baseline_cost.c.num_regs
            && solution_cost.c.insns >= self.baseline_cost.c.insns + 2;
        if self.baseline_cost.is_less(self.tti, &solution_cost)
            && (self.tti.should_drop_lsr_solution_if_less_profitable()
                || spills && self.tti.lsr_drops_spilling_solution())
        {
            solution.clear();
        }
        solution
    }
}
