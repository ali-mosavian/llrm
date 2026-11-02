//! Finds the memory cells that provably still hold a phi's value, so a spilled
//! value read from one needs no store and no slot of its own.
//!
//! # How it works
//! MIR proves, for a phi, that a memory cell already holds its value
//! (`!llrm.home`, carried into the body as `LirBody.homes`). A spilled value
//! read from such a cell needs no store and no slot of its own. This module
//! re-checks the proof against the machine code, since later phases may have
//! changed it:
//!
//! ```text
//!  held(body, wanted):
//!    homes the body names, filtered by the caller's `wanted`
//!      drop a value with a definition after which the cell is behind
//!        (neither a copy nor made after the block's last store to the cell)
//!      walk each block backward with the values live there
//!        drop a value when an instruction may write its cell
//!        and is not itself a store of that value to the cell
//! ```
//!
//! `may_write` is the spiller's own test for the frame, plus any store to other
//! memory that is not certainly elsewhere. `apart` says two cells are certainly
//! different bytes: both are fixed addresses that `overlap` separates. A
//! pointer, or two elements of an array, are never apart; what a pointer
//! reaches is the optimizer's proof, not checked here.
//!
//! # Example
//! No honest example: `held`, `may_write` and `apart` answer queries and print
//! no LIR of their own. Read the spiller's `ISEL_DUMP=1` output (see
//! [`crate::backend::lirtext`]) to see a spilled phi read from its home cell.
//!
//! # Used by
//! the greedy shadow driver (`held`, floating values only);
//! [`crate::backend::floatfold`] (`may_write`); [`crate::backend::postrasink`]
//! (`apart`).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::backend::bodyfacts::_may_write;
use crate::backend::liverows::live_rows_by;
use crate::backend::overlap;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// Whether `cell` and `other` are certainly different bytes: both fixed
/// addresses (no register in either) that `overlap` tells apart. The machine
/// cannot tell two pointers or two elements of one array apart, nor a pointer
/// from a global (`overlap`'s rule: that is MIR's to say), so anything else may
/// overlap.
pub(crate) fn apart(
    cell: &Mem,
    other: &Mem,
) -> bool {
    let fixed = |one: &Mem| one.base.is_none() && one.index.is_none() && one.selector.is_none() && one.addr.is_some();
    fixed(cell) && fixed(other) && !overlap::may_overlap(cell.addr, cell.width, other.addr, other.width)
}

/// Whether `one` stores to bytes of `cell` by a fixed address: the same global,
/// the same frame cell.
fn writes_there(
    one: &Insn,
    cell: &Mem,
) -> bool {
    let fixed = |one: &Mem| one.base.is_none() && one.index.is_none() && one.selector.is_none() && one.addr.is_some();
    fixed(cell)
        && one.what.as_ref().is_some_and(|what| {
            what.dests.iter().any(|place| matches!(place, Loc::Mem(other) if fixed(other) && !apart(cell, other)))
        })
}

/// Whether `one` may write a byte of `cell`, where nothing else vouches for
/// what a pointer reaches: the spiller's answer for the frame, and for any
/// other memory a store that is not certainly elsewhere. (The spiller's alone
/// said no to a store to the very global a load read: nothing in it but frame
/// cells was asked.)
pub fn may_write(
    one: &Insn,
    cell: &Mem,
    sealed: bool,
) -> bool {
    _may_write(one, cell, sealed)
        || one
            .what
            .as_ref()
            .is_some_and(|what| what.dests.iter().any(|place| matches!(place, Loc::Mem(other) if !apart(cell, other))))
}

/// Whether `one` only copies one value to another.
fn copies(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(what.op, Operation::Move | Operation::FloatLoad)
                && matches!(
                    (what.dests.as_slice(), what.sources.as_slice()),
                    ([Loc::Held(_)], [Loc::Held(_)])
                )
                && one.clobbers.is_empty()
                && one.delivers.is_empty()
                && one.requires.is_empty(),
        )
}

/// Whether `one` stores `value` to `cell`: it leaves in the cell what it holds.
fn stores(
    one: &Insn,
    value: u32,
    cell: &Mem,
) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(what.op, Operation::Move | Operation::FloatStore)
                && matches!(what.dests.as_slice(), [Loc::Mem(dest)] if dest == cell)
                && matches!(
                    what.sources.as_slice(),
                    [Loc::Held(held)] if held.value == value
                ),
        )
}

