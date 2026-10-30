//! The base reference of each load and store, decomposed once and shared.
//!
//! # How it works
//! `MemRef::of` splits an access's pointer into a root, a constant
//! displacement and one scaled index, and reads its `!tbaa` type: a fact of the
//! instruction and the pointer chain under it, which the points-to solves,
//! `Annotated`, `Accesses`, the walker and the passes each asked again, 181
//! times in a d_faces -O1 compile (`MemRef::of` was 6.5% of it). Here each
//! access is decomposed on its first ask and kept as an `Rc<MemRef>`, with the
//! values its chain read.
//!
//! The table is held by the module's `Context` (as `typed`'s names are), one
//! for each function history (`Mark::lineage`; a copy of a function has its
//! own). Asked after the function changed, it reads the change log since its
//! mark and drops the references the changes reach: an access whose operand was
//! rewritten or erased, and each access whose chain holds a value whose
//! definition was. A history it cannot read is dropped whole, as is one made
//! under other metadata. Moves change nothing: a reference does not read where
//! an instruction stands.
//!
//! ```text
//!  load p ← gep a, 4 ← alloca a         chain of the load: p, a
//!  gep operand rewritten (Operand, Rewritten)  → drops the load; the next ask
//!                                               decomposes it again
//!  an unrelated add rewritten                   → keeps it
//! ```
//!
//! `LLRM_CHECK_REFERENCES=1` decomposes afresh on every ask and asserts the
//! kept one equal.
//!
//! # Example
//! Two loads whose pointers are `gep`s of one `alloca`; `%q`'s offset is then
//! rewritten:
//!
//! ```mir
//! define i16 @f(i16 %i) {
//! b0:
//!   %a = alloca [8 x i16]
//!   %p = getelementptr inbounds i16, ptr %a, i16 %i
//!   %q = getelementptr inbounds i16, ptr %a, i16 3
//!   %x = load i16, ptr %p
//!   %y = load i16, ptr %q
//!   %z = add i16 %x, %y
//!   ret i16 %z
//! }
//! ```
//!
//! | ask | decomposes |
//! |-----|------------|
//! | `%x`, `%y`, `%x`, `%y` | two: the repeats read the kept ones |
//! | after `%q` is rewritten, `%x` and `%y` | one: `%y`, whose chain holds `%q` |
//!
//! From unit test
//! `test_a_changed_pointer_chain_decomposes_again_and_an_unchanged_one_does_not`
//! in `references_tests.rs`.
//!
//! # Provides
//! `of` (what `MemRef::of` reads through), `built` (decompositions made on this
//! thread, for the test that a second ask makes none).
//!
//! # Needs
//! [`crate::memory`] (`MemRef::decomposed`).
//!
//! # See also
//! LLVM's `MemoryLocation::get`, computed per access and kept by its users;
//! `BasicAA`'s `DecomposedGEP`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use llrm_mir::dense::IdMap;
use llrm_mir::module::{Change, InstId, Mark, ValueId};
use llrm_support::hash::HashMap;

use crate::memory::{MemRef, Unit};

thread_local! {
    static BUILT: Cell<usize> = const { Cell::new(0) };
}

/// The decompositions this thread has made.
pub fn built() -> usize {
    BUILT.with(Cell::get)
}

/// A module's tables, one for each function history.
#[derive(Default)]
pub(crate) struct Bases {
    tables: RefCell<HashMap<u64, Table>>,
}

struct Table {
    mark: Mark,
    /// The metadata the types were read from.
    metadata: (usize, usize),
    /// How many globals the unit listed: with none, no root is a global
    /// variable (`MemRef::object`).
    globals: usize,
    made: IdMap<InstId, Rc<MemRef>>,
    /// Each value some chain read, with the accesses whose chain it is in.
    chains: IdMap<ValueId, Vec<InstId>>,
}

impl Table {
    fn new(unit: &Unit) -> Self {
        Self {
            mark: unit.function.mark(),
            metadata: (unit.metadata.as_ptr() as usize, unit.metadata.len()),
            globals: unit.globals.len(),
            made: IdMap::new(),
            chains: IdMap::new(),
        }
    }

    fn clear(
        &mut self,
        unit: &Unit,
    ) {
        *self = Self::new(unit);
    }

    /// The references the `changes` since `mark` reach, gone.
    fn forget(
        &mut self,
        unit: &Unit,
        changes: &[Change],
    ) {
        let function = unit.function;
        let dropped = |table: &mut Self, inst: InstId| {
            table.made.remove(&inst);
            if let Some(result) = function.instruction(inst).result
                && let Some(accesses) = table.chains.remove(&result)
            {
                for access in accesses {
                    table.made.remove(&access);
                }
            }
        };
        for change in changes {
            match change {
                Change::Operand { inst, .. }
                | Change::Operands { inst, .. }
                | Change::Rewritten(inst)
                | Change::Erased { inst, .. }
                | Change::Inserted { inst, .. } => dropped(self, *inst),
                Change::Cloned { to, .. } => dropped(self, *to),
                Change::Parameters => {
                    self.clear(unit);
                    return;
                }
                // The uses of a value replaced by an equal one are rewritten,
                // each with an entry of its own; what read the value itself
                // goes too.
                Change::Substituted { from: value, .. } => {
                    for access in self.chains.remove(value).into_iter().flatten() {
                        self.made.remove(&access);
                    }
                }
                Change::Moved { .. } | Change::BlockCreated(_) | Change::BlockErased(_) => {}
            }
        }
        self.mark = function.mark();
    }
}

/// The reference of the load or store `inst`.
pub(crate) fn of(
    unit: &Unit,
    inst: InstId,
) -> Option<Rc<MemRef>> {
    use llrm_mir::opcode::Opcode;
    if !matches!(
        unit.function.instruction(inst).opcode,
        Opcode::Load { .. } | Opcode::Store { .. }
    ) {
        return None;
    }
    let bases = unit.context.extension::<Bases>();
    let mut tables = bases.tables.borrow_mut();
    let now = unit.function.mark();
    let table = tables.entry(now.lineage()).or_insert_with(|| Table::new(unit));
    if table.metadata != (unit.metadata.as_ptr() as usize, unit.metadata.len()) || table.globals != unit.globals.len() {
        table.clear(unit);
    } else if table.mark != now {
        match unit.function.changes_since(table.mark) {
            Some(changes) => table.forget(unit, changes),
            None => table.clear(unit),
        }
    }
    if let Some(kept) = table.made.get(&inst) {
        if llrm_support::env_flag!("LLRM_CHECK_REFERENCES") {
            let fresh = MemRef::decomposed(unit, inst, &mut Vec::new());
            assert!(fresh.as_ref() == Some(&**kept), "LLRM_CHECK_REFERENCES: a kept reference is not the access's");
        }
        return Some(Rc::clone(kept));
    }
    BUILT.with(|count| count.set(count.get() + 1));
    let mut chain = Vec::new();
    let made = Rc::new(MemRef::decomposed(unit, inst, &mut chain)?);
    for value in chain {
        table.chains.get_or_insert_with(value, Vec::new).push(inst);
    }
    table.made.insert(inst, Rc::clone(&made));
    Some(made)
}

#[cfg(test)]
#[path = "references_tests.rs"]
mod tests;
