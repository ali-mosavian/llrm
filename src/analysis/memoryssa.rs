//! Port of `qbopt/analysis/memoryssa.py`.

// ---- early port (agent B) ----

use std::collections::BTreeSet;

use indexmap::IndexMap;

use super::{effects, loops, pointerfacts, regions};
use crate::model::mir::{self, MemRef, MirBody, Op};

fn _unknown_write(op: &Op) -> bool {
    effects::unmodeled_write(op) || op.floating.is_some() || op.kind == mir::Kind::Fcheck
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Kind {
    Live,
    Use,
    Def,
    Phi,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Site {
    pub block: i64,
    pub index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Access {
    pub id: i64,
    pub kind: Kind,
    pub block: Option<i64>,
    pub site: Option<Site>,
    pub defining: Option<i64>,
    /// `None` denotes the invocation edge into the entry block.
    pub incoming: Vec<(Option<i64>, i64)>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // `live` and `phis` are read by the rest of the module
pub(crate) struct MemorySSA<'a> {
    pub live: Access,
    pub accesses: Vec<Access>,
    pub sites: IndexMap<Site, Access>,
    pub phis: IndexMap<i64, Access>,
    pub operations: IndexMap<Site, &'a Op>,
    pub pointers: pointerfacts::Offsets<'a>,
}

impl MemorySSA<'_> {
    pub(crate) fn at(&self, site: Site) -> &Access {
        &self.sites[&site]
    }

    /// Possible nearest writes before a site, including live-on-entry.
    pub(crate) fn clobbers(&self, site: Site, memory: &MemRef, dgroup: &BTreeSet<i64>) -> BTreeSet<i64> {
        self._frontier(site, memory, dgroup, None, None, None)
    }

    fn _frontier(
        &self,
        site: Site,
        memory: &MemRef,
        _dgroup: &BTreeSet<i64>,
        boundary: Option<i64>,
        edge: Option<i64>,
        edge_memory: Option<&MemRef>,
    ) -> BTreeSet<i64> {
        let accesses = self.accesses.iter().map(|access| (access.id, access)).collect::<IndexMap<_, _>>();
        let mut pending = vec![self.at(site).defining];
        let mut seen = BTreeSet::new();
        let mut found = BTreeSet::new();
        while let Some(current) = pending.pop() {
            let Some(current) = current.filter(|current| !seen.contains(current)) else {
                continue;
            };
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
                    // `mir.overlapping` passes `dgroup` as a layout, which is
                    // not a `module.Group`: it is Rust's `layout=None`.  An
                    // endpoint Rust cannot represent may overlap.
                    if _unknown_write(op)
                        || op.stores.iter().any(|store| {
                            regions::overlapping(queried, store, None, None, None).unwrap_or(true)
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
}

/// Wire block entries first, then eliminate identity memory phis.
pub(crate) fn built(body: &MirBody) -> MemorySSA<'_> {
    let live = Access {
        id: 0,
        kind: Kind::Live,
        block: None,
        site: None,
        defining: None,
        incoming: Vec::new(),
    };
    let entries = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.at, index as i64 + 1))
        .collect::<IndexMap<_, _>>();
    let mut sites = IndexMap::<Site, Access>::new();
    let mut outgoing = IndexMap::<i64, i64>::new();
    let mut next_id = entries.len() as i64 + 1;
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
                Access {
                    id: next_id,
                    kind,
                    block: Some(block.at),
                    site: Some(site),
                    defining: Some(current),
                    incoming: Vec::new(),
                },
            );
            if kind == Kind::Def {
                current = next_id;
            }
            next_id += 1;
        }
        outgoing.insert(block.at, current);
    }

    let predecessors = loops::predecessors(&body.blocks);
    let empty = BTreeSet::new();
    let incoming = body
        .blocks
        .iter()
        .map(|block| {
            let preds = predecessors.get(&block.at).unwrap_or(&empty);
            let mut edges = preds
                .iter()
                .map(|pred| (Some(*pred), outgoing[pred]))
                .collect::<Vec<_>>();
            if block.at == body.entry || preds.is_empty() {
                edges.push((None, live.id));
            }
            (block.at, edges)
        })
        .collect::<IndexMap<_, _>>();
    let mut replacements = IndexMap::<i64, i64>::new();

    let resolved = |replacements: &IndexMap<i64, i64>, mut value: i64| {
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
            let mut values = incoming[block]
                .iter()
                .map(|(_, value)| resolved(&replacements, *value))
                .collect::<BTreeSet<_>>();
            values.remove(entry);
            if values.len() <= 1 {
                replacements.insert(*entry, values.first().copied().unwrap_or(live.id));
                changed = true;
            }
        }
    }

    let phis = entries
        .iter()
        .filter(|(_, entry)| !replacements.contains_key(*entry))
        .map(|(block, entry)| {
            (
                *block,
                Access {
                    id: *entry,
                    kind: Kind::Phi,
                    block: Some(*block),
                    site: None,
                    defining: None,
                    incoming: incoming[block]
                        .iter()
                        .map(|(pred, value)| (*pred, resolved(&replacements, *value)))
                        .collect(),
                },
            )
        })
        .collect::<IndexMap<_, _>>();
    let sites = sites
        .into_iter()
        .map(|(site, access)| {
            let defining = access.defining.map(|value| resolved(&replacements, value));
            (
                site,
                Access {
                    site: Some(site),
                    defining,
                    ..access
                },
            )
        })
        .collect::<IndexMap<_, _>>();
    let mut accesses = std::iter::once(live.clone())
        .chain(phis.values().cloned())
        .chain(sites.values().cloned())
        .collect::<Vec<_>>();
    accesses.sort_by_key(|access| access.id);
    let operations = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .ops
                .iter()
                .enumerate()
                .map(move |(index, op)| (Site { block: block.at, index }, op))
        })
        .filter(|(site, _)| sites.contains_key(site))
        .collect::<IndexMap<_, _>>();
    MemorySSA {
        live,
        accesses,
        sites,
        phis,
        operations,
        pointers: pointerfacts::offsets(body),
    }
}
