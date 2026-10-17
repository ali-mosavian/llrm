//! POC: LLVM's DSE shape on llrm's memory SSA, for measurement. Not wired into the pipeline.
//!
//! Killing stores are taken bottom-up; for each, the defs above it are walked (scan limit 150, step
//! limit 90, partial limit 5) to the first def it overwrites completely or partly; that def is dead
//! when no read can reach it before a killing def on any path (a forward search over its users), and
//! the paths to the function's exits are covered by killing blocks where the object is visible
//! after return. A second sweep removes stores to private objects nothing reads afterwards.
use std::collections::{HashMap, HashSet};

use llrm_analysis::cfg;
use llrm_analysis::manager::Pointers;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::memoryssa::{self, Accesses, Kind, MemorySSA};
use llrm_analysis::observers;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};

const SCAN_LIMIT: usize = 150;
const STEP_LIMIT: usize = 90;
const PARTIAL_LIMIT: usize = 5;
const SAME_BLOCK_COST: usize = 1;
const OTHER_BLOCK_COST: usize = 5;

pub struct DsePoc;

impl FunctionPass for DsePoc {
    fn name(&self) -> &'static str {
        "dse"
    }

    fn reads(&self) -> Footprint {
        Footprint::MEMORY | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match dropped(unit, analyses) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("dse poc: {error}"),
        }
    }
}

/// Counters of what the walks did, for the replay driver.
#[derive(Default, Debug, Clone)]
pub struct Counts {
    pub killing: usize,
    pub candidates: usize,
    pub complete: usize,
    pub partial: usize,
    pub dead_by_kill: usize,
    pub dead_at_end: usize,
    pub bail_read: usize,
    pub bail_limit: usize,
    pub bail_exit: usize,
    pub bail_phi: usize,
    pub up_steps: usize,
    pub forward_scans: usize,
}

thread_local! {
    pub static COUNTS: std::cell::RefCell<Counts> = std::cell::RefCell::new(Counts::default());
}

fn count(f: impl FnOnce(&mut Counts)) {
    COUNTS.with(|c| f(&mut c.borrow_mut()));
}

fn dropped(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
) -> Result<bool, String> {
    let dead = {
        let (context, layout) = (&*unit.context, unit.layout);
        let function = &*unit.function;
        let has_store = function.walk().any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Store { .. }));
        if !has_store {
            return Ok(false);
        }
        let plain = std::env::var_os("POC_PLAIN").is_some();
        let nopriv = std::env::var_os("POC_NOPRIV").is_some();
        let managed = if plain { None } else { Some(Accesses::managed(context, layout, function, analyses)?) };
        let pointers_held = (!nopriv).then(|| analyses.get::<Pointers>(context, layout, function));
        let published_held = (!nopriv).then(|| analyses.get::<observers::Published>(context, layout, function));
        let shape = analyses.get::<cfg::Shape>(context, layout, function);
        let memory = Unit::within(context, layout, function, analyses.outer()).with_shape(&shape);
        let accesses = managed.unwrap_or_else(|| {
            std::rc::Rc::new(Accesses::plain(&memory, &llrm_analysis::consts::Calls::default()))
        });
        let private_fn: Box<dyn Fn(&MemRef) -> bool> = match (&pointers_held, &published_held) {
            (Some(pointers), Some(published)) => {
                let pointers = Result::as_ref(&**pointers).map_err(String::clone)?;
                let published = Result::as_ref(&**published).map_err(String::clone)?;
                Box::new(observers::private_of(memory, pointers, published))
            }
            _ => Box::new(|_| false),
        };
        let private = &*private_fn;
        let graph = memoryssa::built(&memory, &accesses);
        if std::env::var_os("POC_ANALYSES").is_some() {
            return Ok(false);
        }
        let dead = State::new(&memory, &accesses, &graph, private, &shape).eliminate();
        dead
    };
    unit.function.erase_all(&dead)?;
    Ok(!dead.is_empty())
}

struct State<'a> {
    unit: &'a Unit<'a>,
    accesses: &'a Accesses,
    graph: &'a MemorySSA<'a>,
    private: &'a dyn Fn(&MemRef) -> bool,
    shape: &'a cfg::Shape,
    users: HashMap<usize, Vec<usize>>,
    blocks: HashMap<i64, Vec<i64>>,
    deleted: HashSet<InstId>,
}

enum Overwrite {
    Complete,
    Partial,
    None,
}

impl<'a> State<'a> {
    fn new(
        unit: &'a Unit<'a>,
        accesses: &'a Accesses,
        graph: &'a MemorySSA<'a>,
        private: &'a dyn Fn(&MemRef) -> bool,
        shape: &'a cfg::Shape,
    ) -> Self {
        let mut users: HashMap<usize, Vec<usize>> = HashMap::new();
        for access in graph.accesses.iter().chain(std::iter::once(&graph.live)).chain(graph.phis.values()) {
            if let Some(above) = access.defining {
                users.entry(above).or_default().push(access.id);
            }
            for (_, above) in &access.incoming {
                users.entry(*above).or_default().push(access.id);
            }
        }
        let blocks = cfg::graph(unit.function).iter().map(|block| (block.at, block.succ.to_vec())).collect();
        Self { unit, accesses, graph, private, shape, users, blocks, deleted: HashSet::new() }
    }

