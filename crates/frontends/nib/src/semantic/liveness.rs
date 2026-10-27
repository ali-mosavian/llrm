//! Liveness (section 8): a borrow lasts until the last use of what holds it,
//! on any path through the function's HIR, not until its binding's scope
//! ends. A change to a borrowed owner is noted where it happens and refused
//! once the function is built, if a holder is used after it.

use borrows::BorrowKey;

use super::*;

/// A change to a borrowed owner, at an instruction of a block.
pub(super) struct Conflict {
    block: u32,
    at: usize,
    holders: BTreeSet<BorrowKey>,
    error: Diagnostic,
}

/// A module variable a binding borrows across a call, as of the call.
pub(super) struct Across {
    block: u32,
    at: usize,
    holder: BorrowKey,
    lend: modref::Lend,
}

impl FunctionCompiler<'_> {
    /// Notes `holder`'s borrow across the call about to be emitted.
    pub(super) fn hold_across(
        &mut self,
        lend: modref::Lend,
        holder: BorrowKey,
    ) {
        let at = self.current_block_mut().instructions.len();
        self.across.push(Across { block: self.current, at, holder, lend });
    }

    /// Lends each call what is borrowed across it and used after it.
    pub(super) fn lend_across(&mut self) {
        for across in std::mem::take(&mut self.across) {
            let held = self.derived(&BTreeSet::from([across.holder]));
            if self.used_after(across.block, across.at + 1, &held) {
                self.lends.push(across.lend);
            }
        }
    }

    /// Notes a change to `path` in `owner` here, refused with `error` if
    /// what borrows it, other than the reference `via` it is made through,
    /// is used later. A sequence a loop walks is borrowed by the walk,
    /// for the whole loop.
    pub(super) fn change_borrowed(
        &mut self,
        owner: BorrowKey,
        path: &[String],
        via: Option<BorrowKey>,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        let element = via.is_some_and(|key| self.walking.contains(&key));
        if !element && self.iterated.iter().any(|root| root.overlaps(owner, path)) {
            return Err(error);
        }
        let holders = self.holders(owner, path, false, via);
        self.conflict(holders, error);
        Ok(())
    }

    /// Notes a shared borrow of `path` in `owner` here, refused with `error`
    /// if a `&mut` borrow of it, which may change it, is used later.
    pub(super) fn share_borrowed(
        &mut self,
        owner: BorrowKey,
        path: &[String],
        via: Option<BorrowKey>,
        error: Diagnostic,
    ) {
        let holders = self.holders(owner, path, true, via);
        self.conflict(holders, error);
    }

    /// Notes the drop of `owner` here, refused with `error` if what borrows
    /// it is used later, as a later drop uses its own owner.
    pub(super) fn drop_borrowed(
        &mut self,
        owner: BorrowKey,
        error: Diagnostic,
    ) {
        let holders = self.holders(owner, &[], false, None);
        self.conflict(holders, error);
    }

    fn conflict(
        &mut self,
        holders: BTreeSet<BorrowKey>,
        error: Diagnostic,
    ) {
        if !holders.is_empty() {
            let at = self.current_block_mut().instructions.len();
            self.conflicts.push(Conflict { block: self.current, at, holders, error });
        }
    }

    /// Errs at the first change some holder outlives.
    pub(super) fn check_conflicts(&self) -> Result<(), Diagnostic> {
        for conflict in &self.conflicts {
            let held = self.derived(&conflict.holders);
            if self.used_after(conflict.block, conflict.at, &held) {
                return Err(conflict.error.clone());
            }
        }
        Ok(())
    }

    /// `holders`, every pointer computed from one -- a view's data, a
    /// field's address -- and every place one is stored in. A borrow lives
    /// on in each.
    fn derived(
        &self,
        holders: &BTreeSet<BorrowKey>,
    ) -> BTreeSet<BorrowKey> {
        let pointers: BTreeSet<u32> = self
            .values
            .iter()
            .filter(|one| self.types.types[(one.type_id - 1) as usize].kind == "pointer")
            .map(|one| one.id)
            .collect();
        let mut held = holders.clone();
        loop {
            let before = held.len();
            for instruction in self.blocks.iter().flat_map(|block| &block.instructions) {
                let reads = |operand: &hir::Operand| mentions(operand).iter().any(|one| held.contains(one));
                // A held pointer stored somewhere makes that place a holder.
                if let ("store", [place, value]) = (instruction.op, instruction.operands.as_slice()) {
                    if reads(value) {
                        held.extend(mentions(place));
                    }
                    continue;
                }
                if instruction.operands.iter().any(reads) && !self.makes_owned(instruction) {
                    held.extend(
                        instruction
                            .results
                            .iter()
                            .filter(|one| pointers.contains(one))
                            .map(|one| BorrowKey::Value(*one)),
                    );
                }
            }
            if held.len() == before {
                return held;
            }
        }
    }

    /// Whether `instruction` is a call whose result is a new value the
    /// caller owns, a copy, which borrows nothing it was given.
    fn makes_owned(
        &self,
        instruction: &hir::Instruction,
    ) -> bool {
        let Some(callee) = instruction.callee.as_deref().filter(|_| instruction.op == "call") else {
            return false;
        };
        let result = match self.builtin_ids.get(callee) {
            Some((_, result)) => Some(*result),
            None => self.signatures.values().find(|one| one.name == callee).map(|one| one.result),
        };
        result.is_some_and(ownership::needs_drop)
    }

    /// Whether an instruction from `at` in `block` on, or one a path from
    /// there reaches, uses one of `held` before redefining it.
    fn used_after(
        &self,
        block: u32,
        at: usize,
        held: &BTreeSet<BorrowKey>,
    ) -> bool {
        let mut pending = vec![(block, at, BTreeSet::new())];
        let mut seen: BTreeSet<(u32, BTreeSet<BorrowKey>)> = BTreeSet::new();
        while let Some((block, at, mut killed)) = pending.pop() {
            let builder = &self.blocks[(block - 1) as usize];
            let from = at.min(builder.instructions.len());
            for (index, instruction) in builder.instructions.iter().enumerate().skip(from) {
                let reseat = self.reseats.contains(&(block, index));
                // A store writes its place without reading it.
                let written =
                    matches!(
                        (instruction.op, instruction.operands.first()),
                        ("store", Some(hir::Operand::Place(_)))
                    )
                        || reseat;
                let read = if written { &instruction.operands[1..] } else { &instruction.operands[..] };
                if read.iter().flat_map(mentions).any(|one| held.contains(&one) && !killed.contains(&one)) {
                    return true;
                }
                killed.extend(redefined(instruction, reseat).into_iter().filter(|one| held.contains(one)));
            }
            let Some(terminator) = &builder.terminator else {
                continue;
            };
            if terminator.operands.iter().flat_map(mentions).any(|one| held.contains(&one) && !killed.contains(&one)) {
                return true;
            }
            if killed.len() == held.len() {
                continue;
            }
            for target in &terminator.targets {
                if seen.insert((*target, killed.clone())) {
                    pending.push((*target, 0, killed.clone()));
                }
            }
        }
        false
    }
}

