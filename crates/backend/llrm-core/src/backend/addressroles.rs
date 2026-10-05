//! Which of the two registers of a word address is the base and which the index, decided once.
//!
//! `[a + b]` is commutative; the machine takes one from BX and the other from SI or DI. The operand's own `base` and
//! `index` fields carry the choice from here on, and `regclass` reads them as written, so every phase after this one
//! agrees which value is confined to which registers.

use std::collections::BTreeSet;

use crate::analysis::intervals as ranges;
use crate::backend::allocate;
use crate::backend::target::{self, Segments};
use crate::support::hash::IndexMap;
use crate::backend::regclass;
use crate::model::ir::{Loc, Mem, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use std::sync::Arc;

pub struct AddressRoles {
    pub segments: Segments,
}

impl LIRTransform for AddressRoles {
    fn class_name(&self) -> &'static str {
        "AddressRoles"
    }

    fn name(&self) -> &str {
        "address-roles"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(oriented(&body, &self.segments))
    }
}

/// `body` with every word pair's base and index fields in the roles chosen for them.
pub fn oriented(body: &LirBody, segments: &Segments) -> LirBody {
    let chosen = chosen_roles(body, segments);
    let reversed = |cell: &Mem| -> bool {
        let (Some(base), Some(index)) = (cell.base, cell.index) else { return false };
        if base.width != 2 || index.width != 2 || cell.scale != 1 {
            return false;
        }
        chosen.get(&base.value) == Some(&false) && chosen.get(&index.value) == Some(&true)
    };
    let swapped = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) if reversed(cell) => Loc::Mem(Mem { base: cell.index, index: cell.base, ..cell.clone() }),
            other => other.clone(),
        }
    };
    let mut changed = false;
    let blocks: Vec<LirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .map(|one| match &one.what {
                    Some(what) if what.dests.iter().chain(&what.sources).any(|place| matches!(place, Loc::Mem(cell) if reversed(cell))) => {
                        changed = true;
                        let mut made = (**one).clone();
                        made.what = Some(Semantics { dests: what.dests.iter().map(&swapped).collect(), sources: what.sources.iter().map(&swapped).collect(), ..what.clone() });
                        Arc::new(made)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// Each value of a word pair: true where it is the base. Where a component cannot be split in two sides it is absent.
fn chosen_roles(body: &LirBody, segments: &Segments) -> IndexMap<u32, bool> {
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for one in body.insns() {
        let Some(what) = &one.what else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            if let Loc::Mem(cell) = place {
                if let (Some(base), Some(index)) = (cell.base, cell.index) {
                    if base.width == 2 && index.width == 2 && cell.scale == 1 {
                        pairs.push((base.value, index.value));
                    }
                }
            }
        }
    }
    if pairs.is_empty() {
        return IndexMap::default();
    }
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for (base, index) in &pairs {
        adjacent.entry(*base).or_default().insert(*index);
        adjacent.entry(*index).or_default().insert(*base);
    }
    // The sides of each component, where it has two.
    let mut sides: Vec<(BTreeSet<u32>, BTreeSet<u32>)> = Vec::new();
    let mut unseen: BTreeSet<u32> = adjacent.keys().copied().collect();
    while let Some(&seed) = unseen.iter().next() {
        let mut colors: IndexMap<u32, u8> = IndexMap::from_iter([(seed, 0)]);
        let mut work = vec![seed];
        let mut bipartite = true;
        while let Some(value) = work.pop() {
            for other in &adjacent[&value] {
                match colors.get(other) {
                    None => {
                        colors.insert(*other, 1 - colors[&value]);
                        work.push(*other);
                    }
                    Some(color) if *color == colors[&value] => bipartite = false,
                    Some(_) => {}
                }
            }
        }
        for value in colors.keys() {
            unseen.remove(value);
        }
        if bipartite {
            sides.push((
                colors.iter().filter(|(_, color)| **color == 0).map(|(value, _)| *value).collect(),
                colors.iter().filter(|(_, color)| **color != 0).map(|(value, _)| *value).collect(),
            ));
        }
    }
    // The choice today's per-component key makes, to start from; and what else is confined to the base register.
    let decided = regclass::decided_roles(body, &BTreeSet::new(), segments);
    let forced = regclass::unpaired_classes(body, segments);
    let numbered = ranges::indexed(body);
    let live = ranges::intervals(body, Some(&numbered));
    let masks = allocate::_masks(body, &numbered, segments);
    let word_base = *target::WORD_BASES.iter().next().expect("one word base");
    let in_pair: BTreeSet<u32> = adjacent.keys().copied().collect();
    let fixed: Vec<u32> = decided.iter().filter(|(value, class)| !in_pair.contains(value) && class.is_subset(&target::WORD_BASES)).map(|(value, _)| *value).collect();
    // Whether the first side is the base side.
    // A side can take a role only where every value of it may sit in that role's registers.
    let may = |values: &BTreeSet<u32>, choices: &BTreeSet<iced_x86::Register>| values.iter().all(|value| forced.get(value).is_none_or(|class| class.intersection(choices).next().is_some()));
    let mut first_is_base: Vec<bool> = sides
        .iter()
        .map(|(left, right)| {
            let votes = |values: &BTreeSet<u32>| values.iter().filter(|value| decided.get(*value).is_some_and(|class| class.is_subset(&target::WORD_BASES))).count();
            votes(left) >= votes(right)
        })
        .collect();
    // What one set of base values costs: first what a call clobbers (BX does not survive one), then, for each pair live
    // at once, the cheaper of the two, which the one base register must give up.
    let crowd = |values: &[u32]| -> (f64, f64) {
        let across: f64 = values.iter().filter_map(|value| live.get(value)).filter(|interval| allocate::_clobbered(interval, word_base, &masks, 2)).map(|interval| interval.weight).sum();
        let mut together = 0.0;
        for (at, one) in values.iter().enumerate() {
            let Some(mine) = live.get(one) else { continue };
            for other in &values[at + 1..] {
                if live.get(other).is_some_and(|theirs| mine.overlaps(theirs)) {
                    together += mine.weight.min(live[other].weight);
                }
            }
        }
        (across, together)
    };
    let less = |one: (f64, f64), other: (f64, f64)| one.0 < other.0 || (one.0 == other.0 && one.1 < other.1);
    let possible = |at: usize, first: bool| -> bool {
        let (left, right) = &sides[at];
        let (bases, indexes) = if first { (left, right) } else { (right, left) };
        may(bases, &target::WORD_BASES) && may(indexes, &target::WORD_INDEXES)
    };
    let bases = |first_is_base: &[bool]| -> Vec<u32> {
        let mut out: Vec<u32> = fixed.clone();
        for ((left, right), first) in sides.iter().zip(first_is_base) {
            out.extend(if *first { left } else { right }.iter().copied());
        }
        out
    };
    let mut best = crowd(&bases(&first_is_base));
    for _ in 0..4 {
        let mut improved = false;
        for at in 0..sides.len() {
            if !possible(at, !first_is_base[at]) {
                continue;
            }
            first_is_base[at] = !first_is_base[at];
            let tried = crowd(&bases(&first_is_base));
            if less(tried, best) {
                best = tried;
                improved = true;
            } else {
                first_is_base[at] = !first_is_base[at];
            }
        }
        if !improved {
            break;
        }
    }
    let mut roles: IndexMap<u32, bool> = IndexMap::default();
    for ((left, right), first) in sides.iter().zip(&first_is_base) {
        for value in left {
            roles.insert(*value, *first);
        }
        for value in right {
            roles.insert(*value, !*first);
        }
    }
    roles
}
