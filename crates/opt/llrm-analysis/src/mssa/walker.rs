//! The clobber walker of `MemorySSA.cpp`: `ClobberWalker`, `ClobberWalkerBase`,
//! `CachingWalker` and `SkipSelfWalker`, with `upward_defs_iterator` from
//! `MemorySSA.h`. Caps stand at their `cl::opt` defaults.
//!
//! DEVIATE: no `invariant.group` (MIR has none:
//! `getInvariantGroupClobberingInstruction` is omitted), no fence-like
//! instructions (MIR has no fence opcode), and no `PHITransAddr`: a
//! location crossing a phi keeps its pointer and, unless the pointer is
//! guaranteed loop invariant, its size becomes unknown, which is LLVM's own
//! result when the translation fails.

use std::rc::Rc;

use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::HashSet;

use super::alias::{AliasQuery, Location};
use super::{AccessId, Kind, LIVE_ON_ENTRY, Mssa};
use crate::cfg;

/// [`MaxCheckLimit`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L76),
/// `memssa-check-limit`.
pub const MAX_CHECK_LIMIT: u32 = 100;

/// [`UpwardDefsElem`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L1185).
#[derive(Clone, Debug)]
pub struct UpwardDefsElem {
    pub access: AccessId,
    pub loc: Option<Rc<Location>>,
    pub may_be_cross_iteration: bool,
}

impl UpwardDefsElem {
    fn key(&self) -> (AccessId, Option<u32>, bool) {
        (self.access, self.loc.as_ref().map(|loc| loc.id), self.may_be_cross_iteration)
    }
}

/// [`UpwardsMemoryQuery`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L340).
#[derive(Clone)]
pub struct UpwardsMemoryQuery {
    /// True if our original query started off as a call.
    pub is_call: bool,
    /// The location the query started with: none when `is_call`.
    pub starting_loc: Option<Rc<Location>>,
    /// The instruction we were querying about.
    pub inst: Option<InstId>,
    /// The access we actually got called with, used to test local domination.
    pub original_access: Option<AccessId>,
    pub skip_self_access: bool,
}

impl UpwardsMemoryQuery {
    fn of(
        aa: &AliasQuery,
        inst: InstId,
        access: AccessId,
    ) -> Self {
        let is_call = matches!(
            aa.unit().function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        );
        Self {
            is_call,
            starting_loc: if is_call { None } else { aa.location_of(inst) },
            inst: Some(inst),
            original_access: Some(access),
            skip_self_access: false,
        }
    }
}

/// [`instructionClobbersQuery`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L275):
/// whether the def `md` may clobber the location `use_loc` of `use_inst`.
pub fn instruction_clobbers_query(
    mssa: &Mssa,
    aa: &AliasQuery,
    md: AccessId,
    use_loc: Option<&Location>,
    use_inst: Option<InstId>,
) -> bool {
    let def_inst = mssa.access(md).inst.expect("defining instruction not actually an instruction");
    let function = aa.unit().function;
    // These intrinsics show up as affecting memory, but they are markers
    // (the `allow.*check`, `noalias.scope.decl` and `pseudoprobe` ones of the
    // switch are not in MIR).
    if matches!(
        aa.unit().intrinsic(def_inst),
        Some(
            llrm_mir::intrinsics::Intrinsic::InvariantStart
                | llrm_mir::intrinsics::Intrinsic::InvariantEnd
                | llrm_mir::intrinsics::Intrinsic::Assume
        )
    ) {
        return false;
    }
    if let Some(use_inst) = use_inst {
        if matches!(
            function.instruction(use_inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        ) {
            return aa.mod_set_call(def_inst, use_inst);
        }
        if matches!(function.instruction(def_inst).opcode, Opcode::Load { .. })
            && matches!(function.instruction(use_inst).opcode, Opcode::Load { .. })
        {
            return !aa.are_loads_reorderable(use_inst, def_inst);
        }
    }
    match use_loc {
        Some(loc) => aa.mod_set_loc(def_inst, loc),
        // Without a location the def is assumed to clobber.
        None => true,
    }
}

/// [`upward_defs_iterator::IsGuaranteedLoopInvariant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2642):
/// the pointer refers to one location during the whole function: a base that is
/// not an instruction or an alloca, or an instruction in the entry block, with
/// constant indices.
fn is_guaranteed_loop_invariant(
    aa: &AliasQuery,
    loc: &Location,
) -> bool {
    use llrm_mir::module::{Operand, ValueDef};
    let function = aa.unit().function;
    let invariant_base = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(def) => matches!(function.instruction(def).opcode, Opcode::Alloca { .. }),
        },
        _ => true,
    };
    let in_entry = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def) == function.entry(),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    };
    let reference = &loc.reference;
    if reference.pointer.is_some_and(in_entry) {
        return true;
    }
    // A pointer made with a variable index is not invariant.
    if reference.base.is_some() {
        return false;
    }
    reference.root.is_none_or(invariant_base)
}

