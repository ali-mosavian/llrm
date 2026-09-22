//! Port of `qbopt/analysis/memoryssa.py`: conservative memory SSA over MIR,
//! rebuilt after a body changes.
//!
//! Stores, calls and barriers define a single memory state; loads use that state.
//! The clobber walker skips stores proven disjoint by MIR alias analysis.
//! Raised call write effects participate in the same alias queries as stores.
//! Calls without write metadata and barriers remain conservative.

#![allow(private_interfaces)] // `RegionLayout` is regions' crate-private type.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use super::regions::{RegionLayout, overlapping};
use super::{effects, loops, pointerfacts};
use crate::model::mir::{self, MemRef, MirBody, Op};

fn _unknown_write(op: &Op) -> bool {
    effects::unmodeled_write(op) || op.floating.is_some() || op.kind == mir::Kind::Fcheck
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Live,
    Use,
    Def,
    Phi,
}

impl Kind {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Live => "live-on-entry",
            Self::Use => "use",
            Self::Def => "def",
            Self::Phi => "phi",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Site {
    pub block: i64,
    pub index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Access {
    pub id: usize,
    pub kind: Kind,
    pub block: Option<i64>,
    pub site: Option<Site>,
    pub defining: Option<usize>,
    // None denotes the invocation edge into the entry block.
    pub incoming: Vec<(Option<i64>, usize)>,
}

impl Access {
    fn new(id: usize, kind: Kind) -> Self {
        Self { id, kind, block: None, site: None, defining: None, incoming: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub struct MemorySSA<'a> {
    pub live: Access,
    pub accesses: Vec<Access>,
    pub sites: IndexMap<Site, Access>,
    pub phis: IndexMap<i64, Access>,
    pub operations: IndexMap<Site, &'a Op>,
    pub pointers: pointerfacts::Offsets<'a>,
}

impl MemorySSA<'_> {
    pub fn at(&self, site: Site) -> &Access {
        &self.sites[&site]
    }

    /// Possible nearest writes before a site, including live-on-entry.
    ///
    /// Walk every phi input. A visited set closes cycles without treating
    /// the backedge as evidence that memory is unchanged. An empty result
    /// means no reachable source was found, not a reusable memory value.
    /// This identifies memory states, not a dominating scalar definition;
    /// forwarding consumers must establish value availability separately.
    pub fn clobbers(&self, site: Site, memory: &MemRef, dgroup: Option<&RegionLayout>) -> BTreeSet<usize> {
        self._frontier(site, memory, dgroup, None, None, None)
    }

    /// Whether a dominating earlier read's memory state still applies.
    ///
    /// The caller must establish dominance and equal addresses. Stop at
    /// the earlier memory version, rejecting any possibly aliasing write
    /// on the way, including writes carried by loop backedges.
    pub fn unchanged(&self, earlier: Site, later: Site, memory: &MemRef, dgroup: Option<&RegionLayout>) -> bool {
        let boundary = self.at(earlier).defining;
        boundary.is_some_and(|boundary| {
            self._frontier(later, memory, dgroup, Some(boundary), None, None) == BTreeSet::from([boundary])
        })
    }

    fn _frontier(
        &self,
        site: Site,
        memory: &MemRef,
        dgroup: Option<&RegionLayout>,
        boundary: Option<usize>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
    ) -> BTreeSet<usize> {
        let accesses: BTreeMap<usize, &Access> = self.accesses.iter().map(|access| (access.id, access)).collect();
        let mut pending = vec![self.at(site).defining];
        let mut seen = BTreeSet::new();
        let mut found = BTreeSet::new();
        while let Some(current) = pending.pop() {
            let Some(current) = current else {
                continue;
            };
            if seen.contains(&current) {
                continue;
            }
            seen.insert(current);
            if Some(current) == boundary {
                found.insert(current);
                continue;
            }
            let access = accesses[&current];
            match access.kind {
                Kind::Live => {
                    found.insert(current);
                }
                Kind::Phi => pending.extend(
                    access
                        .incoming
                        .iter()
                        .filter(|(parent, _)| edge.is_none() || access.block != Some(site.block) || *parent == edge)
                        .map(|(_, value)| Some(*value)),
                ),
                Kind::Def => {
                    let op = self.operations[&access.site.expect("a def has a site")];
                    let queried = match edge_memory {
                        Some(edge_memory) if access.block != Some(site.block) => edge_memory,
                        _ => memory,
                    };
                    // Rust refuses a region endpoint Python's integers can
                    // express; either way the store may overlap.
                    if _unknown_write(op)
                        || op.stores.iter().any(|store| {
                            overlapping(queried, store, None, None, dgroup).unwrap_or(true)
                                && !self.pointers.disjoint(queried, store)
                        })
                    {
                        found.insert(current);
                    } else {
                        pending.push(access.defining);
                    }
                }
                Kind::Use => pending.push(access.defining),
            }
        }
        found
    }

    /// An earlier load/store still supplies these bytes on one incoming edge.
    ///
    /// The caller proves equal addresses and that the scalar provider dominates
    /// the predecessor. Only the destination's memory phi is edge-selected;
    /// other intervening joins still require agreement along every path.
    /// A translated address applies outside the destination; its prefix must
    /// still be checked against the original phi-based address.
    pub fn available_on_edge(
        &self,
        earlier: Site,
        later: Site,
        predecessor: i64,
        memory: &MemRef,
        dgroup: Option<&RegionLayout>,
        edge_memory: Option<&MemRef>,
    ) -> bool {
        let source = self.at(earlier);
        let boundary = if source.kind == Kind::Def { Some(source.id) } else { source.defining };
        boundary.is_some_and(|boundary| {
            self._frontier(later, memory, dgroup, Some(boundary), Some(predecessor), edge_memory)
                == BTreeSet::from([boundary])
        })
    }
}

/// Wire block entries first, then eliminate identity memory phis.
///
/// Preallocating entries handles backedges without iterative guesses about
/// memory versions. Entry blocks with backedges retain an invocation input.
pub fn built(body: &MirBody) -> MemorySSA<'_> {
    let live = Access::new(0, Kind::Live);
    let entries: IndexMap<i64, usize> =
        body.blocks.iter().enumerate().map(|(index, block)| (block.at, index + 1)).collect();
    let mut sites: IndexMap<Site, Access> = IndexMap::new();
    let mut outgoing: BTreeMap<i64, usize> = BTreeMap::new();
    let mut next_id = entries.len() + 1;
    for block in &body.blocks {
        let mut current = entries[&block.at];
        for (index, op) in block.ops.iter().enumerate() {
            let defines = !op.stores.is_empty() || _unknown_write(op);
            if !(!op.loads.is_empty() || defines) {
                continue;
            }
            let kind = if defines { Kind::Def } else { Kind::Use };
            let site = Site { block: block.at, index };
            sites.insert(
                site,
                Access { id: next_id, kind, block: Some(block.at), site: Some(site), defining: Some(current), incoming: Vec::new() },
            );
            if kind == Kind::Def {
                current = next_id;
            }
            next_id += 1;
        }
        outgoing.insert(block.at, current);
    }

    let predecessors = loops::predecessors(&body.blocks);
    let incoming: BTreeMap<i64, Vec<(Option<i64>, usize)>> = body
        .blocks
        .iter()
        .map(|block| {
            let parents = &predecessors[&block.at];
            let mut edges: Vec<(Option<i64>, usize)> =
                parents.iter().map(|pred| (Some(*pred), outgoing[pred])).collect();
            if block.at == body.entry || parents.is_empty() {
                edges.push((None, live.id));
            }
            (block.at, edges)
        })
        .collect();
    let mut replacements: BTreeMap<usize, usize> = BTreeMap::new();

    let resolved = |replacements: &BTreeMap<usize, usize>, mut value: usize| -> usize {
        while let Some(next) = replacements.get(&value) {
            value = *next;
        }
        value
    };

    let mut changed = true;
    while changed {
        changed = false;
        for (block, entry) in &entries {
            if replacements.contains_key(entry) {
                continue;
            }
            let mut values: BTreeSet<usize> =
                incoming[block].iter().map(|(_, value)| resolved(&replacements, *value)).collect();
            values.remove(entry);
            if values.len() <= 1 {
                replacements.insert(*entry, values.iter().next().copied().unwrap_or(live.id));
                changed = true;
            }
        }
    }

    let phis: IndexMap<i64, Access> = entries
        .iter()
        .filter(|(_, entry)| !replacements.contains_key(entry))
        .map(|(block, entry)| {
            let mut access = Access::new(*entry, Kind::Phi);
            access.block = Some(*block);
            access.incoming =
                incoming[block].iter().map(|(pred, value)| (*pred, resolved(&replacements, *value))).collect();
            (*block, access)
        })
        .collect();
    let sites: IndexMap<Site, Access> = sites
        .into_iter()
        .map(|(site, access)| {
            let defining = access.defining.map(|value| resolved(&replacements, value));
            (site, Access { site: Some(site), defining, ..access })
        })
        .collect();
    let mut accesses: Vec<Access> = std::iter::once(live.clone())
        .chain(phis.values().cloned())
        .chain(sites.values().cloned())
        .collect();
    accesses.sort_by_key(|access| access.id);
    let operations: IndexMap<Site, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| {
            block.ops.iter().enumerate().map(move |(index, op)| (Site { block: block.at, index }, op))
        })
        .filter(|(site, _)| sites.contains_key(site))
        .collect();
    MemorySSA { live, accesses, sites, phis, operations, pointers: pointerfacts::offsets(body) }
}

#[cfg(test)]
#[path = "memoryssa_tests.rs"]
mod tests;
