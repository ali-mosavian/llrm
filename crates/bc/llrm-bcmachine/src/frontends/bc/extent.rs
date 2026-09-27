//! Port of `qbopt/frontend/extent.py`: whose code a byte is -- the module's
//! own main body, or one of its SUB/FUNCTIONs.
//!
//! Built entirely on blocks.rs's reachability, not on CodeView. Every
//! code-segment PUBDEF is a SUB/FUNCTION's own entry point, and reachability,
//! not address order, decides ownership: the main body is generally several
//! disjoint byte ranges. A block ending at an inline `ON GOTO` table has its
//! real entries recomputed here rather than trusting `Block.succ`, whose
//! over-approximation would let one body's walk swallow another's.

use std::collections::BTreeSet;

use llrm_qbruntime::INLINE_TABLE;

use crate::abi::{callsite, events, handlers};
use crate::frontends::bc::blocks::{
    Block, CodeMap, ENTRY, Ends, code_map, event_stub, has_header, local_call_target,
    partition as block_partition,
};
use crate::frontends::bc::raising_control;
use crate::objectfile::module::Module;
use crate::objectfile::omf;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self, Repr};

pub const EVENT_STUB_NAME: &str = "event poll stub";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyKind {
    Main,
    Procedure,
    EventStub,
    EventHandler,
    ErrorHandler,
    /// A statement RESUME re-enters, as a body of its own for a consumer
    /// whose bodies have one entry each (`Body::apart`).
    ResumeEntry,
}

impl BodyKind {
    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            BodyKind::Main => "MAIN",
            BodyKind::Procedure => "PROCEDURE",
            BodyKind::EventStub => "EVENT_STUB",
            BodyKind::EventHandler => "EVENT_HANDLER",
            BodyKind::ErrorHandler => "ERROR_HANDLER",
            BodyKind::ResumeEntry => "RESUME_ENTRY",
        }
    }

    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            BodyKind::Main => "main",
            BodyKind::Procedure => "procedure",
            BodyKind::EventStub => "event-stub",
            BodyKind::EventHandler => "event-handler",
            BodyKind::ErrorHandler => "error-handler",
            BodyKind::ResumeEntry => "resume-entry",
        }
    }
}

