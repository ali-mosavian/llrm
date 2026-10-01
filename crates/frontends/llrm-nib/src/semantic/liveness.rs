//! Liveness (section 8): a borrow lasts until the last use of what holds it,
//! on any path through the function's HIR, not until its binding's scope
//! ends. A change to a borrowed owner is noted where it happens and refused
//! once the function is built, if a holder is used after it.

use super::*;
use borrows::BorrowKey;

/// A change to a borrowed owner, at an instruction of a block.
pub(super) struct Conflict {
    block: u32,
    at: usize,
    holders: BTreeSet<BorrowKey>,
    error: Diagnostic,
}

impl FunctionCompiler<'_> {
    /// Notes a change to `owner` here, refused with `error` if what borrows
    /// it is used later. A sequence a loop walks is borrowed by the walk,
    /// for the whole loop.
    pub(super) fn change_borrowed(&mut self, owner: BorrowKey, path: &[String], error: Diagnostic) -> Result<(), Diagnostic> {
        if self.iterated.iter().any(|root| root.overlaps(owner, path)) {
            return Err(error);
        }
        let holders = self.holders(owner, path);
        self.conflict(holders, error);
        Ok(())
    }

    fn conflict(&mut self, holders: BTreeSet<BorrowKey>, error: Diagnostic) {
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

    /// `holders`, and every pointer computed from one: a view's data, a
    /// field's address. A borrow lives on in each.
    fn derived(&self, holders: &BTreeSet<BorrowKey>) -> BTreeSet<BorrowKey> {
        let pointers: BTreeSet<u32> = self.values.iter().filter(|one| self.types.types[(one.type_id - 1) as usize].kind == "pointer").map(|one| one.id).collect();
        let mut held = holders.clone();
        loop {
            let before = held.len();
            for instruction in self.blocks.iter().flat_map(|block| &block.instructions) {
                if instruction.operands.iter().any(|operand| mentions(operand).iter().any(|one| held.contains(one))) {
                    held.extend(instruction.results.iter().filter(|one| pointers.contains(one)).map(|one| BorrowKey::Value(*one)));
                }
            }
            if held.len() == before {
                return held;
            }
        }
    }

    /// Whether an instruction from `at` in `block` on, or one a path from
    /// there reaches, uses one of `held` before redefining it.
    fn used_after(&self, block: u32, at: usize, held: &BTreeSet<BorrowKey>) -> bool {
        let mut pending = vec![(block, at, BTreeSet::new())];
        let mut seen: BTreeSet<(u32, BTreeSet<BorrowKey>)> = BTreeSet::new();
        while let Some((block, at, mut killed)) = pending.pop() {
            let builder = &self.blocks[(block - 1) as usize];
            for instruction in &builder.instructions[at.min(builder.instructions.len())..] {
                if instruction.operands.iter().flat_map(mentions).any(|one| held.contains(&one) && !killed.contains(&one)) {
                    return true;
                }
                killed.extend(redefined(instruction).into_iter().filter(|one| held.contains(one)));
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
        hir::Operand::IndirectPlace { base, .. } | hir::Operand::DescriptorPlace { base, .. } => vec![BorrowKey::Value(*base)],
    }
}

/// What `instruction` gives a new value: its results, or the whole place
/// it stores to.
fn redefined(instruction: &hir::Instruction) -> Vec<BorrowKey> {
    let stored = match (instruction.op, instruction.operands.first()) {
        ("store", Some(hir::Operand::Place(place))) => Some(BorrowKey::Place(*place)),
        _ => None,
    };
    instruction.results.iter().map(|one| BorrowKey::Value(*one)).chain(stored).collect()
}
