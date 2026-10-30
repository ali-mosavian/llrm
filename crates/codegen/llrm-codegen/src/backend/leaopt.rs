//! Two address computations that differ only in a displacement are one:
//! LLVM's `X86OptimizeLEAs`
//! ([X86OptimizeLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86OptimizeLEAs.cpp)).
//!
//! # How it works
//! Before phi elimination, each block's `lea d, [base + index*scale + k]` are
//! listed by everything but `k` (`findLEAs`, `getMemOpKey`). Two steps follow.
//!
//! `removeRedundantLEAs`: a later LEA of a list goes when every reader of its
//! value is an instruction that has the value as the base of its one address
//! and nowhere else (`isReplaceable`); the readers then read the earlier LEA's
//! value, their displacement shifted by the difference of the two.
//!
//! `removeRedundantAddrCalc`, in a build for size only: a load or store whose
//! address is the list's address plus another displacement is made through the
//! LEA's value (`chooseBestLEA`: the displacement fits an int32, the closest
//! LEA, one before the access if there is one, one whose displacement is a
//! byte if another's is not). A LEA after the access is lifted to just before
//! it.
//!
//! ```text
//!   v1 := lea [v0+8]            v1 := lea [v0+8]
//!   v2 := lea [v0+24]           mov v3, [v1]
//!   mov v3, [v1]        ==>     mov v4, [v1+16]
//!   mov v4, [v2]
//! ```
//!
//! DEVIATE LO-1 (exit: displacements of a symbol or a frame cell shifted by
//! `Addr::plus`, owner w-6, 2026-11-09): only an address of registers and a
//! number is a candidate; the pin also takes a global, a constant-pool entry
//! or a symbol as the displacement.
//!
//! DEVIATE LO-2 (exit: the wrap of a 16-bit offset proved, owner w-6,
//! 2026-11-09): only a body of 32-bit addresses is rewritten; a displacement
//! moved between two 16-bit sums can change what wraps.
//!
//! DEVIATE LO-3 (no number): the first step runs over the whole body before
//! the second runs over any block; the pin does both for one block, then the
//! next. A block laid out before the one whose LEA is removed sees the
//! second step before the readers change.
//!
//! # Example
//! The unit tests `a_later_lea_of_the_same_address_is_the_earlier_one` and
//! `a_size_build_makes_an_access_through_the_lea`.
//!
//! # Runs
//! A machine phase after `machinesink` and before `phielim`
//! (`flow::machine_with`), at every -O level but -O0.

use std::sync::Arc;

use crate::model::ir::{Held, Loc, Mem, Operation};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap};

pub struct OptimizeLeas {
    /// A build for size (`shouldOptimizeForSize`).
    pub size: bool,
}

impl LIRTransform for OptimizeLeas {
    fn class_name(&self) -> &'static str {
        "X86OptimizeLEAs"
    }

    fn name(&self) -> &str {
        "leaopt"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(optimized(&body, self.size))
    }
}

/// What a list is keyed by: every part of an address but its displacement.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Key {
    base: Held,
    index: Option<Held>,
    scale: i64,
}

/// A `lea` the pass takes: `d := lea [base + index*scale + offset]`.
#[derive(Clone, Copy, Debug)]
struct Lea {
    value: u32,
    width: u32,
    key: Key,
    offset: i64,
}

/// An address the pass takes: registers and a number, no symbol, segment or
/// stack argument (LO-1).
fn plain(cell: &Mem) -> Option<Key> {
    (cell.addr.is_none() && cell.selector.is_none() && !cell.stack_argument)
        .then_some(())
        .and(cell.base)
        .map(|base| Key { base, index: cell.index, scale: cell.scale })
}

/// An instruction no one tied to a register, group or call.
fn free(one: &Insn) -> bool {
    one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
}

/// `isLEA`: `d := lea [address]` of an address `plain` takes.
fn lea_of(one: &Insn) -> Option<Lea> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(dest)], [Loc::Mem(cell)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
        return None;
    };
    (what.op == Operation::Address && what.name.as_deref() == Some("lea") && free(one) && one.defines == [dest.value])
        .then(|| plain(cell).map(|key| Lea { value: dest.value, width: dest.width, key, offset: cell.offset }))
        .flatten()
}

/// The one address an instruction other than a LEA reads or writes
/// (`getMemoryOperandIdx`, `mayLoadOrStore`), as `(is a source, index)`.
fn access_of(one: &Insn) -> Option<(bool, usize, Key)> {
    let what = one.what.as_ref()?;
    if what.op == Operation::Address || !free(one) {
        return None;
    }
    let cells: Vec<(bool, usize, &Mem)> = what
        .sources
        .iter()
        .enumerate()
        .map(|(at, place)| (true, at, place))
        .chain(what.dests.iter().enumerate().map(|(at, place)| (false, at, place)))
        .filter_map(|(source, at, place)| match place {
            Loc::Mem(cell) => Some((source, at, cell)),
            _ => None,
        })
        .collect();
    let [(source, at, cell)] = cells.as_slice() else { return None };
    plain(cell).map(|key| (*source, *at, key))
}

