//! Which loops a loop pass visits: the loop pass manager's worklist, for the
//! passes that walk every loop of a function.
//!
//! # How it works
//! LLVM's `LoopPassManager` runs a list of passes on a worklist of loops. A
//! pass that makes loops (`LoopFullUnroll`) hands them back
//! (`LPMUpdater::addSiblingLoops`) and the passes after it run on those loops
//! alone. Ours are whole-function passes, so a slot that wants only some loops
//! sets a scope: [`only`] names the headers, and each pass drops the loops
//! outside them from the list it walks with [`retain`]. Without a scope every
//! loop is visited, as before.
//!
//! # Example
//! After a full unroll made the loops headed by `b7` and `b9`, the slot runs
//! `indvars` inside `loopfilter::only(&[b7, b9])`: its steps see those two
//! loops and no others.
//!
//! # Used by
//! `indvars` (its steps), `loopunrollpass` (the loops it makes), `pipeline`.

use std::cell::RefCell;

use llrm_mir::module::BlockId;

thread_local! {
    static ONLY: RefCell<Option<Vec<BlockId>>> = const { RefCell::new(None) };
}

/// The scope: the loops of the named headers until the guard drops.
pub struct Scope(Option<Vec<BlockId>>);

impl Drop for Scope {
    fn drop(&mut self) {
        ONLY.with(|only| *only.borrow_mut() = self.0.take());
    }
}

/// Restrict the loops visited to those headed by `headers`.
pub fn only(headers: &[BlockId]) -> Scope {
    Scope(ONLY.with(|only| only.borrow_mut().replace(headers.to_vec())))
}

/// Whether a scope narrows the loops now: a run for one loop of a loop pass
/// manager, not for the function.
pub fn scoped() -> bool {
    ONLY.with(|only| only.borrow().is_some())
}

/// Whether the loop headed by `header` is in the scope; every loop is, without one.
pub fn allows(header: BlockId) -> bool {
    ONLY.with(|only| only.borrow().as_ref().is_none_or(|only| only.contains(&header)))
}

thread_local! {
    static UNSWITCHED: RefCell<(bool, Vec<BlockId>)> = const { RefCell::new((false, Vec::new())) };
}

/// What a scoped `SimpleLoopUnswitch` hands the loop pass manager
/// (`LPMUpdater::addSiblingLoops` and `revisitCurrentLoop`): the sibling loops
/// it cloned and whether the loop is to be run again.
pub fn note_unswitched(
    revisit: bool,
    siblings: Vec<BlockId>,
) {
    UNSWITCHED.with(|noted| *noted.borrow_mut() = (revisit, siblings));
}

/// The last [`note_unswitched`], cleared.
pub fn take_unswitched() -> (bool, Vec<BlockId>) {
    UNSWITCHED.with(|noted| std::mem::take(&mut *noted.borrow_mut()))
}

/// `headers` less the loops outside the scope; all of them without one.
pub fn retain(headers: &mut Vec<BlockId>) {
    ONLY.with(|only| {
        if let Some(only) = only.borrow().as_ref() {
            headers.retain(|header| only.contains(header));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_keeps_its_loops_and_ends_with_its_guard() {
        let all: Vec<BlockId> = (0..5).map(BlockId).collect();
        let mut seen = all.clone();
        retain(&mut seen);
        assert_eq!(seen, all, "no scope: every loop");
        {
            let _scope = only(&[all[1], all[3]]);
            let mut seen = all.clone();
            retain(&mut seen);
            assert_eq!(seen, vec![all[1], all[3]]);
        }
        let mut seen = all.clone();
        retain(&mut seen);
        assert_eq!(seen, all, "the guard dropped");
    }
}
