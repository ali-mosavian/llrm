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

/// What moved: an owner, or the field of it a path names.
pub(super) type Moved = (Owner, Vec<String>);

#[derive(Default)]
pub(super) struct Moves {
    /// The moved set flowing into each block not yet started.
    entry: BTreeMap<u32, BTreeSet<Moved>>,
    /// The moved set at the end of what each started block holds so far.
    state: BTreeMap<u32, BTreeSet<Moved>>,
    /// A plain assignment is resolving its target, which it may reinitialize.
    pub(super) writing: bool,
    /// The owner a field path is being resolved in: the path, not the
    /// owner, is what is used, and the path is checked.
    pub(super) projecting: std::cell::Cell<Option<Owner>>,
    /// A move found on a loop's back edge, reported after its statement.
    pub(super) error: Option<Diagnostic>,
}

impl FunctionCompiler<'_> {
    /// The moved set where code is emitted now.
    pub(super) fn moved(&mut self) -> &mut BTreeSet<Moved> {
        let block = self.current;
        let entry = self.moves.entry.remove(&block).unwrap_or_default();
        self.moves.state.entry(block).or_insert(entry)
    }

    /// The paths of `owner` moved here; an empty one is the whole owner.
    pub(super) fn moved_paths(&self, owner: Owner) -> Vec<Vec<String>> {
        let block = self.current;
        let set = self.moves.state.get(&block).or_else(|| self.moves.entry.get(&block));
        set.into_iter().flatten().filter(|(one, _)| *one == owner).map(|(_, path)| path.clone()).collect()
    }

    /// Errs when `binding`, named `name`, is used after a move.
    pub(super) fn check_unmoved(
        &self,
        name: &str,
        binding: &Binding,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match owner(&binding.storage) {
            Some(one) if !self.moves.writing && self.moved_paths(one).iter().any(Vec::is_empty) => Err(Diagnostic::new(
                span,
                format!("{name:?} was moved; copy it with .copy() to keep using it"),
            )),
            _ => Ok(()),
        }
    }

    /// Errs when the struct `binding`, named `name`, is used whole after a
    /// field of it moved; not when a field path is being resolved in it.
    pub(super) fn check_whole(&self, name: &str, binding: &Binding, span: Span) -> Result<(), Diagnostic> {
        let Some(one) = owner(&binding.storage) else {
            return Ok(());
        };
        if self.moves.projecting.take() == Some(one) || self.moves.writing {
            return Ok(());
        }
        match self.moved_paths(one).first() {
            Some(path) => Err(Diagnostic::new(span, format!("{name:?} was partly moved: \"{name}.{}\"", path.join(".")))),
            None => Ok(()),
        }
    }

    /// Errs when the field `path` of `owner`, named `name`, or a field of
    /// it, has moved.
    pub(super) fn check_path_unmoved(&self, owner: Owner, name: &str, path: &[String], span: Span) -> Result<(), Diagnostic> {
        let spelled = |path: &[String]| std::iter::once(name.to_owned()).chain(path.iter().cloned()).collect::<Vec<_>>().join(".");
        for moved in self.moved_paths(owner) {
            if path.starts_with(&moved) {
                return Err(Diagnostic::new(span, format!("{:?} was moved; copy it with .copy() to keep using it", spelled(&moved))));
            }
            if moved.starts_with(path) {
                return Err(Diagnostic::new(span, format!("{:?} was partly moved: {:?}", spelled(path), spelled(&moved))));
            }
        }
        Ok(())
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
                    owner(&one.storage).is_some_and(|key| moved.iter().any(|one| one.0 == key && !before.contains(one)))
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
                .extend(moved.iter().cloned());
        }
    }

    /// `owner` holds a value again, or the field `path` of it does.
    pub(super) fn reinitialized(&mut self, storage: &Storage, path: &[String]) {
        if let Some(one) = owner(storage) {
            self.refilled(one, path);
        }
    }

    /// The field `path` of `owner`, or all of it, holds a value again.
    pub(super) fn refilled(&mut self, one: Owner, path: &[String]) {
        self.moved().retain(|(owner, moved)| *owner != one || !moved.starts_with(path));
    }

    /// The owner, its name and the fields down to `place`, when `place` is
    /// a field, or a field of a field, of a named owner.
    pub(super) fn projected(&self, place: &Expr) -> Option<(Owner, String, Vec<String>)> {
        let (name, path, exact) = borrows::owner_path(place)?;
        if path.is_empty() || !exact {
            return None;
        }
        Some((owner(&self.visible(name)?.storage)?, name.to_owned(), path))
    }

    /// Readies a use of the field `place`: errs if it, or a field of it,
    /// moved; its owner, resolved next, is used only through it.
    pub(super) fn project(&self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        if let Some((owner, name, path)) = self.projected(place) {
            self.check_path_unmoved(owner, &name, &path, span)?;
            self.moves.projecting.set(Some(owner));
        }
        Ok(())
    }
}
