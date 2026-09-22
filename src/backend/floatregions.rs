//! Port of `qbopt/backend/floatregions.py`: owned extended-precision storage
//! between independently allocated x87 regions.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::analysis::loops;
use crate::backend::frame::{self as frames, Frame};
use crate::backend::lower::Unlowered;
use crate::backend::phielim::placed_on_edges;
use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::mir::MirBlock;
use crate::support::pyset::PySet;

/// The Python exceptions this module and `floatalloc` raise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Raised {
    Unlowered(Unlowered),
    Refused(frames::Refused),
    /// `ValueError`.
    Value(String),
}

impl From<Unlowered> for Raised {
    fn from(error: Unlowered) -> Self {
        Self::Unlowered(error)
    }
}

impl From<frames::Refused> for Raised {
    fn from(error: frames::Refused) -> Self {
        Self::Refused(error)
    }
}

impl fmt::Display for Raised {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlowered(error) => error.fmt(formatter),
            Self::Refused(error) => error.fmt(formatter),
            Self::Value(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Raised {}

fn unlowered(message: &str) -> Raised {
    Raised::Unlowered(Unlowered(message.to_owned()))
}

/// `loops` takes MIR blocks; only `at` and `succ` are read.
pub(crate) fn _graph(body: &LirBody) -> Vec<MirBlock> {
    body.blocks
        .iter()
        .map(|block| MirBlock::new(block.at, Vec::new(), Vec::new(), block.succ.clone()))
        .collect()
}

/// Whether the x87 stack cannot be assumed to survive this instruction.
#[must_use]
pub fn boundary(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    matches!(what.op, Operation::Call | Operation::Barrier)
        || what.sources.iter().chain(&what.dests).any(|arg| matches!(arg, Loc::St(_)))
}

fn _held(arg: &Loc) -> Option<Held> {
    match arg {
        Loc::Held(held) => Some(*held),
        _ => None,
    }
}

/// `regions` is keyed by `(block.at, position)`: a phi is defined at -1 and
/// read at its predecessor's end.
pub fn bridged(
    body: &LirBody,
    regions: &HashMap<(i64, i64), i64>,
    mut frame: Option<&mut Frame>,
) -> Result<LirBody, Raised> {
    let mut definitions: IndexMap<u32, Vec<(i64, i64)>> = IndexMap::new();
    let mut readers: IndexMap<u32, Vec<(i64, i64)>> = IndexMap::new();
    let mut widths: HashMap<u32, BTreeSet<u32>> = HashMap::new();
    let pinned: HashSet<u32> = body.pins.keys().copied().collect();
    let mut identifiers: HashSet<u32> = body.origin.keys().copied().collect();
    identifiers.extend(&pinned);
    for block in &body.blocks {
        for phi in &block.phis {
            identifiers.insert(phi.result);
            identifiers.extend(phi.incoming.iter().map(|(_, value)| *value));
        }
        for (index, one) in block.insns.iter().enumerate() {
            identifiers.extend(one.defines.iter().chain(&one.uses));
            let Some(what) = &one.what else {
                continue;
            };
            for (operands, locations) in [(&what.dests, &mut definitions), (&what.sources, &mut readers)] {
                for arg in operands {
                    if let Loc::Held(arg) = arg {
                        identifiers.insert(arg.value);
                        widths.entry(arg.value).or_default().insert(arg.width);
                        if arg.width == 10 {
                            locations.entry(arg.value).or_default().push((block.at, index as i64));
                        }
                    }
                }
            }
        }
    }
    let mut floating: HashSet<u32> = definitions.keys().chain(readers.keys()).copied().collect();
    let mut phis: Vec<(usize, &crate::model::lir::Phi)> = body
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(index, block)| block.phis.iter().map(move |phi| (index, phi)))
        .collect();
    loop {
        let mut expanded = floating.clone();
        for (_, phi) in &phis {
            let values = std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value));
            if values.clone().any(|value| floating.contains(&value)) {
                expanded.extend(values);
            }
        }
        if expanded == floating {
            break;
        }
        floating = expanded;
    }
    phis.retain(|(_, phi)| floating.contains(&phi.result));
    let at_of: IndexMap<i64, &crate::model::lir::LirBlock> =
        body.blocks.iter().map(|block| (block.at, block)).collect();
    let predecessors = loops::predecessors(&_graph(body));
    for (index, phi) in &phis {
        let block = &body.blocks[*index];
        let before = &predecessors[&block.at];
        if block.at == body.entry
            || phi.incoming.is_empty()
            || phi.incoming.len() != before.len()
            || phi.incoming.iter().map(|(where_, _)| *where_).collect::<BTreeSet<i64>>() != *before
        {
            return Err(unlowered("floating phi does not cover its incoming edges"));
        }
        definitions.entry(phi.result).or_default().push((block.at, -1));
        for (where_, value) in &phi.incoming {
            let Some(source) = at_of.get(where_) else {
                return Err(unlowered("floating phi has an external predecessor"));
            };
            readers.entry(*value).or_default().push((*where_, source.insns.len() as i64));
        }
    }
    // Iterated only to pick which refusal comes first.
    let mut crossing: PySet<i64> = phis
        .iter()
        .flat_map(|(_, phi)| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)))
        .map(i64::from)
        .collect();
    let spanning: PySet<i64> = readers
        .iter()
        .filter(|(value, uses)| {
            definitions.get(*value).is_some_and(|defined| {
                uses.iter().any(|one| regions[one] != regions[&defined[0]])
            })
        })
        .map(|(value, _)| i64::from(*value))
        .collect();
    for value in spanning.iter() {
        crossing.add(*value);
    }
    if crossing.is_empty() {
        return Ok(body.clone());
    }
    let Some(frame) = frame.as_deref_mut() else {
        return Err(unlowered("floating region crossing requires an owned frame"));
    };
    let doms = loops::dominators(&_graph(body), Some(body.entry));
    for value in crossing.iter() {
        let value = *value as u32;
        let defined = definitions.entry(value).or_default();
        if defined.len() != 1
            || pinned.contains(&value)
            || widths.entry(value).or_default().iter().any(|width| *width != 10)
        {
            return Err(unlowered("floating region crossing requires an unpinned SSA definition"));
        }
        let (defined_at, defined_index) = defined[0];
        for (at, index) in readers.entry(value).or_default().iter() {
            if !doms[at].contains(&defined_at) || *at == defined_at && *index <= defined_index {
                return Err(unlowered("floating region input is not dominated by its definition"));
            }
        }
    }
    let crossing: BTreeSet<u32> = crossing.iter().map(|value| *value as u32).collect();
    let mut cells: IndexMap<u32, Mem> = IndexMap::new();
    for value in &crossing {
        cells.insert(*value, frame.cell(("floating-region", i64::from(*value)), 10)?);
    }
    let mut fresh = identifiers.iter().copied().max().unwrap_or(0) + 1;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let mut resident: IndexMap<u32, Held> = IndexMap::new();
        for one in &block.insns {
            if boundary(one) {
                resident.clear();
            }
            let explicit: HashSet<u32> = match &one.what {
                None => HashSet::new(),
                Some(what) => what
                    .sources
                    .iter()
                    .chain(&what.dests)
                    .filter_map(_held)
                    .filter(|arg| arg.width == 10)
                    .map(|arg| arg.value)
                    .collect(),
            };
            if one
                .uses
                .iter()
                .chain(&one.defines)
                .any(|value| crossing.contains(value) && !explicit.contains(value))
            {
                return Err(unlowered("floating region value has an unmodelled use"));
            }
            if one
                .requires
                .iter()
                .chain(&one.delivers)
                .any(|(held, _)| crossing.contains(&held.value))
            {
                return Err(unlowered("floating region value has an integer register constraint"));
            }
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            let mut renamed: IndexMap<u32, Held> = IndexMap::new();
            for arg in &what.sources {
                if let Loc::Held(arg) = arg {
                    if crossing.contains(&arg.value) && !renamed.contains_key(&arg.value) {
                        let local = match resident.get(&arg.value) {
                            Some(local) => *local,
                            None => {
                                let local = Held { value: fresh, width: 10 };
                                fresh += 1;
                                resident.insert(arg.value, local);
                                insns.push(Arc::new(Insn::new(
                                    one.at,
                                    Some((one.at, one.at)),
                                    Some(Semantics {
                                        name: Some("fld".to_owned()),
                                        dests: vec![Loc::Held(local)],
                                        sources: vec![Loc::Mem(cells[&arg.value].clone())],
                                        ..Semantics::new(Operation::FloatLoad)
                                    }),
                                    vec![local.value],
                                    Vec::new(),
                                )));
                                local
                            }
                        };
                        renamed.insert(arg.value, local);
                    }
                }
            }
            let mut replaced = (**one).clone();
            replaced.what = Some(Semantics {
                sources: what
                    .sources
                    .iter()
                    .map(|arg| match arg {
                        Loc::Held(held) => renamed.get(&held.value).map_or_else(|| arg.clone(), |local| Loc::Held(*local)),
                        _ => arg.clone(),
                    })
                    .collect(),
                ..what.clone()
            });
            replaced.uses = one
                .uses
                .iter()
                .map(|value| renamed.get(value).map_or(*value, |local| local.value))
                .collect();
            replaced.widths = one
                .widths
                .iter()
                .map(|(value, width)| (renamed.get(value).map_or(*value, |local| local.value), *width))
                .collect();
            insns.push(Arc::new(replaced));
            for arg in &what.dests {
                if let Loc::Held(arg) = arg {
                    if crossing.contains(&arg.value) {
                        insns.push(Arc::new(Insn::new(
                            one.at,
                            Some((one.at, one.at)),
                            Some(Semantics {
                                name: Some("fstp".to_owned()),
                                dests: vec![Loc::Mem(cells[&arg.value].clone())],
                                sources: vec![Loc::Held(*arg)],
                                ..Semantics::new(Operation::FloatStore)
                            }),
                            Vec::new(),
                            vec![arg.value],
                        )));
                    }
                }
            }
        }
        let mut replaced = block.clone();
        replaced.insns = insns;
        replaced.phis = block.phis.iter().filter(|phi| !floating.contains(&phi.result)).cloned().collect();
        blocks.push(replaced);
    }
    let mut transfers: IndexMap<(i64, i64), Vec<(u32, u32)>> = IndexMap::new();
    for (index, phi) in &phis {
        for (where_, value) in &phi.incoming {
            transfers.entry((*where_, body.blocks[*index].at)).or_default().push((phi.result, *value));
        }
    }
    let mut selected: IndexMap<(i64, i64), Vec<Arc<Insn>>> = IndexMap::new();
    for (edge, pairs) in &transfers {
        let (where_, _) = *edge;
        let at = at_of[&where_].insns.last().map_or(where_, |last| last.at);
        let (mut loads, mut stores) = (Vec::new(), Vec::new());
        for (result, value) in pairs {
            let local = Held { value: fresh, width: 10 };
            fresh += 1;
            loads.push(Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(Semantics {
                    name: Some("fld".to_owned()),
                    dests: vec![Loc::Held(local)],
                    sources: vec![Loc::Mem(cells[value].clone())],
                    ..Semantics::new(Operation::FloatLoad)
                }),
                vec![local.value],
                Vec::new(),
            )));
            stores.push(Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(Semantics {
                    name: Some("fstp".to_owned()),
                    dests: vec![Loc::Mem(cells[result].clone())],
                    sources: vec![Loc::Held(local)],
                    ..Semantics::new(Operation::FloatStore)
                }),
                Vec::new(),
                vec![local.value],
            )));
        }
        loads.extend(stores);
        selected.insert(*edge, loads);
    }
    let mut out = body.clone();
    out.blocks = blocks;
    Ok(placed_on_edges(&out, &selected))
}