fn fits_int32(value: i64) -> bool {
    i32::try_from(value).is_ok()
}

fn fits_int8(value: i64) -> bool {
    i8::try_from(value).is_ok()
}

/// The cell of `place`, which must be one.
fn cell_mut(
    what: &mut crate::model::ir::Semantics,
    source: bool,
    at: usize,
) -> &mut Mem {
    let place = if source { &mut what.sources[at] } else { &mut what.dests[at] };
    match place {
        Loc::Mem(cell) => cell,
        _ => unreachable!("access_of found a cell here"),
    }
}

/// `body` with the redundant address computations removed.
pub fn optimized(
    body: &LirBody,
    size: bool,
) -> LirBody {
    // LO-2.
    if body.bits != 32 {
        return body.clone();
    }
    let first = redundant_leas(body);
    if !size {
        return first;
    }
    redundant_addresses(&first)
}

/// `removeRedundantLEAs` over every block.
fn redundant_leas(body: &LirBody) -> LirBody {
    // Where each value is read: an instruction, or a phi (never replaceable).
    let mut readers: HashMap<u32, Vec<Option<Arc<Insn>>>> = HashMap::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            for value in &one.uses {
                readers.entry(*value).or_default().push(Some(Arc::clone(one)));
            }
        }
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                readers.entry(value).or_default().push(None);
            }
        }
    }
    // The value each removed LEA is read as, and the displacement it adds.
    let mut rename: HashMap<u32, (u32, i64)> = HashMap::default();
    let mut removed: HashSet<u64> = HashSet::default();
    for block in &body.blocks {
        let mut lists: IndexMap<Key, Vec<(Lea, Arc<Insn>)>> = IndexMap::default();
        for one in block.insns.iter() {
            if let Some(lea) = lea_of(one) {
                lists.entry(lea.key).or_default().push((lea, Arc::clone(one)));
            }
        }
        for list in lists.values() {
            let mut gone: HashSet<u32> = HashSet::default();
            for (at, (earlier, _)) in list.iter().enumerate() {
                if gone.contains(&earlier.value) {
                    continue;
                }
                for (later, later_insn) in &list[at + 1..] {
                    if gone.contains(&later.value) {
                        continue;
                    }
                    let shift = later.offset - earlier.offset;
                    if earlier.width != later.width || !replaceable(&readers, later, shift) {
                        continue;
                    }
                    rename.insert(later.value, (earlier.value, shift));
                    removed.insert(later_insn.id());
                    gone.insert(later.value);
                    let merged = readers.remove(&later.value).unwrap_or_default();
                    readers.entry(earlier.value).or_default().extend(merged);
                }
            }
        }
    }
    if removed.is_empty() {
        return body.clone();
    }
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .filter(|one| !removed.contains(&one.id()))
                .map(|one| shifted(one, &rename))
                .collect();
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

/// `isReplaceable`'s use check: every reader of `later` has it as the base of
/// its one address and nowhere else, and the shifted displacement fits.
fn replaceable(
    readers: &HashMap<u32, Vec<Option<Arc<Insn>>>>,
    later: &Lea,
    shift: i64,
) -> bool {
    let Some(all) = readers.get(&later.value) else { return true };
    all.iter().all(|reader| {
        let Some(one) = reader else { return false };
        let Some(what) = one.what.as_ref() else { return false };
        let named = what
            .dests
            .iter()
            .chain(&what.sources)
            .flat_map(|place| llrm_lir::values(place))
            .filter(|held| held.value == later.value)
            .count();
        let cells: Vec<&Mem> = what
            .dests
            .iter()
            .chain(&what.sources)
            .filter_map(|place| match place {
                Loc::Mem(cell) => Some(cell),
                _ => None,
            })
            .collect();
        let [cell] = cells.as_slice() else { return false };
        named == 1
            && cell.base.is_some_and(|base| base.value == later.value)
            && cell.addr.is_none()
            && fits_int32(cell.offset + shift)
    })
}