/// Whether `one` may read a byte of `cell`: a call, or a load that is not
/// certainly elsewhere.
fn reads_there(
    one: &Insn,
    cell: &Mem,
) -> bool {
    one.call.is_some()
        || one.what.as_ref().is_some_and(|what| {
            what.sources.iter().any(|place| matches!(place, Loc::Mem(other) if !apart(cell, other)))
        })
}

/// Whether `cell` is behind `value` for no instruction that matters after its
/// definition at `at`, so that the cell can be the value's spill slot: the
/// definition is a copy (the source was stored, MIR's proof), or the block
/// stores the value to the cell before anything may read the cell, or it
/// last wrote the cell by a plain store before a definition that reads nothing
/// of the value (the phi's input). A reader between a definition and the store
/// would see the value early.
fn _cell_holds_after(
    body: &LirBody,
    at: (u32, u32),
    value: u32,
    cell: &Mem,
) -> bool {
    let insns = &body.blocks[at.0 as usize].insns;
    let one = &insns[at.1 as usize];
    if copies(one) {
        return true;
    }
    let behind = insns[at.1 as usize + 1..]
        .iter()
        .try_for_each(|next| {
            if stores(next, value, cell) {
                Err(true)
            } else if reads_there(next, cell) || writes_there(next, cell) {
                Err(false)
            } else {
                Ok(())
            }
        })
        .err();
    if let Some(closed) = behind {
        return closed;
    }
    if one.uses.contains(&value) {
        return false;
    }
    // That the plain store wrote the bits the definition makes is MIR's proof
    // (`!llrm.home`: each input of the phi was stored to the cell); nothing
    // here compares them.
    // The last write to the cell before the definition, back along the blocks
    // that have one predecessor.
    let mut block = at.0 as usize;
    let mut end = at.1 as usize;
    for _ in 0..body.blocks.len() {
        let found = body.blocks[block].insns[..end]
            .iter()
            .rev()
            .find(|before| writes_there(before, cell) || _may_write(before, cell, body.sealed_arguments));
        if let Some(last) = found {
            return last
                .what
                .as_ref()
                .is_some_and(|what| matches!(what.dests.as_slice(), [Loc::Mem(dest)] if dest == cell));
        }
        let mut before = body.blocks.iter().enumerate().filter(|(_, from)| from.succ.contains(&body.blocks[block].at));
        let (Some((only, _)), None) = (before.next(), before.next()) else { return false };
        (block, end) = (only, body.blocks[only].insns.len());
    }
    false
}

/// Whether the instructions `after` a definition of `spilled` store the value
/// it copies (`values`) to `cell` before anything reads or writes the cell or
/// redefines the value: a store of it to the cell now is overwritten by the
/// same bytes.
pub fn stored_again(
    after: &[Arc<Insn>],
    cell: &Mem,
    mut values: Vec<u32>,
    spilled: u32,
) -> bool {
    for next in after {
        if stores_any(next, &values, cell) {
            return true;
        }
        // A read of the spilled value in between is reloaded from the cell,
        // which the store has not reached.
        if reads_there(next, cell)
            || writes_there(next, cell)
            || next.uses.contains(&spilled)
            || next.defines.iter().any(|value| values.contains(value))
        {
            return false;
        }
        // A copy of the value is the value.
        if let Some((to, from)) = next.copy_of()
            && values.contains(&from)
        {
            values.push(to);
        }
    }
    false
}

fn stores_any(
    one: &Insn,
    values: &[u32],
    cell: &Mem,
) -> bool {
    values.iter().any(|value| stores(one, *value, cell))
}