/// [`upward_defs_begin`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L1226):
/// the defs one step above `elem`, with the location carried across a phi, and
/// the phi's argument block.
fn upward_defs(
    mssa: &Mssa,
    aa: &AliasQuery,
    elem: &UpwardDefsElem,
) -> Vec<(UpwardDefsElem, Option<BlockId>)> {
    let access = mssa.access(elem.access);
    let walking_phi = access.kind == Kind::Phi;
    let step = |above: AccessId, block: Option<BlockId>| {
        let mut out = UpwardDefsElem {
            access: above,
            loc: elem.loc.clone(),
            may_be_cross_iteration: elem.may_be_cross_iteration,
        };
        // No need for phi translation or handling of cross-iteration
        // dependences if we're not walking past a phi.
        if walking_phi {
            match &elem.loc {
                // Mark size as unknown, if the location is not guaranteed to be
                // loop-invariant for any possible loop in the function.
                Some(loc) if !is_guaranteed_loop_invariant(aa, loc) => out.loc = Some(aa.widened(loc)),
                Some(_) => {}
                // We can't easily analyze invariance for calls, so
                // conservatively assume they may be introducing cross-iteration
                // dependences for any phi translation.
                None => out.may_be_cross_iteration = true,
            }
        }
        (out, block)
    };
    if walking_phi {
        access.incoming.iter().map(|&(block, value)| step(value, Some(block))).collect()
    } else {
        access.defining.into_iter().map(|above| step(above, None)).collect()
    }
}

/// `def_chain`: `start`, then the defining access of each def, stopping at a
/// phi and at live on entry.
fn defining_of(
    mssa: &Mssa,
    access: AccessId,
) -> Option<AccessId> {
    let one = mssa.access(access);
    if one.kind == Kind::Def { one.defining } else { None }
}

#[derive(Clone)]
struct DefPath {
    loc: Option<Rc<Location>>,
    /// Because we always walk in reverse, `last` is where the walk has reached
    /// (`First` of the C++ is never read and is not kept).
    last: AccessId,
    previous: Option<usize>,
    may_be_cross_iteration: bool,
}

struct UpwardsWalkResult {
    result: AccessId,
    is_known_clobber: bool,
}

#[derive(Clone, Copy)]
struct TerminatedPath {
    clobber: AccessId,
    last_node: usize,
}

struct OptznResult {
    primary_clobber: TerminatedPath,
    other_clobbers: Vec<TerminatedPath>,
}

/// [`ClobberWalker`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L484).
struct ClobberWalker<'m, 'a> {
    mssa: &'m Mssa,
    aa: &'m AliasQuery<'a>,
    query: UpwardsMemoryQuery,
    upward_walk_limit: u32,
    /// Phi optimization bookkeeping.
    paths: Vec<DefPath>,
    visited_phis: HashSet<(AccessId, Option<u32>, bool)>,
}

impl<'m, 'a> ClobberWalker<'m, 'a> {
    fn clobbers(
        &self,
        md: AccessId,
        loc: Option<&Location>,
    ) -> bool {
        instruction_clobbers_query(self.mssa, self.aa, md, loc, self.query.inst)
    }

