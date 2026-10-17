//! `LLRM_COALESCE_SHADOW`: an LLVM-shaped coalescer run beside
//! `coalesce::joined` on the same body, to count what it would join and what it
//! would not (a spike; the result is not used).
//!
//! The shape is `RegisterCoalescer`'s: copies in loop-depth order, two live
//! ranges joined if they do not overlap and their classes meet, with no
//! interference graph and no Briggs or George test. It builds its own intervals
//! (`intervals_afresh`, the memo would hide the cost) and is charged for
//! everything it reads.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges};
use crate::backend::classes::RegisterClasses;
use crate::backend::coalesce::{_copy, _find, _merged};
use crate::backend::target::{self, Segments};
use crate::backend::{allocate, regclass};
use crate::model::lir::{Block, LirBody};
use crate::support::hash::IndexMap;

/// Joins this makes that `ours` does not, and the reverse, each by the first
/// reason the other refused.
pub fn compare(
    body: &LirBody,
    pinned: &IndexMap<u32, RegId>,
    segments: &Segments,
    classes: &RegisterClasses,
    ours: &IndexMap<usize, bool>,
    refused: &[(usize, &'static str)],
) -> IndexMap<u32, u32> {
    let (joined, why, parent) = shadow(body, pinned, segments, classes);
    let mut report: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let (mut copies, mut ours_n, mut theirs_n) = (0, 0, 0);
    for (key, mine) in ours {
        copies += 1;
        let theirs = joined.get(key).copied().unwrap_or(false);
        ours_n += usize::from(*mine);
        theirs_n += usize::from(theirs);
        match (mine, theirs) {
            (true, false) => {
                let reason = why.iter().find(|(k, _)| k == key).map_or("unknown", |(_, w)| *w);
                *report.entry(format!("missed:{reason}")).or_default() += 1;
            }
            (false, true) => {
                let reason = refused.iter().find(|(k, _)| k == key).map_or("unknown", |(_, w)| *w);
                *report.entry(format!("extra:{reason}")).or_default() += 1;
            }
            _ => {}
        }
    }
    let insns: usize = body.blocks.iter().map(|block| block.insns.len()).sum();
    let classes: Vec<String> = report.iter().map(|(k, v)| format!("{k}={v}")).collect();
    eprintln!("[coalesce-shadow] insns {insns} copies {copies} ours {ours_n} poc {theirs_n} {}", classes.join(" "));
    parent
}

type Why = Vec<(usize, &'static str)>;

fn shadow(
    body: &LirBody,
    pinned: &IndexMap<u32, RegId>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> (IndexMap<usize, bool>, Why, IndexMap<u32, u32>) {
    let index = ranges::indexed(body);
    let mut live = ranges::intervals_afresh(body);
    let masks = allocate::_masks(body, &index, segments);
    let mut widths = allocate::_widest(body);
    let where_of = regclass::classes(body, &BTreeSet::new(), segments, classes);
    let mut palette: IndexMap<u32, BTreeSet<RegId>> = live
        .keys()
        .map(|one| (*one, target::order(where_of.get(one), segments, classes).into_iter().collect()))
        .collect();
    let everything: BTreeSet<RegId> = classes.available.iter().copied().collect();
    for (value, register) in pinned {
        if !everything.contains(register) && !where_of.contains_key(value) {
            palette.insert(*value, BTreeSet::from([*register]));
        }
    }
    let mut held: IndexMap<u32, RegId> = pinned.clone();
    let mut parent: IndexMap<u32, u32> = IndexMap::default();
    let mut node_of: IndexMap<u32, u32> = IndexMap::default();
    let busy = crate::analysis::frequency::Frequency::of(body);
    let depth = ranges::depths_shared(body);
    let mut blocks: Vec<&Block> = body.blocks.iter().collect();
    blocks.sort_by(|a, b| {
        let (a_depth, b_depth) = (depth.get(&a.at).copied().unwrap_or(0), depth.get(&b.at).copied().unwrap_or(0));
        b_depth
            .cmp(&a_depth)
            .then_with(
                || if a_depth == 0 { std::cmp::Ordering::Equal } else { busy.block(b.at).total_cmp(&busy.block(a.at)) },
            )
    });
    let mut why: Why = Vec::new();
    for block in blocks {
        for one in &block.insns {
            let Some(pair) = _copy(one) else { continue };
            let key = ranges::key(one);
            let (here, there) = (_find(&mut parent, pair.0), _find(&mut parent, pair.1));
            if here == there {
                continue;
            }
            let (mine_node, theirs_node) =
                (node_of.get(&here).copied().unwrap_or(here), node_of.get(&there).copied().unwrap_or(there));
            let (mine_pin, theirs_pin) = (held.get(&mine_node).copied(), held.get(&theirs_node).copied());
            if matches!((mine_pin, theirs_pin), (Some(a), Some(b)) if a != b) {
                why.push((key, "pin"));
                continue;
            }
            let (Some(mine), Some(theirs)) = (live.get(&mine_node), live.get(&theirs_node)) else {
                why.push((key, "no-interval"));
                continue;
            };
            if mine.overlaps(theirs) {
                why.push((key, "overlap"));
                continue;
            }
            let allowed: BTreeSet<RegId> = palette
                .get(&mine_node)
                .into_iter()
                .flatten()
                .filter(|register| palette.get(&theirs_node).is_some_and(|other| other.contains(register)))
                .copied()
                .collect();
            if allowed.is_empty() {
                why.push((key, "palette"));
                continue;
            }
            let merged = _merged(mine, theirs);
            let width =
                widths.get(&mine_node).copied().unwrap_or(0).max(widths.get(&theirs_node).copied().unwrap_or(0)).max(1);
            let allowed: BTreeSet<RegId> = allowed
                .into_iter()
                .filter(|register| !allocate::_clobbered(&merged, *register, &masks, width))
                .collect();
            if allowed.is_empty() {
                why.push((key, "clobber"));
                continue;
            }
            if [mine_pin, theirs_pin].into_iter().flatten().any(|pin| !allowed.contains(&pin)) {
                why.push((key, "pin-clobber"));
                continue;
            }
            let pin = mine_pin.or(theirs_pin);
            parent.insert(here, there);
            node_of.insert(there, mine_node);
            live.insert(mine_node, merged);
            live.swap_remove(&theirs_node);
            widths.insert(mine_node, width);
            palette.insert(mine_node, allowed);
            palette.swap_remove(&theirs_node);
            held.swap_remove(&theirs_node);
            if let Some(register) = pin {
                held.insert(mine_node, register);
            }
        }
    }
    let joined = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter_map(|one| {
            _copy(one).map(|pair| (ranges::key(one), _find(&mut parent, pair.0) == _find(&mut parent, pair.1)))
        })
        .collect();
    (joined, why, parent)
}