/// `one` with the values `rename` moves read as the new ones, and the address
/// that read one displaced by the shift.
fn shifted(
    one: &Arc<Insn>,
    rename: &HashMap<u32, (u32, i64)>,
) -> Arc<Insn> {
    let moved = |value: u32| rename.get(&value).map_or(value, |&(first, _)| first);
    let renamed = one.renamed(&moved);
    let Some(what) = one.what.as_ref() else { return renamed };
    let displaced = |place: &Loc| match place {
        Loc::Mem(cell) => cell.base.and_then(|base| rename.get(&base.value)).map(|&(_, shift)| shift),
        _ => None,
    };
    let shifts: Vec<(bool, usize, i64)> = what
        .sources
        .iter()
        .enumerate()
        .map(|(at, place)| (true, at, place))
        .chain(what.dests.iter().enumerate().map(|(at, place)| (false, at, place)))
        .filter_map(|(source, at, place)| displaced(place).map(|shift| (source, at, shift)))
        .collect();
    if shifts.is_empty() {
        return renamed;
    }
    let mut made = (*renamed).clone();
    let mut semantics = made.what.take().expect("renamed keeps what");
    for (source, at, shift) in shifts {
        cell_mut(&mut semantics, source, at).offset += shift;
    }
    made.what = Some(semantics);
    Arc::new(made)
}