    /// `getWalkTarget`: the nearest def or phi that `from` can legally be
    /// optimized to.
    fn get_walk_target(
        &self,
        from: AccessId,
    ) -> AccessId {
        assert!(!self.mssa.access(from).incoming.is_empty(), "phi with no operands?");
        let mut node = Some(cfg::id(self.mssa.access(from).block));
        while let Some(at) = node.and_then(|at| self.mssa.dominance.immediate_resolved(at)) {
            if let Some(defs) = self.mssa.get_block_defs(cfg::block(at)) {
                return *defs.last().expect("a non-empty defs list");
            }
            node = Some(at);
        }
        LIVE_ON_ENTRY
    }

    /// `walkToPhiOrClobber`: walk to the next phi or clobber in the def chain
    /// starting at `desc.last`, updating `desc.last`, optionally stopping
    /// at `stop_at` or `skip_stop_at`.
    fn walk_to_phi_or_clobber(
        &mut self,
        desc: &mut DefPath,
        stop_at: Option<AccessId>,
        skip_stop_at: Option<AccessId>,
    ) -> UpwardsWalkResult {
        let mut limit_already_reached = false;
        // `upward_walk_limit` may be 0 here, due to the loop in tryOptimizePhi.
        // Set it to 1: this does no alias calls; it either returns in
        // the first iteration or is set back to 0 if all def chains are
        // free of MemoryDefs.
        if self.upward_walk_limit == 0 {
            self.upward_walk_limit = 1;
            limit_already_reached = true;
        }
        let mut current = Some(desc.last);
        while let Some(at) = current {
            desc.last = at;
            if Some(at) == stop_at || Some(at) == skip_stop_at {
                return UpwardsWalkResult { result: at, is_known_clobber: false };
            }
            if self.mssa.access(at).kind == Kind::Def {
                if self.mssa.is_live_on_entry_def(at) {
                    return UpwardsWalkResult { result: at, is_known_clobber: true };
                }
                self.upward_walk_limit -= 1;
                if self.upward_walk_limit == 0 {
                    return UpwardsWalkResult { result: at, is_known_clobber: true };
                }
                if self.clobbers(at, desc.loc.as_deref()) {
                    return UpwardsWalkResult { result: at, is_known_clobber: true };
                }
            }
            current = defining_of(self.mssa, at);
        }
        if limit_already_reached {
            self.upward_walk_limit = 0;
        }
        assert!(self.mssa.access(desc.last).kind == Kind::Phi, "ended at a non-clobber that's not a phi?");
        UpwardsWalkResult { result: desc.last, is_known_clobber: false }
    }

    /// `addSearches`.
    fn add_searches(
        &mut self,
        phi: AccessId,
        paused_searches: &mut Vec<usize>,
        prior_node: usize,
        may_be_cross_iteration: bool,
    ) {
        let elem = UpwardDefsElem { access: phi, loc: self.paths[prior_node].loc.clone(), may_be_cross_iteration };
        for (e, _) in upward_defs(self.mssa, self.aa, &elem) {
            paused_searches.push(self.paths.len());
            self.paths.push(DefPath {
                loc: e.loc,
                last: e.access,
                previous: Some(prior_node),
                may_be_cross_iteration: e.may_be_cross_iteration,
            });
        }
    }

