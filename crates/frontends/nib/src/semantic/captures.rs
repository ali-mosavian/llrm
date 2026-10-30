//! Which borrowed parameters a function keeps beyond its call (section 8).
//! A borrow cannot outlive its call unless the function stores it where the
//! caller sees it, returns it, or hands it to something that does: then the
//! parameter is captured. Every other `&` and `&mut` parameter is stated
//! `nocapture`, and a caller's objects stay reachable only by the borrows
//! it still holds.

use borrows::{BorrowKey, Root};

use super::*;

/// Where a borrowed parameter, by its ordinal among the parameters, may go.
#[derive(Clone, Debug)]
pub(super) enum Escape {
    /// Stored where the caller reaches it, returned, or made a raw pointer.
    Kept(usize),
    /// Lent to `callee` as its argument number `argument`.
    Lent { ordinal: usize, callee: String, argument: usize },
}

impl FunctionCompiler<'_> {
    /// The ordinals of the borrowed parameters `roots` borrow from.
    fn lent_ordinals(
        &self,
        roots: &BTreeSet<Root>,
    ) -> Vec<usize> {
        roots.iter().filter_map(|root| self.borrowed_ordinals.get(&root.owner).copied()).collect()
    }

    /// Notes that what `roots` borrow from is kept: of each parameter among
    /// them.
    pub(super) fn keep_lent(
        &mut self,
        roots: &BTreeSet<Root>,
    ) {
        let kept: Vec<Escape> = self.lent_ordinals(roots).into_iter().map(Escape::Kept).collect();
        self.escapes.extend(kept);
    }

    /// Notes what a call of `callee` is lent, argument by argument.
    pub(super) fn lend_to_call(
        &mut self,
        callee: &str,
        lent: &[borrows::Lent],
    ) {
        let mut lends = Vec::new();
        for (argument, one) in lent.iter().enumerate() {
            for ordinal in self.lent_ordinals(&one.roots) {
                lends.push(Escape::Lent { ordinal, callee: callee.to_owned(), argument });
            }
        }
        self.escapes.extend(lends);
    }

    /// The identity of each borrowed parameter's binding, to its ordinal.
    pub(super) fn note_borrowed_parameter(
        &mut self,
        key: BorrowKey,
        ordinal: usize,
    ) {
        self.borrowed_ordinals.insert(key, ordinal);
    }
}

/// The (function, ordinal) of each borrowed parameter that is kept: by its
/// own body, or by a callee it is lent to that keeps it, or that no body
/// here shows.
pub(super) fn kept(compiled: &[Compiled]) -> BTreeSet<(String, usize)> {
    let defined: BTreeSet<&str> = compiled.iter().map(|one| one.function.name.as_str()).collect();
    let mut kept: BTreeSet<(String, usize)> = BTreeSet::new();
    loop {
        let before = kept.len();
        for one in compiled {
            for escape in &one.escapes {
                match escape {
                    Escape::Kept(ordinal) => {
                        kept.insert((one.function.name.clone(), *ordinal));
                    }
                    Escape::Lent { ordinal, callee, argument } => {
                        if !defined.contains(callee.as_str()) || kept.contains(&(callee.clone(), *argument)) {
                            kept.insert((one.function.name.clone(), *ordinal));
                        }
                    }
                }
            }
        }
        if kept.len() == before {
            return kept;
        }
    }
}