/// Each value `wanted` accepts that the body's homes name and that still holds:
/// its cell.
pub fn held(
    body: &LirBody,
    wanted: &dyn Fn(u32, &Mem) -> bool,
) -> IndexMap<u32, Mem> {
    let mut found: IndexMap<u32, Mem> = body
        .homes
        .iter()
        .filter(|(value, cell)| wanted(**value, cell))
        .map(|(value, cell)| (*value, cell.clone()))
        .collect();
    if found.is_empty() {
        return found;
    }
    // Only these values are looked at, where they occur: every instruction of
    // the body was walked, and the liveness of every value solved, afresh
    // for each spill round to read the few homes in it.
    let asked: BTreeSet<u32> = found.keys().copied().collect();
    // Every definition is a copy.
    let reads = crate::backend::postings::following(body, |postings| {
        let mut reads: BTreeSet<usize> = BTreeSet::new();
        for &value in &asked {
            let cell = &found[&value];
            if postings.defs(value).iter().any(|at| !_cell_holds_after(body, *at, value, cell)) {
                found.shift_remove(&value);
            }
            reads.extend(postings.uses(value).iter().map(|at| at.0 as usize));
        }
        reads
    });
    let live_out = live_rows_by(body, |value| asked.contains(&value));
    for (position, block) in body.blocks.iter().enumerate() {
        let mut alive: BTreeSet<u32> = live_out.leaving(block.at).filter(|value| found.contains_key(value)).collect();
        // Nothing of these values is live at the exit or read here: no
        // instruction changes anything.
        if alive.is_empty() && !reads.contains(&position) {
            continue;
        }
        for one in block.insns.iter().rev() {
            // What a pointer reaches is the optimizer's proof; what is checked
            // is a write to the cell itself.
            let disturbed: Vec<u32> = alive
                .iter()
                .copied()
                .filter(|value| {
                    (_may_write(one, &found[value], body.sealed_arguments) || writes_there(one, &found[value]))
                        && !stores(one, *value, &found[value])
                })
                .collect();
            for value in disturbed {
                found.shift_remove(&value);
                alive.remove(&value);
            }
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied().filter(|value| found.contains_key(value)));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Held, Imm, Semantics, Space};

    fn cell() -> Mem {
        Mem::new(Some(Addr::new(Space::Frame, -8)), 8)
    }

    fn insn(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(0, None, Some(what), defines, uses))
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10 })
    }

    fn store(value: u32) -> Arc<Insn> {
        insn(Operation::FloatStore, "fstp", vec![Loc::Mem(cell())], vec![held(value)], vec![], vec![value])
    }

    /// fpbench -O3: the spill store of a copy and the program's store of its
    /// source wrote one cell twice. A store of the source next, with nothing
    /// between, is the same bytes.
    #[test]
    fn test_a_store_of_the_copied_value_to_the_cell_next_makes_the_spill_store_redundant() {
        assert!(stored_again(&[store(1)], &cell(), vec![1], 2));
    }

    /// A read of the cell before the store would see the value early; a use of
    /// the spilled value is a reload of the cell the store has not reached.
    #[test]
    fn test_a_read_of_the_cell_or_of_the_spilled_value_before_the_store_keeps_the_spill_store() {
        let read = insn(Operation::FloatLoad, "fld", vec![held(9)], vec![Loc::Mem(cell())], vec![9], vec![]);
        let reader = insn(Operation::FloatArith, "fmul", vec![held(8)], vec![held(2), held(2)], vec![8], vec![2]);
        assert!(!stored_again(&[read, store(1)], &cell(), vec![1], 2));
        assert!(!stored_again(&[reader, store(1)], &cell(), vec![1], 2));
    }

    fn two_blocks(
        first: Vec<Arc<Insn>>,
        second: Vec<Arc<Insn>>,
    ) -> LirBody {
        use crate::model::lir::LirBlock;
        let blocks = vec![
            crate::model::lir::Block::from(LirBlock { succ: vec![1].into(), ..LirBlock::new(0, first) }),
            crate::model::lir::Block::from(LirBlock::new(1, second)),
        ];
        LirBody::new("homes", 0, blocks, Default::default(), Default::default())
    }

    /// fpbench -O3: a phi's input is a `fld` of a pool constant into the phi's
    /// own register, after the block before stored the constant's bits to the
    /// array cell. The cell holds the value after the definition; the same
    /// definition that reads the value (`fadd` into itself) leaves the cell
    /// behind, and so does one with no store before it.
    #[test]
    fn test_a_definition_after_the_blocks_last_store_to_the_cell_leaves_the_cell_holding_the_value() {
        let bits = insn(
            Operation::Move,
            "mov",
            vec![Loc::Mem(cell())],
            vec![Loc::Imm(Imm { value: 5, width: 4, address: None })],
            vec![],
            vec![],
        );
        let pool = Loc::Mem(Mem::new(Some(Addr::new(Space::Segment, 0)), 8));
        let made = insn(Operation::FloatLoad, "fld", vec![held(1)], vec![pool], vec![1], vec![]);
        let again = insn(Operation::FloatArith, "fadd", vec![held(1)], vec![held(1), held(1)], vec![1], vec![1]);
        let body = two_blocks(vec![bits], vec![made.clone(), again]);
        assert!(_cell_holds_after(&body, (1, 0), 1, &cell()));
        assert!(!_cell_holds_after(&body, (1, 1), 1, &cell()), "reads its own value");
        let none = two_blocks(vec![], vec![made]);
        assert!(!_cell_holds_after(&none, (1, 0), 1, &cell()), "no store before it");
    }
}