    /// `getBlockingAccess`: an access that keeps us from optimizing to
    /// `stop_where`.
    fn get_blocking_access(
        &mut self,
        stop_where: AccessId,
        paused_searches: &mut Vec<usize>,
        new_paused: &mut Vec<usize>,
        terminated: &mut Vec<TerminatedPath>,
    ) -> Option<TerminatedPath> {
        assert!(!paused_searches.is_empty(), "no searches to continue?");
        // BFS vs DFS really doesn't make a difference here, so just do a DFS
        // with `paused_searches` as our stack.
        while let Some(path_index) = paused_searches.pop() {
            let mut node = self.paths[path_index].clone();
            // If we've already visited this path with this location, we don't
            // need to do so again.
            if !self.visited_phis.insert((node.last, node.loc.as_ref().map(|loc| loc.id), node.may_be_cross_iteration))
            {
                continue;
            }
            let skip_stop_where = if self.query.skip_self_access
                && match (&node.loc, &self.query.starting_loc) {
                    (Some(a), Some(b)) => a.id == b.id,
                    (None, None) => true,
                    _ => false,
                } {
                assert!(self.mssa.access(self.query.original_access.expect("an access")).kind == Kind::Def);
                self.query.original_access
            } else {
                None
            };
            let res = self.walk_to_phi_or_clobber(&mut node, Some(stop_where), skip_stop_where);
            self.paths[path_index] = node.clone();
            if res.is_known_clobber {
                assert!(Some(res.result) != Some(stop_where) && Some(res.result) != skip_stop_where);
                // If this wasn't a cache hit, we hit a clobber when walking.
                // That's a failure.
                let term = TerminatedPath { clobber: res.result, last_node: path_index };
                if !self.mssa.dominates(res.result, stop_where) {
                    return Some(term);
                }
                // Otherwise, it's a valid thing to potentially optimize to.
                terminated.push(term);
                continue;
            }
            if res.result == stop_where || Some(res.result) == skip_stop_where {
                // We've hit our target. Save this path off for if we want to
                // continue walking. If we are skipping the
                // original access and have reached back to it, do not save the
                // path: we've just looped back to self.
                if Some(res.result) != skip_stop_where {
                    new_paused.push(path_index);
                }
                continue;
            }
            assert!(!self.mssa.is_live_on_entry_def(res.result), "liveOnEntry is a clobber");
            self.add_searches(res.result, paused_searches, path_index, node.may_be_cross_iteration);
        }
        None
    }

    /// Moves the `TerminatedPath` with the 'most dominated' clobber to the end.
    fn move_dominated_path_to_end(
        &self,
        paths: &mut [TerminatedPath],
    ) {
        assert!(!paths.is_empty(), "need a path to move");
        let mut dom = 0;
        for i in 1..paths.len() {
            if !self.mssa.dominates(paths[i].clobber, paths[dom].clobber) {
                dom = i;
            }
        }
        let last = paths.len() - 1;
        if last != dom {
            paths.swap(last, dom);
        }
    }

