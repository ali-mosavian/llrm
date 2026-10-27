//! Select a far pointer's two word loads as one `les`-form load.
//!
//! # How it works
//! A far-pointer field is two word loads in the language but one
//! instruction on the target. Once allocation has put the offset and the
//! selector in unrelated registers the pair can no longer be joined, so
//! this runs first, on virtual registers.
//!
//! ```text
//!   mov  v1, [p]                     les  v1:v2, [p]
//!   mov  v2, [p+2]           =>      (second load becomes an anchor)
//!                                    ... [v2:v1] used as a far cell
//! ```
//!
//! [`selectors`] first finds the values that are read only as a memory cell's
//! selector: never as a number, a register argument, a phi input or a base
//! or index. Only such a second value may become the far segment register.
//!
//! [`selected`] then walks adjacent instruction pairs. Both must be plain
//! `mov reg,[mem]` loads (no clobber, constraint, group, relocation,
//! spill marker or volatile access), the second cell must be the word right
//! after the first (same registers, displacement or offset 2 higher), the
//! second destination must be in the selector set, and the cell must
//! name at least one virtual register (a fixed address survives allocation
//! unchanged) that neither load defines. The pair becomes one four-byte
//! load named by the target's far-segment load form; the second load
//! turns into an anchor so source ownership is kept.
//!
//! # Example
//! `world->words[i]` and `rdr->flags[...]` load far pointer fields of near
//! structs. No dump prints the body before this pass, so this is its result:
//! each pointer is one `les` defining the offset and the selector (`v11, v12
//! :=`), followed by the anchor the second word load became. The first stage
//! dump of `mark`:
//!
//! ```lir
//! _mark@3: 26 instructions
//!   ; ...
//!   block 0x0007
//!     ; ...
//!     0x0009  v11, v12 := les [None v1@unplaced]  ; inserted
//!     0x0009     ; inserted
//!     0x000b  v13 := mov [[es:r0+0x0] v11@unplaced]  ; inserted
//!     0x000c  v14, v15 := les [None v2@unplaced]  ; inserted
//!     0x000c     ; inserted
//! ```
//!
//! From `ISEL_DUMP=1` of `tests/fixtures/c/farloadloop.c` with `-m16 -O2`.
//!
//! # Runs
//! Per block in isel's cross-instruction selection (`isel/combined.rs`),
//! first, before [`crate::backend::comparefold`]. Every optimization level.
//!
//! # See also
//! The phase map: [`crate::backend`]

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{self, Insn};
use crate::support::hash::{IndexMap, IndexSet};

/// Select private `mov offset,[p]; mov selector,[p+2]` pairs as `les`.
pub fn selected(
    regs: Regs,
    insns: &[Arc<Insn>],
    selectors: &BTreeSet<u32>,
) -> Vec<Arc<Insn>> {
    let mut made: IndexMap<usize, Arc<Insn>> = IndexMap::default();
    let mut erased: BTreeSet<usize> = BTreeSet::new();
    for (at, first) in insns[..insns.len().saturating_sub(1)].iter().enumerate() {
        if erased.contains(&at) {
            continue;
        }
        let second = &insns[at + 1];
        let Some(joined) = _pair(regs, first, second, selectors) else {
            continue;
        };
        made.insert(at, joined);
        erased.insert(at + 1);
    }
    insns
        .iter()
        .enumerate()
        .map(|(at, one)| {
            if let Some(joined) = made.get(&at) {
                Arc::clone(joined)
            } else if erased.contains(&at) {
                let mut anchored = (*lir::anchor(Arc::clone(one))).clone();
                anchored.defines = Vec::new();
                anchored.uses = Vec::new();
                anchored.widths = Vec::new();
                Arc::new(anchored)
            } else {
                Arc::clone(one)
            }
        })
        .collect()
}

fn _pair(
    regs: Regs,
    first: &Insn,
    second: &Insn,
    selectors: &BTreeSet<u32>,
) -> Option<Arc<Insn>> {
    if !_plain(first)
        || !_plain(second)
        || first.covers.is_some_and(|covers| covers.0 != covers.1)
        || second.covers.is_some_and(|covers| covers.0 != covers.1)
    {
        return None;
    }
    let mut words: Vec<(Held, Mem)> = Vec::new();
    for one in [first, second] {
        match &one.what {
            Some(what)
                if what.op == Operation::Move
                    && what.name.as_deref() == Some("mov")
                    && matches!(what.dests.as_slice(), [Loc::Held(_)])
                    && matches!(what.sources.as_slice(), [Loc::Mem(_)]) =>
            {
                let (Loc::Held(dest), Loc::Mem(cell)) = (&what.dests[0], &what.sources[0]) else { unreachable!() };
                // Python's chained `dest.width != cell.width != 2`.
                if dest.width != cell.width && cell.width != 2 {
                    return None;
                }
                words.push((*dest, cell.clone()));
            }
            _ => return None,
        }
    }
    let (first_dest, first_cell) = words[0].clone();
    let (second_dest, second_cell) = words[1].clone();
    if !_next_word(&first_cell, &second_cell)
        || !selectors.contains(&second_dest.value)
        || first.volatile()
        || second.volatile()
    {
        return None;
    }
    // A fixed address survives allocation unchanged; only a virtual
    // base/selector/index needs the early complete-load selection.
    if ir::values(&Loc::Mem(first_cell.clone())).is_empty() {
        return None;
    }
    let defined: BTreeSet<u32> = [first_dest.value, second_dest.value].into_iter().collect();
    if [&first_cell, &second_cell]
        .into_iter()
        .any(|cell| ir::values(&Loc::Mem(cell.clone())).iter().any(|value| defined.contains(&value.value)))
    {
        return None;
    }
    // Spelt as the load of the far segment; the rewriter respells it for the
    // segment register the selector is given.
    let what = Semantics {
        name: Some(
            regs.load_form(regs.far()).expect("a target with a far segment states the load that fills it").into(),
        ),
        dests: vec![Loc::Held(first_dest), Loc::Held(second_dest)],
        sources: vec![Loc::Mem(Mem { width: 4, ..first_cell })],
        ..Semantics::new(Operation::Move)
    };
    let mut joined = first.clone();
    joined.what = Some(what);
    joined.defines =
        first.defines.iter().chain(&second.defines).copied().collect::<IndexSet<u32>>().into_iter().collect();
    joined.uses = first.uses.iter().chain(&second.uses).copied().collect::<IndexSet<u32>>().into_iter().collect();
    Some(Arc::new(joined))
}

