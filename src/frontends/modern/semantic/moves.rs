//! Which owners have moved at each point (section 8): a moved binding
//! cannot be used until it is assigned again. The moved set is kept per
//! HIR block and flows along each terminator's edges, so every construct
//! that builds control flow -- branches, loops, `break`, `?` -- is covered
//! by the one rule.

use super::*;

/// An owning binding, by its storage.
pub(super) type Owner = (bool, u32);

pub(super) fn owner(storage: &Storage) -> Option<Owner> {
    match storage {
        Storage::Place(place) => Some((false, *place)),
        Storage::Reference(pointer) => Some((true, *pointer)),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct Moves {
    /// The moved set flowing into each block not yet started.
    entry: BTreeMap<u32, BTreeSet<Owner>>,
    /// The moved set at the end of what each started block holds so far.
    state: BTreeMap<u32, BTreeSet<Owner>>,
    /// A plain assignment is resolving its target, which it may reinitialize.
    pub(super) writing: bool,
    /// A move found on a loop's back edge, reported after its statement.
    pub(super) error: Option<Diagnostic>,
}

impl FunctionCompiler<'_> {
    /// The moved set where code is emitted now.
    pub(super) fn moved(&mut self) -> &mut BTreeSet<Owner> {
        let block = self.current;
        let entry = self.moves.entry.remove(&block).unwrap_or_default();
        self.moves.state.entry(block).or_insert(entry)
    }

    fn is_moved(&self, owner: Owner) -> bool {
        let block = self.current;
        self.moves
            .state
            .get(&block)
            .or_else(|| self.moves.entry.get(&block))
            .is_some_and(|set| set.contains(&owner))
    }

    /// Errs when `binding`, named `name`, is used after a move.
    pub(super) fn check_unmoved(
        &self,
        name: &str,
        binding: &Binding,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match owner(&binding.storage) {
            Some(one) if !self.moves.writing && self.is_moved(one) => Err(Diagnostic::new(
                span,
                format!("{name:?} was moved; copy it with .copy() to keep using it"),
            )),
            _ => Ok(()),
        }
    }

    /// The moved set flows from the current block along `targets`. A block
    /// already started is a loop's head: nothing it can still see may have
    /// moved since. The scopes a jump there leaves are not seen.
    pub(super) fn flow_moves(&mut self, targets: &[u32]) {
        let moved = self.moved().clone();
        for target in targets {
            if let Some(before) = self.moves.state.get(target) {
                let depth = self.loops.iter().rev().find(|one| one.next == *target).map_or(self.scopes.len(), |one| one.next_depth);
                let visible = self.scopes[..depth.min(self.scopes.len())].iter().flat_map(|scope| scope.iter());
                let fresh = visible.filter(|(_, one)| {
                    owner(&one.storage)
                        .is_some_and(|key| moved.contains(&key) && !before.contains(&key))
                });
                if let Some((name, _)) = fresh.min_by_key(|(name, _)| name.as_str()) {
                    self.moves.error.get_or_insert_with(|| {
                        Diagnostic::new(
                            Span::new(0, 0, 0),
                            format!("{name:?} is moved in one loop iteration and used in the next"),
                        )
                    });
                }
                continue;
            }
            self.moves
                .entry
                .entry(*target)
                .or_default()
                .extend(moved.iter().copied());
        }
    }

    /// `owner` holds a value again.
    pub(super) fn reinitialized(&mut self, storage: &Storage) {
        if let Some(one) = owner(storage) {
            self.moved().remove(&one);
        }
    }
}