    /// `tryOptimizePhi`: try to optimize a phi as best as we can.
    fn try_optimize_phi(
        &mut self,
        phi: AccessId,
        loc: Option<Rc<Location>>,
    ) -> OptznResult {
        assert!(self.paths.is_empty() && self.visited_phis.is_empty(), "reset the optimization state");
        self.paths.push(DefPath { loc, last: phi, previous: None, may_be_cross_iteration: false });
        // How many 'valid' optimization nodes we had prior to calling
        // addSearches / getBlockingAccess. Necessary for caching if we
        // had a blocker.
        let mut prior_paths_size = self.paths.len();
        let mut paused_searches: Vec<usize> = Vec::new();
        let mut new_paused: Vec<usize> = Vec::new();
        let mut terminated_paths: Vec<TerminatedPath> = Vec::new();
        self.add_searches(phi, &mut paused_searches, 0, false);
        let mut current = phi;
        loop {
            assert!(!self.mssa.is_live_on_entry_def(current), "liveOnEntry wasn't treated as a clobber?");
            let target = self.get_walk_target(current);
            // If a TerminatedPath doesn't dominate the target, then it wasn't a
            // legal optimization for the prior phi.
            debug_assert!(terminated_paths.iter().all(|p| self.mssa.dominates(p.clobber, target)));
            if let Some(blocker) =
                self.get_blocking_access(target, &mut paused_searches, &mut new_paused, &mut terminated_paths)
            {
                // Find the node we started at. We can't search based on `last`,
                // since we may have gone around a loop with a
                // different location.
                let mut index = Some(blocker.last_node);
                let found = loop {
                    let at = index.expect("a path from the blocker to a node before the search");
                    if at < prior_paths_size {
                        break at;
                    }
                    index = self.paths[at].previous;
                };
                let cur_node = &self.paths[found];
                assert!(cur_node.last == current);
                return OptznResult {
                    primary_clobber: TerminatedPath { clobber: cur_node.last, last_node: found },
                    other_clobbers: Vec::new(),
                };
            }
            // If there's nothing left to search, then all paths led to valid
            // clobbers that we got from our cache; pick the nearest
            // to the start, and allow the rest to be cached back.
            if new_paused.is_empty() {
                self.move_dominated_path_to_end(&mut terminated_paths);
                let result = terminated_paths.pop().expect("a terminated path");
                return OptznResult { primary_clobber: result, other_clobbers: terminated_paths };
            }
            let mut def_chain_end: Option<AccessId> = None;
            let mut clobbers: Vec<TerminatedPath> = Vec::new();
            for &paused in &new_paused {
                let mut node = self.paths[paused].clone();
                let wr = self.walk_to_phi_or_clobber(&mut node, None, None);
                self.paths[paused] = node;
                if wr.is_known_clobber {
                    clobbers.push(TerminatedPath { clobber: wr.result, last_node: paused });
                } else {
                    // Micro-opt: if we hit the end of the chain, save it.
                    def_chain_end = Some(wr.result);
                }
            }
            if !terminated_paths.is_empty() {
                // If we couldn't find the dominating phi/liveOnEntry in the
                // above loop, do it now.
                if def_chain_end.is_none() {
                    let mut at = target;
                    while let Some(next) = defining_of(self.mssa, at) {
                        at = next;
                    }
                    def_chain_end = Some(at);
                }
                let chain_bb = self.mssa.access(def_chain_end.expect("a chain end")).block;
                // Because we know the chain end is as 'high' as we can go,
                // block dominance is sufficient.
                for tp in &terminated_paths {
                    if self
                        .mssa
                        .dominance
                        .dominates_resolved(cfg::id(chain_bb), cfg::id(self.mssa.access(tp.clobber).block))
                    {
                        clobbers.push(*tp);
                    }
                }
            }
            // If we have clobbers in the def chain, find the one closest to
            // `current` and quit.
            if !clobbers.is_empty() {
                self.move_dominated_path_to_end(&mut clobbers);
                let result = clobbers.pop().expect("a clobber");
                return OptznResult { primary_clobber: result, other_clobbers: clobbers };
            }
            // Because liveOnEntry is a clobber, this must be a phi.
            let def_chain_phi = def_chain_end.expect("a chain end");
            assert!(self.mssa.access(def_chain_phi).kind == Kind::Phi);
            prior_paths_size = self.paths.len();
            paused_searches.clear();
            for &i in &new_paused {
                let cross = self.paths[i].may_be_cross_iteration;
                self.add_searches(def_chain_phi, &mut paused_searches, i, cross);
            }
            new_paused.clear();
            current = def_chain_phi;
        }
    }

    /// `findClobber`: the nearest clobber for the query, optimizing phis if
    /// possible.
    fn find_clobber(
        &mut self,
        start: AccessId,
        query: UpwardsMemoryQuery,
        up_walk_limit: &mut u32,
    ) -> AccessId {
        self.query = query;
        self.upward_walk_limit = *up_walk_limit;
        // Starting limit must be > 0.
        if self.upward_walk_limit == 0 {
            self.upward_walk_limit += 1;
        }
        let mut current = start;
        // This walker pretends uses don't exist. If we're handed one, silently
        // grab its def.
        if self.mssa.access(start).kind == Kind::Use {
            current = self.mssa.access(start).defining.expect("a use has a defining access");
        }
        let mut first = DefPath {
            loc: self.query.starting_loc.clone(),
            last: current,
            previous: None,
            may_be_cross_iteration: false,
        };
        // Fast path for the overly-common case (no crazy phi optimization
        // necessary).
        let walk = self.walk_to_phi_or_clobber(&mut first, None, None);
        let result = if walk.is_known_clobber {
            walk.result
        } else {
            let opt = self.try_optimize_phi(first.last, self.query.starting_loc.clone());
            debug_assert!(
                opt.other_clobbers.iter().all(|p| self.mssa.dominates(p.clobber, opt.primary_clobber.clobber))
            );
            self.paths.clear();
            self.visited_phis.clear();
            opt.primary_clobber.clobber
        };
        *up_walk_limit = self.upward_walk_limit;
        result
    }
}