impl Repr for BodyKind {
    fn repr(&self) -> String {
        pyrepr::str_enum("BodyKind", self.name(), self.value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Body {
    pub kind: BodyKind,
    pub seed: usize,
    pub name: Option<String>,
    pub ranges: Vec<(usize, usize)>,
    /// Where the runtime enters it besides its seed: each statement RESUME
    /// continues at that nothing in the body flows to.
    pub entries: Vec<usize>,
    /// The code only each entry reaches, entry by entry: its part of `ranges`.
    pub entry_ranges: Vec<Vec<(usize, usize)>>,
}

impl Body {
    pub fn length(&self) -> usize {
        self.ranges.iter().map(|(lo, hi)| hi - lo).sum()
    }

    /// The body as bodies of one entry each: its seed's code, then each
    /// entry's, as the partition made them before a body had entries.
    pub fn apart(&self) -> Vec<Body> {
        let theirs: Vec<(usize, usize)> = self.entry_ranges.iter().flatten().copied().collect();
        let own = self.ranges.iter().flat_map(|&range| without(range, &theirs)).collect();
        let owner = Body { ranges: own, entries: Vec::new(), entry_ranges: Vec::new(), ..self.clone() };
        let entries = self.entries.iter().zip(&self.entry_ranges).map(|(&seed, ranges)| Body {
            kind: BodyKind::ResumeEntry,
            seed,
            name: Some("resume entry".to_owned()),
            ranges: ranges.clone(),
            entries: Vec::new(),
            entry_ranges: Vec::new(),
        });
        std::iter::once(owner).chain(entries).collect()
    }
}

/// `range` less every one of `holes`.
fn without((lo, hi): (usize, usize), holes: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out = vec![(lo, hi)];
    for &(start, end) in holes {
        out = out.into_iter().flat_map(|(lo, hi)| [(lo, hi.min(start)), (lo.max(end), hi)]).filter(|(lo, hi)| lo < hi).collect();
    }
    out
}

/// `repr(tuple[tuple[int, int], ...])`.
fn spans(ranges: &[(usize, usize)]) -> String {
    let items: Vec<pyrepr::Tuple<usize>> = ranges.iter().map(|&(lo, hi)| pyrepr::Tuple(vec![lo, hi])).collect();
    pyrepr::tuple(&items)
}

impl Repr for Body {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Body",
            &[
                ("kind", self.kind.repr()),
                ("seed", self.seed.repr()),
                ("name", self.name.as_deref().map_or("None".to_owned(), pyrepr::string)),
                ("ranges", spans(&self.ranges)),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partition {
    pub bodies: Vec<Body>,
    // gaps code_map() itself already proved benign -- nop padding, or dead
    // code BC emitted and never enters. Not counted against completeness.
    pub benign: Vec<(usize, usize)>,
    // real, reached code that no body's own reachability claims
    pub unexplained: Vec<(usize, usize)>,
    // claimed by more than one body -- a sign the ownership graph is wrong
    pub conflicts: Vec<(usize, usize)>,
}

impl Partition {
    pub fn complete(&self) -> bool {
        self.unexplained.is_empty() && self.conflicts.is_empty()
    }
}

impl Repr for Partition {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Partition",
            &[
                ("bodies", pyrepr::tuple(&self.bodies)),
                ("benign", spans(&self.benign)),
                ("unexplained", spans(&self.unexplained)),
                ("conflicts", spans(&self.conflicts)),
            ],
        )
    }
}

/// A table block's real successors, not Block.succ's own safe over-approximation.
///
/// Only an INLINE_TABLE call (B$OGTA) is a real jump table with entries worth
/// following; anything else `_table_at` matches -- the /X RESUME map -- is
/// data nothing here jumps into, so only the byte past it is a successor.
pub fn _table_targets(module: &Module, table: (usize, usize), call_name: Option<&str>) -> Vec<usize> {
    let (lo, hi) = table;
    if !call_name.is_some_and(|name| INLINE_TABLE.contains(name)) {
        return vec![hi];
    }
    let entries: BTreeSet<i64> =
        module.operands.keys().copied().filter(|&at| (lo as i64) < at && at < hi as i64).collect();
    let mut out: Vec<usize> = entries.iter().map(|at| module.operands[at].disp as usize).collect();
    out.push(hi);
    out
}

pub fn _table_at(mapped: &CodeMap, end: usize) -> Option<(usize, usize)> {
    mapped.tables.iter().copied().find(|table| table.0 == end)
}

pub fn _successors(module: &Module, mapped: &CodeMap, block: &Block) -> Vec<usize> {
    if block.ends != Ends::Table {
        return block.succ.clone();
    }
    let Some(table) = _table_at(mapped, block.end) else {
        return Vec::new();
    };
    _table_targets(module, table, module.calls.get(&(block.insns.last().unwrap().at as i64)).map(String::as_str))
}

/// Every block this seed's own control flow reaches, refusing to cross into another body's.
pub fn _reachable(
    seed: usize,
    others: &BTreeSet<usize>,
    blocks_by_at: &IndexMap<usize, Block>,
    module: &Module,
    mapped: &CodeMap,
) -> BTreeSet<usize> {
    let mut visited: BTreeSet<usize> = BTreeSet::new();
    let mut frontier = vec![seed];
    while let Some(at) = frontier.pop() {
        if visited.contains(&at) || !blocks_by_at.contains_key(&at) {
            continue;
        }
        visited.insert(at);
        for target in _successors(module, mapped, &blocks_by_at[&at]) {
            if !visited.contains(&target) && !others.contains(&target) {
                frontier.push(target);
            }
        }
    }
    visited
}

pub fn _merge(spans: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = spans.to_vec();
    sorted.sort();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (lo, hi) in sorted {
        match out.last_mut() {
            Some(last) if last.1 == lo => last.1 = hi,
            _ => out.push((lo, hi)),
        }
    }
    out
}

pub fn _ranges(mapped: &CodeMap, blocks_by_at: &IndexMap<usize, Block>, owned: &BTreeSet<usize>) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = owned.iter().map(|at| (blocks_by_at[at].at, blocks_by_at[at].end)).collect();
    for &(lo, hi) in &mapped.tables {
        let owner = blocks_by_at.values().find(|block| block.end == lo);
        if owner.is_some_and(|owner| owned.contains(&owner.at)) {
            spans.push((lo, hi));
        }
    }
    _merge(&spans)
}

/// The module's code, cut into its main body, its procedures, and whatever neither accounts for.
pub fn partition(module: &Module) -> Result<Partition, String> {
    let basic = has_header(module);
    let native = !basic && module.name.ends_with("_TEXT") && omf::code_segment(&module.records).is_some();
    if !basic && !native {
        return Err("no module header: which offset is the entry is not known without one".to_owned());
    }
    let mapped = code_map(module)?;

    let all_blocks = block_partition(module, &mapped);
    let contracts = callsite::for_module(module, None).expect("no external contract to mismatch");
    let all_blocks = raising_control::terminal_edges(all_blocks, &contracts);
    let blocks_by_at: IndexMap<usize, Block> = all_blocks.iter().map(|blk| (blk.at, blk.clone())).collect();
    let names = match omf::pubdef_names(&module.records, module.seg) {
        Ok(names) => names,
        Err(error) => panic!("ValueError: {error}"),
    };

    let mut seeds: Vec<(BodyKind, usize, Option<String>)> =
        if basic { vec![(BodyKind::Main, ENTRY, None)] } else { Vec::new() };
    if let Some(stub) = event_stub(module) {
        seeds.push((BodyKind::EventStub, stub, Some(EVENT_STUB_NAME.to_owned())));
    }
    seeds.extend(module.publics.iter().map(|&at| (BodyKind::Procedure, at as usize, names.get(&at).cloned())));
    let publics: BTreeSet<usize> = module.publics.iter().map(|&at| at as usize).collect();
    if native {
        let mut private: BTreeSet<usize> = all_blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|insn| local_call_target(module, insn))
            .collect();
        private.extend(mapped.procedures.iter().copied());
        let private: BTreeSet<usize> = private.difference(&publics).copied().collect();
        seeds.extend(private.into_iter().map(|at| (BodyKind::Procedure, at, None)));
    }

    let timers: BTreeSet<usize> = events::handler_entries(module).iter().map(|&at| at as usize).collect();
    seeds.extend(
        timers.difference(&publics).map(|&at| (BodyKind::EventHandler, at, Some("timer handler".to_owned()))),
    );

    let handlers: BTreeSet<usize> = handlers::error_entries(module).iter().map(|&at| at as usize).collect();
    let occupied: BTreeSet<usize> = seeds.iter().map(|(_, seed, _)| *seed).collect();
    seeds.extend(
        handlers.difference(&occupied).map(|&at| (BodyKind::ErrorHandler, at, Some("error handler".to_owned()))),
    );

    let seed_offsets: BTreeSet<usize> = seeds.iter().map(|(_, seed, _)| *seed).collect();
    let mut reached: IndexMap<usize, BTreeSet<usize>> = IndexMap::default();
    for (_, seed, _) in &seeds {
        let mut others = seed_offsets.clone();
        others.remove(seed);
        reached.insert(*seed, _reachable(*seed, &others, &blocks_by_at, module, &mapped));
    }
    // RESUME re-enters code at a statement the body's own flow may never
    // reach, as after an ERROR: each such statement is another entry of the
    // body whose code it lies in, the nearest main or procedure seed below.
    let mut entries: IndexMap<usize, Vec<(usize, BTreeSet<usize>)>> = IndexMap::default();
    if !handlers.is_empty() {
        let owned: BTreeSet<usize> = reached.values().flatten().copied().collect();
        let resumable: BTreeSet<usize> = module
            .targets
            .iter()
            .map(|&at| at as usize)
            .filter(|at| blocks_by_at.contains_key(at) && !owned.contains(at))
            .collect();
        let bodies: BTreeSet<usize> =
            seeds.iter().filter(|(kind, _, _)| matches!(kind, BodyKind::Main | BodyKind::Procedure)).map(|(_, seed, _)| *seed).collect();
        for &entry in &resumable {
            let Some(&owner) = bodies.range(..=entry).next_back() else { continue };
            let mut others: BTreeSet<usize> = owned.union(&resumable).copied().collect();
            others.remove(&entry);
            let more = _reachable(entry, &others, &blocks_by_at, module, &mapped);
            reached.get_mut(&owner).expect("a seed's reach").extend(more.iter().copied());
            entries.entry(owner).or_default().push((entry, more));
        }
    }

    let mut owners: IndexMap<usize, usize> = IndexMap::default();
    for (_, seed, _) in &seeds {
        for &at in &reached[seed] {
            *owners.entry(at).or_insert(0) += 1;
        }
    }

    let bodies: Vec<Body> = seeds
        .iter()
        .map(|(kind, seed, name)| Body {
            kind: *kind,
            seed: *seed,
            name: name.clone(),
            ranges: _ranges(&mapped, &blocks_by_at, &reached[seed]),
            entries: entries.get(seed).map_or_else(Vec::new, |entries| entries.iter().map(|(entry, _)| *entry).collect()),
            entry_ranges: entries.get(seed).map_or_else(Vec::new, |entries| entries.iter().map(|(_, reach)| _ranges(&mapped, &blocks_by_at, reach)).collect()),
        })
        .collect();
    let owned_by = |at: &usize| owners.get(at).copied().unwrap_or(0);
    let conflicts = _merge(
        &blocks_by_at.iter().filter(|(at, _)| owned_by(at) > 1).map(|(_, blk)| (blk.at, blk.end)).collect::<Vec<_>>(),
    );
    let unexplained = _merge(
        &blocks_by_at.iter().filter(|(at, _)| owned_by(at) == 0).map(|(_, blk)| (blk.at, blk.end)).collect::<Vec<_>>(),
    );
    let floor = if basic { ENTRY } else { module.start as usize };
    let benign: Vec<(usize, usize)> = mapped.unreached.iter().copied().filter(|gap| gap.0 >= floor).collect();

    Ok(Partition { bodies, benign, unexplained, conflicts })
}

#[cfg(test)]
#[path = "extent_tests.rs"]
mod tests;
