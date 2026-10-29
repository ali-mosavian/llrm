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
//! target that prefers pre-indexed addressing. The `-lsr-preferred-addressing-mode`
//! option (L170) is not either: the target's hook decides.
//! Ported, for `AMK_PostIndexed`: L1445, L1461-1471, L3780, L3988, L4735, L5196,
//! L5609 and `getFixupInsertPos` (L6190, `lsr_implement.rs`; a store keeps the
//! default position, as the pin's `I->getType()` is `void` for it).
//! DEVIATE LU3: see `solve`, the drop of a rewrite that spills (real mode).

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
            // Favoring post-increment, an address use is left to the cost
            // model: the required registers can mislead it.
            if !self.tti.preferred_addressing_mode().has_post_indexed() || lu.kind != Kind::Address {
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

    /// `Solve`: the index of the formula chosen for each use, none if there is
    /// no satisfactory solution or the baseline is better and the target would
    /// drop the solution.
    pub fn solve(
        &self,
        se: &mut ScalarEvolution,
    ) -> Vec<usize> {
        for (at, lu) in self.uses.iter().enumerate() {
            for formula in &lu.formulae {
                llrm_support::debug!("lsr", "  candidate use {at} {:?}: {:?}", lu.kind, formula);
            }
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