impl Mssa {
    fn walker<'m, 'a>(
        &'m self,
        aa: &'m AliasQuery<'a>,
    ) -> ClobberWalker<'m, 'a> {
        ClobberWalker {
            mssa: self,
            aa,
            query: UpwardsMemoryQuery {
                is_call: false,
                starting_loc: None,
                inst: None,
                original_access: None,
                skip_self_access: false,
            },
            upward_walk_limit: 0,
            paths: Vec::new(),
            visited_phis: HashSet::default(),
        }
    }

    /// [`ClobberWalkerBase::getClobberingMemoryAccessBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2530)
    /// (`UseInvariantGroup` is false): the clobbering access of the instruction
    /// of `ma`, the result stored in the access as its optimized access.
    /// `CachingWalker` is `skip_self` false, `SkipSelfWalker` true.
    pub fn get_clobbering_memory_access_base(
        &mut self,
        aa: &AliasQuery,
        ma: AccessId,
        skip_self: bool,
        limit: &mut u32,
    ) -> AccessId {
        let starting = self.access(ma);
        // If this is a MemoryPhi, we can't do anything.
        if starting.kind == Kind::Phi {
            return ma;
        }
        let mut is_optimized = false;
        // If this is an already optimized use or def, return the optimized
        // result.
        if self.is_optimized(ma) {
            if !skip_self || self.access(ma).kind != Kind::Def {
                return self.get_optimized(ma).expect("an optimized access");
            }
            is_optimized = true;
        }
        let Some(inst) = self.access(ma).inst else { return ma };
        let query = UpwardsMemoryQuery::of(aa, inst, ma);
        if !query.is_call && query.starting_loc.is_none() {
            // No location to disambiguate by: conservatively the access itself.
            return ma;
        }
        if aa.is_use_trivially_optimizable_to_live_on_entry(inst) {
            self.set_optimized(ma, LIVE_ON_ENTRY);
            return LIVE_ON_ENTRY;
        }
        let optimized;
        if !is_optimized {
            // Start with the thing we already think clobbers this location.
            let defining = self.access(ma).defining.expect("a defining access");
            // At this point, the defining access may be the live on entry def.
            // If it is, we will not get a better result.
            if self.is_live_on_entry_def(defining) {
                self.set_optimized(ma, defining);
                return defining;
            }
            optimized = self.walker(aa).find_clobber(defining, query.clone(), limit);
            self.set_optimized(ma, optimized);
        } else {
            optimized = self.get_optimized(ma).expect("an optimized access");
        }
        if skip_self && self.access(optimized).kind == Kind::Phi && self.access(ma).kind == Kind::Def && *limit > 0 {
            let mut skipping = query;
            skipping.skip_self_access = true;
            return self.walker(aa).find_clobber(optimized, skipping, limit);
        }
        optimized
    }

    /// [`ClobberWalkerBase::getClobberingMemoryAccessBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2452)
    /// for a location: the access that clobbers `loc`, walking up from
    /// `starting`.
    pub fn get_clobbering_memory_access_for_location(
        &self,
        aa: &AliasQuery,
        starting: AccessId,
        loc: &Rc<Location>,
        limit: &mut u32,
    ) -> AccessId {
        assert!(self.access(starting).kind != Kind::Use, "use cannot be defining access");
        if self.is_live_on_entry_def(starting) {
            return starting;
        }
        let query = UpwardsMemoryQuery {
            is_call: false,
            starting_loc: Some(loc.clone()),
            inst: None,
            original_access: Some(starting),
            skip_self_access: false,
        };
        // Unlike the other function, do not walk to the def of a def: we are
        // handed something we already believe is the clobbering access.
        self.walker(aa).find_clobber(starting, query, limit)
    }