/// `removeRedundantAddrCalc` over every block.
fn redundant_addresses(body: &LirBody) -> LirBody {
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let insns = through_leas(block.insns.iter().map(Arc::clone).collect());
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

/// One block's loads and stores made through the LEAs that name their
/// address.
fn through_leas(mut insns: Vec<Arc<Insn>>) -> Vec<Arc<Insn>> {
    let mut lists: IndexMap<Key, Vec<Lea>> = IndexMap::default();
    for one in &insns {
        if let Some(lea) = lea_of(one) {
            lists.entry(lea.key).or_default().push(lea);
        }
    }
    if lists.is_empty() {
        return insns;
    }
    // `InstrPos`: two apart, so a lifted LEA has a free place before its user.
    let mut position: HashMap<u32, i64> = HashMap::default();
    for (at, one) in insns.iter().enumerate() {
        if let Some(lea) = lea_of(one) {
            position.insert(lea.value, 2 * (at as i64 + 1));
        }
    }
    let mut at = 0;
    while at < insns.len() {
        let user = Arc::clone(&insns[at]);
        let Some((source, place, key)) = access_of(&user) else {
            at += 1;
            continue;
        };
        let Some(list) = lists.get(&key) else {
            at += 1;
            continue;
        };
        let offset = match user.what.as_ref().and_then(|what| {
            let cell = if source { &what.sources[place] } else { &what.dests[place] };
            match cell {
                Loc::Mem(cell) => Some(cell.offset),
                _ => None,
            }
        }) {
            Some(offset) => offset,
            None => {
                at += 1;
                continue;
            }
        };
        let here = 2 * (at as i64 + 1);
        // `chooseBestLEA`.
        let mut best: Option<(Lea, i64, i64)> = None;
        for lea in list {
            let shift = offset - lea.offset;
            if !fits_int32(shift) || lea.width != key.base.width {
                continue;
            }
            let distance = here - position[&lea.value];
            if distance > 0 || best.is_none() {
                if let Some((_, kept, _)) = best
                    && !fits_int8(shift)
                    && fits_int8(kept)
                {
                    continue;
                }
                best = Some((*lea, shift, distance));
            }
            if distance < 0 {
                break;
            }
        }
        let Some((lea, shift, distance)) = best else {
            at += 1;
            continue;
        };
        let mut lifted = 0;
        if distance < 0 {
            let from = insns.iter().position(|one| one.defines == [lea.value]).expect("the LEA is in the block");
            let mut moved = (*insns[from]).clone();
            moved.at = user.at;
            insns.remove(from);
            insns.insert(at, Arc::new(moved));
            position.insert(lea.value, here - 1);
            lifted = 1;
        }
        let user_at = at + lifted;
        insns[user_at] = through(&user, source, place, lea, shift);
        at = user_at + 1;
    }
    insns
}

/// `user` with its address `[lea + shift]`.
fn through(
    user: &Arc<Insn>,
    source: bool,
    place: usize,
    lea: Lea,
    shift: i64,
) -> Arc<Insn> {
    let mut made = (**user).clone();
    let mut what = made.what.take().expect("an access has semantics");
    {
        let cell = cell_mut(&mut what, source, place);
        cell.base = Some(Held { value: lea.value, width: lea.width });
        cell.index = None;
        cell.scale = 1;
        cell.offset = shift;
        cell.exact = false;
    }
    let named: HashSet<u32> = what
        .dests
        .iter()
        .chain(&what.sources)
        .flat_map(|place| llrm_lir::values(place))
        .map(|held| held.value)
        .collect();
    let mut uses: Vec<u32> = made.uses.iter().copied().filter(|value| named.contains(value)).collect();
    if !uses.contains(&lea.value) {
        uses.push(lea.value);
    }
    made.uses = uses;
    made.widths.retain(|(value, _)| named.contains(value));
    made.what = Some(what);
    Arc::new(made)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::optimized;
    use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Held {
        Held { value, width: 4 }
    }

    fn cell(
        base: u32,
        offset: i64,
    ) -> Mem {
        Mem { base: Some(held(base)), offset, ..Mem::new(None, 4) }
    }

    fn lea(
        at: i64,
        value: u32,
        base: u32,
        offset: i64,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 3)),
            Some(Semantics {
                name: Some("lea".into()),
                dests: vec![Loc::Held(held(value))],
                sources: vec![Loc::Mem(cell(base, offset))],
                ..Semantics::new(Operation::Address)
            }),
            vec![value],
            vec![base],
        ))
    }

    fn load(
        at: i64,
        value: u32,
        base: u32,
        offset: i64,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 3)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(held(value))],
                sources: vec![Loc::Mem(cell(base, offset))],
                ..Semantics::new(Operation::Move)
            }),
            vec![value],
            vec![base],
        ))
    }

    /// `value := value + other`: reads the LEA's value, not through an address.
    fn sum(
        at: i64,
        value: u32,
        read: u32,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 2)),
            Some(Semantics {
                name: Some("add".into()),
                dests: vec![Loc::Held(held(value))],
                sources: vec![Loc::Held(held(read)), Loc::Held(held(read))],
                ..Semantics::new(Operation::Binary)
            }),
            vec![value],
            vec![read],
        ))
    }

    fn body(
        insns: Vec<Arc<Insn>>,
        bits: u32,
    ) -> LirBody {
        let block: Block = LirBlock::new(0, insns).into();
        LirBody { bits, ..LirBody::new("leaopt", 0, vec![block], IndexMap::default(), IndexMap::default()) }
    }

    fn shape(result: &LirBody) -> Vec<(String, Option<u32>, i64)> {
        result.blocks[0]
            .insns
            .iter()
            .map(|one| {
                let what = one.what.as_ref().unwrap();
                let cell = what
                    .sources
                    .iter()
                    .chain(&what.dests)
                    .find_map(
                        |place| match place {
                            Loc::Mem(cell) => Some(cell),
                            _ => None,
                        },
                    );
                (
                    what.name.as_deref().map(str::to_owned).unwrap(),
                    cell.and_then(|cell| cell.base).map(|base| base.value),
                    cell.map_or(0, |cell| cell.offset),
                )
            })
            .collect()
    }

    /// `lea [v0+8]` then `lea [v0+24]` read only as an address base: the
    /// second goes and its reader reads `[v1+20]`. Without the pass both LEAs
    /// stayed: 2 instructions the pin has 1 of.
    #[test]
    fn a_later_lea_of_the_same_address_is_the_earlier_one() {
        let result = optimized(&body(vec![lea(0, 1, 0, 8), lea(3, 2, 0, 24), load(6, 3, 2, 4)], 32), false);
        assert_eq!(shape(&result), [("lea".into(), Some(0), 8), ("mov".into(), Some(1), 20)]);
        assert_eq!(result.blocks[0].insns[1].uses, [1]);
    }

    /// A reader of the value that is not an address base keeps the LEA.
    #[test]
    fn a_lea_read_as_a_value_stays() {
        let result = optimized(&body(vec![lea(0, 1, 0, 8), lea(3, 2, 0, 24), sum(6, 3, 2)], 32), false);
        assert_eq!(result.blocks[0].insns.len(), 3);
    }

    /// The second step is for size only.
    #[test]
    fn an_access_is_made_through_a_lea_only_in_a_size_build() {
        let input = body(vec![lea(0, 1, 0, 8), load(3, 3, 0, 24)], 32);
        assert_eq!(shape(&optimized(&input, false))[1], ("mov".into(), Some(0), 24));
        assert_eq!(shape(&optimized(&input, true))[1], ("mov".into(), Some(1), 16));
    }

    /// A LEA after the access is lifted to before it.
    #[test]
    fn a_lea_after_the_access_is_lifted_in_a_size_build() {
        let result = optimized(&body(vec![load(0, 3, 0, 24), lea(3, 1, 0, 8)], 32), true);
        assert_eq!(shape(&result), [("lea".into(), Some(0), 8), ("mov".into(), Some(1), 16)]);
    }

    /// A body of 16-bit addresses is left alone (LO-2).
    #[test]
    fn a_body_of_16_bit_addresses_is_left() {
        let input = body(vec![lea(0, 1, 0, 8), lea(3, 2, 0, 24), load(6, 3, 2, 4)], 16);
        assert_eq!(optimized(&input, true).blocks[0].insns.len(), 3);
    }
}