    fn function(&self) -> &llrm_mir::module::Function {
        self.unit.function
    }

    fn store_loc(
        &self,
        inst: InstId,
    ) -> Option<&MemRef> {
        let Opcode::Store { volatile: false, .. } = self.function().instruction(inst).opcode else { return None };
        match self.accesses.writes(inst)? {
            [one] => Some(one),
            _ => None,
        }
    }

    fn block_of(
        &self,
        inst: InstId,
    ) -> Option<i64> {
        self.function().parent(inst).map(cfg::id)
    }

    fn overlaps(
        &self,
        one: &MemRef,
        other: &MemRef,
    ) -> bool {
        memoryssa::may_clobber(self.unit, None, one, other)
    }

    fn overwrite(
        &self,
        killing: &MemRef,
        dead: &MemRef,
    ) -> Overwrite {
        if memoryssa::covers(self.unit, killing, dead) {
            Overwrite::Complete
        } else if self.overlaps(killing, dead) {
            Overwrite::Partial
        } else {
            Overwrite::None
        }
    }

    /// Whether `inst` may read `loc`.
    fn reads(
        &self,
        inst: InstId,
        loc: &MemRef,
    ) -> bool {
        match self.accesses.reads(inst) {
            None => !(self.private)(loc),
            Some(reads) => reads.iter().any(|read| self.overlaps(loc, read)),
        }
    }

    fn eliminate(mut self) -> Vec<InstId> {
        // Killing stores, bottom-up.
        let order: Vec<InstId> = self.function().walk().map(|(_, inst)| inst).collect();
        for &killing in order.iter().rev() {
            if self.deleted.contains(&killing) {
                continue;
            }
            let Some(loc) = self.store_loc(killing).cloned() else { continue };
            count(|c| c.killing += 1);
            let mut scan = SCAN_LIMIT;
            let mut steps = STEP_LIMIT;
            let mut partial = PARTIAL_LIMIT;
            let mut start = self.graph.at(killing).defining;
            let mut seen_phis: HashSet<usize> = HashSet::new();
            let mut pending: Vec<Option<usize>> = vec![start.take()];
            while let Some(from) = pending.pop() {
                let mut at = from;
                loop {
                    if scan == 0 || steps == 0 {
                        count(|c| c.bail_limit += 1);
                        pending.clear();
                        break;
                    }
                    let Some(id) = at else { break };
                    let access = self.graph.access(id).clone();
                    count(|c| c.up_steps += 1);
                    match access.kind {
                        Kind::Live => break,
                        Kind::Phi => {
                            if seen_phis.insert(id) {
                                // LLVM leaves the phi to the caller, which walks every incoming access.
                                for (_, above) in &access.incoming {
                                    pending.push(Some(*above));
                                }
                            }
                            count(|c| c.bail_phi += 1);
                            break;
                        }
                        Kind::Use => {
                            at = access.defining;
                        }
                        Kind::Def => {
                            let site = access.site.expect("a def has a site");
                            let cost = if self.block_of(site) == self.block_of(killing) { SAME_BLOCK_COST } else { OTHER_BLOCK_COST };
                            if steps <= cost {
                                count(|c| c.bail_limit += 1);
                                pending.clear();
                                break;
                            }
                            steps -= cost;
                            if self.deleted.contains(&site) {
                                at = access.defining;
                                continue;
                            }
                            // A def that may read the killing store's bytes ends the walk.
                            if self.store_loc(site).is_none() {
                                if self.reads(site, &loc) {
                                    count(|c| c.bail_read += 1);
                                    pending.clear();
                                    break;
                                }
                                at = access.defining;
                                continue;
                            }
                            let dead_loc = self.store_loc(site).expect("a store").clone();
                            match self.overwrite(&loc, &dead_loc) {
                                Overwrite::None => {
                                    at = access.defining;
                                    continue;
                                }
                                Overwrite::Partial => {
                                    if partial <= 1 {
                                        steps = steps.saturating_sub(1);
                                        at = access.defining;
                                        continue;
                                    }
                                    partial -= 1;
                                    count(|c| c.partial += 1);
                                }
                                Overwrite::Complete => count(|c| c.complete += 1),
                            }
                            count(|c| c.candidates += 1);
                            if self.dead_by(killing, &loc, site, &dead_loc, &mut scan) {
                                count(|c| c.dead_by_kill += 1);
                                self.deleted.insert(site);
                            }
                            at = access.defining;
                        }
                    }
                }
            }
        }
        // Stores to private objects nothing reads afterwards.
        for &inst in &order {
            if self.deleted.contains(&inst) {
                continue;
            }
            let Some(loc) = self.store_loc(inst).cloned() else { continue };
            if !(self.private)(&loc) {
                continue;
            }
            let mut scan = SCAN_LIMIT;
            if self.unread_after(inst, &loc, &mut scan) {
                count(|c| c.dead_at_end += 1);
                self.deleted.insert(inst);
            }
        }
        order.into_iter().filter(|inst| self.deleted.contains(inst)).collect()
    }