/// The values and places `operand` reads or addresses.
fn mentions(operand: &hir::Operand) -> Vec<BorrowKey> {
    match operand {
        hir::Operand::Value(value) => vec![BorrowKey::Value(*value)],
        hir::Operand::Constant(..) => Vec::new(),
        hir::Operand::Place(place) => vec![BorrowKey::Place(*place)],
        hir::Operand::ArrayElement(place, indices) | hir::Operand::ProjectedPlace { place, indices, .. } => {
            std::iter::once(BorrowKey::Place(*place)).chain(indices.iter().flat_map(mentions)).collect()
        }
        hir::Operand::IndirectPlace { base, .. } | hir::Operand::DescriptorPlace { base, .. } => {
            vec![BorrowKey::Value(*base)]
        }
    }
}

/// What `instruction` gives a new value: its results, or the whole place
/// it stores to.
fn redefined(
    instruction: &hir::Instruction,
    reseat: bool,
) -> Vec<BorrowKey> {
    let stored = match (instruction.op, instruction.operands.first()) {
        ("store", Some(hir::Operand::Place(place))) => Some(BorrowKey::Place(*place)),
        // A reseated view's descriptor is written through its address.
        ("store", Some(hir::Operand::IndirectPlace { base, .. })) if reseat => Some(BorrowKey::Value(*base)),
        _ => None,
    };
    instruction.results.iter().map(|one| BorrowKey::Value(*one)).chain(stored).collect()
}