pub(crate) fn _plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.symbol == Some(true)
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.rematerialized)
}

/// Whether `high` is the word immediately after `low` by one address.
pub(crate) fn _next_word(
    low: &Mem,
    high: &Mem,
) -> bool {
    let same = |cell: &Mem| Mem { addr: cell.addr.map(|addr| ir::Addr { disp: 0, ..addr }), offset: 0, ..cell.clone() };
    if same(low) != same(high) {
        return false;
    }
    let (Some(low_addr), Some(high_addr)) = (low.addr, high.addr) else {
        return low.addr.is_none() && high.addr.is_none() && high.offset == low.offset + 2;
    };
    let moved = high_addr.disp - low_addr.disp;
    moved == 2 && [0, 2].contains(&(high.offset - low.offset)) || moved == 0 && high.offset == low.offset + 2
}

/// Values read only as a cell's selector: what makes a far load the right load.
///
/// `kept` holds values a phi reads, which no instruction here shows.
pub fn selectors(
    made: &IndexMap<i64, Vec<Arc<Insn>>>,
    kept: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let mut selecting: BTreeSet<u32> = BTreeSet::new();
    let mut numeric: BTreeSet<u32> = kept.clone();
    for block in made.values() {
        for one in block {
            numeric.extend(one.requires.iter().chain(&one.delivers).map(|(held, _)| held.value));
            let Some(what) = &one.what else {
                continue;
            };
            for r#where in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = r#where {
                    numeric.extend([cell.base, cell.index].into_iter().flatten().map(|held| held.value));
                    if let Some(selector) = cell.selector {
                        selecting.insert(selector.value);
                    }
                }
            }
            numeric.extend(what.sources.iter().filter_map(|r#where| match r#where {
                Loc::Held(held) => Some(held.value),
                _ => None,
            }));
        }
    }
    selecting.difference(&numeric).copied().collect()
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_farload.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::selected;
    use crate::model::ir::{self, Addr, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::Insn;

    fn load(
        at: i64,
        value: u32,
        cell: Mem,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let destination = ir::Held { value, width: 2 };
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(destination)],
                sources: vec![Loc::Mem(cell)],
                ..Semantics::new(Operation::Move)
            }),
            vec![value],
            uses,
        ))
    }

    #[test]
    fn test_adjacent_words_are_one_far_load_only_when_the_high_word_is_a_selector() {
        let regs = crate::backend::registerinfo::test_regs();
        // qcport's dynamic far-struct fields emitted two loads per far pointer.
        let cell = |disp: i64| Mem {
            base: Some(ir::Held { value: 10, width: 2 }),
            selector: Some(ir::Held { value: 11, width: 2 }),
            ..Mem::new(Some(Addr::new(Space::Far, disp)), 2)
        };
        let values = |cell: &Mem| ir::values(&Loc::Mem(cell.clone())).iter().map(|one| one.value).collect();
        let (first_cell, second_cell) = (cell(4), cell(6));
        let original = vec![
            load(1, 1, first_cell.clone(), values(&first_cell)),
            load(2, 2, second_cell.clone(), values(&second_cell)),
        ];

        let selected = selected(regs, &original, &BTreeSet::from([2]));

        assert_eq!(selected[0].what.as_ref().unwrap().name.as_deref(), Some("les"));
        let Loc::Mem(source) = &selected[0].what.as_ref().unwrap().sources[0] else { panic!() };
        assert_eq!(source.width, 4);
        assert_eq!(selected[0].defines, vec![1, 2]);
        assert_eq!(selected[1].what.as_ref().unwrap().op, Operation::Nothing);
        // Adjacent words whose high one is a number, not a selector, stay two
        // loads.
        assert_eq!(super::selected(regs, &original, &BTreeSet::new()), original);
    }

    #[test]
    fn test_fixed_far_pointer_load_defers_fusion_until_after_allocation() {
        let regs = crate::backend::registerinfo::test_regs();
        // indexed.lru_use's fixed parameter pair became an eager LES.
        let load = |at: i64, value: u32, displacement: i64| {
            load(at, value, Mem::new(Some(Addr::new(Space::Frame, displacement)), 2), Vec::new())
        };

        let original = vec![load(1, 1, 4), load(2, 2, 6)];

        assert_eq!(selected(regs, &original, &BTreeSet::from([2])), original);
    }
}