    /// `CachingWalker::invalidateInfo` and `SkipSelfWalker::invalidateInfo`.
    pub fn invalidate_info(
        &mut self,
        ma: AccessId,
    ) {
        if self.access(ma).kind != Kind::Phi {
            self.reset_optimized(ma);
        }
    }

    /// `CachingWalker::getClobberingMemoryAccess(MA, BAA)`.
    pub fn get_clobbering_memory_access(
        &mut self,
        aa: &AliasQuery,
        ma: AccessId,
    ) -> AccessId {
        let mut limit = MAX_CHECK_LIMIT;
        self.get_clobbering_memory_access_base(aa, ma, false, &mut limit)
    }

    /// `SkipSelfWalker::getClobberingMemoryAccess(MA, BAA)`.
    pub fn get_clobbering_memory_access_skip_self(
        &mut self,
        aa: &AliasQuery,
        ma: AccessId,
    ) -> AccessId {
        let mut limit = MAX_CHECK_LIMIT;
        self.get_clobbering_memory_access_base(aa, ma, true, &mut limit)
    }

    /// [`checkClobberSanity`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L390):
    /// `clobber_at` is a clobber for `start`, and nothing between them clobbers
    /// `start`.
    pub fn check_clobber_sanity(
        &self,
        aa: &AliasQuery,
        start: AccessId,
        clobber_at: AccessId,
        start_loc: Option<&Rc<Location>>,
        query_inst: Option<InstId>,
    ) -> Result<(), String> {
        if !self.dominates(clobber_at, start) {
            return Err("clobber doesn't dominate start?".to_owned());
        }
        if self.is_live_on_entry_def(start) {
            return if self.is_live_on_entry_def(clobber_at) {
                Ok(())
            } else {
                Err("liveOnEntry must clobber itself".to_owned())
            };
        }
        let mut found_clobber = false;
        let mut visited: HashSet<(AccessId, Option<u32>, bool)> = HashSet::default();
        let mut worklist: Vec<UpwardDefsElem> =
            vec![UpwardDefsElem { access: start, loc: start_loc.cloned(), may_be_cross_iteration: false }];
        // Walk all paths from `start` to `clobber_at`, while looking for
        // clobbers.
        while let Some(map) = worklist.pop() {
            if !visited.insert(map.key()) {
                continue;
            }
            let mut at = Some(map.access);
            while let Some(ma) = at {
                if ma == clobber_at {
                    if self.access(ma).kind == Kind::Def {
                        found_clobber = found_clobber
                            || self.is_live_on_entry_def(ma)
                            || instruction_clobbers_query(self, aa, ma, map.loc.as_deref(), query_inst);
                    }
                    break;
                }
                if self.is_live_on_entry_def(ma) {
                    return Err("hit liveOnEntry before clobber?".to_owned());
                }
                match self.access(ma).kind {
                    Kind::Def => {
                        if ma != start && instruction_clobbers_query(self, aa, ma, map.loc.as_deref(), query_inst) {
                            return Err("found clobber before reaching ClobberAt!".to_owned());
                        }
                    }
                    Kind::Use => {
                        if ma != start {
                            return Err("can only find use in def chain if start is a use".to_owned());
                        }
                    }
                    Kind::Phi => {
                        for (e, block) in upward_defs(
                            self,
                            aa,
                            &UpwardDefsElem {
                                access: ma,
                                loc: map.loc.clone(),
                                may_be_cross_iteration: map.may_be_cross_iteration,
                            },
                        ) {
                            if block.is_none_or(|b| self.dominance.reachable_resolved(cfg::id(b))) {
                                worklist.push(e);
                            }
                        }
                    }
                }
                at = if self.access(ma).kind == Kind::Phi { None } else { self.access(ma).defining };
            }
        }
        // If `clobber_at` is a MemoryPhi, we can assume something above it
        // acted as a clobber. Otherwise it should have acted as a
        // clobber at some point.
        if self.access(clobber_at).kind == Kind::Phi || found_clobber {
            Ok(())
        } else {
            Err("ClobberAt never acted as a clobber".to_owned())
        }
    }
}