    /// Whether the store `dead` is overwritten by `killing` (and the other killing stores found) on
    /// every path before anything reads it.
    fn dead_by(
        &mut self,
        killing: InstId,
        _killing_loc: &MemRef,
        dead: InstId,
        dead_loc: &MemRef,
        scan: &mut usize,
    ) -> bool {
        let mut killers: Vec<InstId> = vec![killing];
        let mut work: Vec<usize> = self.users.get(&self.graph.at(dead).id).cloned().unwrap_or_default();
        let mut seen: HashSet<usize> = work.iter().copied().collect();
        let mut i = 0;
        while i < work.len() {
            if *scan < work.len() - i {
                count(|c| c.bail_limit += 1);
                return false;
            }
            *scan -= 1;
            count(|c| c.forward_scans += 1);
            let id = work[i];
            i += 1;
            let access = self.graph.access(id).clone();
            if access.kind == Kind::Phi {
                let phi_block = access.block;
                let covered = killers.iter().any(|k| {
                    matches!((self.block_of(*k), phi_block), (Some(kb), Some(pb)) if kb != pb && self.shape.dominance.dominates(kb, pb))
                });
                if covered {
                    continue;
                }
                self.push_users(id, &mut work, &mut seen);
                continue;
            }
            let Some(site) = access.site else { continue };
            if self.deleted.contains(&site) {
                self.push_users(id, &mut work, &mut seen);
                continue;
            }
            if killers.contains(&site) {
                continue;
            }
            // Another store that overwrites the dead bytes completely, or partly together.
            if let Some(other) = self.store_loc(site) {
                if site != dead && memoryssa::covers(self.unit, other, dead_loc) {
                    killers.push(site);
                    continue;
                }
            }
            if self.reads(site, dead_loc) {
                count(|c| c.bail_read += 1);
                return false;
            }
            if access.kind == Kind::Def {
                self.push_users(id, &mut work, &mut seen);
            }
        }
        // Paths to an exit that no killing block covers keep the store, unless the object is private.
        if !(self.private)(dead_loc) && !self.exits_covered(dead, &killers) {
            count(|c| c.bail_exit += 1);
            return false;
        }
        true
    }

    fn push_users(
        &self,
        id: usize,
        work: &mut Vec<usize>,
        seen: &mut HashSet<usize>,
    ) {
        for &user in self.users.get(&id).into_iter().flatten() {
            if seen.insert(user) {
                work.push(user);
            }
        }
    }

    fn exits_covered(
        &self,
        dead: InstId,
        killers: &[InstId],
    ) -> bool {
        let Some(home) = self.block_of(dead) else { return false };
        let blocking: HashSet<i64> = killers.iter().filter_map(|k| self.block_of(*k)).collect();
        // A killer after the dead store in its own block covers every path.
        if killers.iter().any(|k| self.block_of(*k) == Some(home) && self.after(*k, dead)) {
            return true;
        }
        let mut seen: HashSet<i64> = HashSet::new();
        let mut stack: Vec<i64> = self.blocks.get(&home).cloned().unwrap_or_default();
        let exit = self.blocks.get(&home).is_none_or(Vec::is_empty);
        if exit {
            return false;
        }
        while let Some(block) = stack.pop() {
            if blocking.contains(&block) || !seen.insert(block) {
                continue;
            }
            let next = self.blocks.get(&block).cloned().unwrap_or_default();
            if next.is_empty() {
                return false;
            }
            stack.extend(next);
        }
        true
    }

    fn after(
        &self,
        later: InstId,
        earlier: InstId,
    ) -> bool {
        let Some(block) = self.function().parent(earlier) else { return false };
        let list = self.function().block(block).instructions();
        let at = |inst: InstId| list.iter().position(|one| *one == inst);
        matches!((at(later), at(earlier)), (Some(l), Some(e)) if l > e)
    }

    /// A store to a private object is dead when no access after it can read it.
    fn unread_after(
        &self,
        store: InstId,
        loc: &MemRef,
        scan: &mut usize,
    ) -> bool {
        let id = self.graph.at(store).id;
        let mut work: Vec<usize> = self.users.get(&id).cloned().unwrap_or_default();
        let mut seen: HashSet<usize> = work.iter().copied().collect();
        let mut i = 0;
        while i < work.len() {
            if *scan < work.len() - i {
                return false;
            }
            *scan -= 1;
            let user = work[i];
            i += 1;
            let access = self.graph.access(user).clone();
            if let Some(site) = access.site {
                if !self.deleted.contains(&site) && self.reads(site, loc) {
                    return false;
                }
            }
            if access.kind != Kind::Use {
                self.push_users(user, &mut work, &mut seen);
            }
        }
        true
    }
}
