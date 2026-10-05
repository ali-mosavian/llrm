//! Port of `qbopt/backend/addressforms.py`: fold address arithmetic into
//! the memory operands that read it.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::support::hash::{IndexMap, IndexSet};

use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir::{self, Insn};

/// Make selected word definitions usable as dword address components.
///
/// A plain word load can directly select `movzx r32,m16`.  A complete far
/// pointer load must still use LES/LFS/LGS, so its offset is first delivered
/// to a short-lived temporary and then zero-extended into the original SSA
/// value.  In both cases the promoted value has one definition and remains
/// the same live range for later low-word uses.
pub fn promote(
    blocks: &IndexMap<i64, Vec<Arc<Insn>>>,
    values: &BTreeSet<u32>,
    fresh: &mut dyn FnMut() -> u32,
) -> Result<IndexMap<i64, Vec<Arc<Insn>>>, String> {
    if values.is_empty() {
        return Ok(blocks.clone());
    }
    let mut definitions: IndexMap<u32, (i64, usize)> = IndexMap::default();
    for (&at, insns) in blocks {
        for (index, one) in insns.iter().enumerate() {
            for &value in &one.defines {
                if values.contains(&value) {
                    definitions.insert(value, (at, index));
                }
            }
        }
    }
    if definitions.keys().copied().collect::<BTreeSet<u32>>() != *values {
        let missing = values
            .iter()
            .copied()
            .filter(|value| !definitions.contains_key(value))
            .collect::<Vec<u32>>();
        return Err(format!(
            "secondary address values have no definition: {missing:?}"
        ));
    }
    let mut out: IndexMap<i64, Vec<Arc<Insn>>> = blocks.clone();
    // Work backwards within each block so inserting a follower cannot move a
    // definition still waiting to be rewritten.
    let mut ordered = definitions.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|&(_, (at, index))| (at, -(index as i64)));
    for (value, (at, index)) in ordered {
        let one = Arc::clone(&out[&at][index]);
        let Some(what) = &one.what else {
            return Err(format!(
                "value#{value} has no selected definition to promote"
            ));
        };
        let mut destinations = what.dests.clone();
        let position = destinations.iter().position(
            |destination| matches!(destination, Loc::Held(destination) if destination.value == value && destination.width == 2),
        );
        let Some(position) = position else {
            return Err(format!("value#{value} has no word destination to promote"));
        };
        if what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && destinations.len() == 1
            && what.sources.len() == 1
            && match &what.sources[0] {
                Loc::Mem(source) => source.width == 2,
                Loc::Held(source) => source.width == 2,
                _ => false,
            }
        {
            destinations[0] = Loc::Held(ir::Held { value, width: 4 });
            let mut changed = (*one).clone();
            changed.what = Some(Semantics {
                op: Operation::Extend,
                name: Some("movzx".to_owned()),
                dests: destinations,
                ..what.clone()
            });
            changed.widths = one
                .widths
                .iter()
                .copied()
                .chain([(value, 4)])
                .collect::<IndexSet<_>>()
                .into_iter()
                .collect();
            out[&at][index] = Arc::new(changed);
            continue;
        }
        if what.op != Operation::Move
            || !matches!(what.name.as_deref(), Some("les" | "lfs" | "lgs"))
            || position != 0
        {
            let spelled = match what.name.as_deref() {
                Some(name) if !name.is_empty() => name.to_owned(),
                _ => what.op.to_string(),
            };
            return Err(format!("value#{value} cannot be promoted from {spelled}"));
        }
        let temporary = fresh();
        destinations[0] = Loc::Held(ir::Held {
            value: temporary,
            width: 2,
        });
        let mut leader = (*one).clone();
        leader.what = Some(Semantics {
            dests: destinations,
            ..what.clone()
        });
        leader.defines = one
            .defines
            .iter()
            .map(|&found| if found == value { temporary } else { found })
            .collect();
        leader.widths = one
            .widths
            .iter()
            .map(|&(found, width)| (if found == value { temporary } else { found }, width))
            .collect();
        let mut follower = (*lir::anchor(Arc::clone(&one))).clone();
        follower.what = Some(Semantics {
            name: Some("movzx".to_owned()),
            dests: vec![Loc::Held(ir::Held { value, width: 4 })],
            sources: vec![Loc::Held(ir::Held {
                value: temporary,
                width: 2,
            })],
            ..Semantics::new(Operation::Extend)
        });
        follower.defines = vec![value];
        follower.uses = vec![temporary];
        follower.widths = vec![(temporary, 2), (value, 4)];
        follower.call = None;
        out[&at].splice(index..=index, [Arc::new(leader), Arc::new(follower)]);
    }
    Ok(out)
}
