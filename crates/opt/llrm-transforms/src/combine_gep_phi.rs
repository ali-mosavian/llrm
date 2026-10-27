//! `foldGEPOfPhi`: a `getelementptr` of a phi of `getelementptr`s as a
//! `getelementptr` of a phi of the one index they differ in.
//!
//! Translation of
//! [`foldGEPOfPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3211)
//! at d1106deb71cc.
//!
//! MIRROR: every incoming value is a GEP of one source type and operand count,
//! none is the GEP itself, they differ in at most one operand, which is not a
//! struct index past the first; the phi has one use if an index is to vary;
//! the new GEP has the flags all of them had.
//!
//! # Example
//! ```text
//! %p = phi ptr [ %a, %l ], [ %b, %r ]       %i = phi i64 [ %x, %l ], [ %y, %r ]
//!   (%a, %b: gep i32, ptr %base, i64 %x/%y)  %g = gep i32, ptr %base, i64 %i
//! ```
//!
//! # Used by
//! The `getelementptr` visit of [`combine_gep`](crate::combine_gep), which
//! tries `fold_gep_of_phi` on a pointer that is a phi.

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::pattern;

use crate::combine::Combiner;
use crate::combine_gep::{Gep, GepFlags, Step};

impl Combiner<'_> {
    /// `BB->getFirstInsertionPt()`: the first instruction that is neither a
    /// phi nor a landing pad.
    pub(crate) fn first_insertion_point(
        &self,
        block: BlockId,
    ) -> InstId {
        self.function
            .iter_block(block)
            .find(|&one| !matches!(
                self.function.instruction(one).opcode,
                Opcode::Phi | Opcode::LandingPad { .. }
            ))
            .expect("a terminated block")
    }

    /// `foldGEPOfPhi(GEP, PN, Builder)`: the pointer `gep` can read instead of
    /// `phi`.
    pub(crate) fn fold_gep_of_phi(
        &mut self,
        gep: InstId,
        phi: InstId,
    ) -> Option<Operand> {
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi)).collect();
        let first_def = self.definition_of(incoming.first()?.0)?;
        // Don't fold a GEP into itself through a loop: the previous
        // iteration's value would stay live to no gain.
        if first_def == gep {
            return None;
        }
        let first: Gep = self.gep_of(first_def)?;
        let steps = self.steps(first.source, &first.indices)?;
        let mut flags = GepFlags::of(first.flags);
        // The operand the GEPs differ in, and each incoming value of it.
        let mut differing: Option<usize> = None;
        for &(value, _) in &incoming[1..] {
            let def = self.definition_of(value)?;
            let other: Gep = self.gep_of(def)?;
            if first.indices.len() != other.indices.len() || first.source != other.source || def == gep {
                return None;
            }
            let operands = std::iter::once(first.ptr).chain(first.indices.iter().copied());
            let others = std::iter::once(other.ptr).chain(other.indices.iter().copied());
            for (at, (a, b)) in operands.zip(others).enumerate() {
                if self.type_of(a) != self.type_of(b) {
                    return None;
                }
                if a == b {
                    continue;
                }
                // The first two operands vary for any GEP; the rest must be
                // static for struct slots. Only one operand may differ: two
                // would need an R+R+R addressing mode no backend has.
                if differing.is_some() || (at > 1 && matches!(steps[at - 1], Step::Struct(_))) {
                    return None;
                }
                differing = Some(at);
            }
            flags = flags.and(GepFlags::of(other.flags));
        }
        // A varying index needs a new phi: the old one must go.
        let phi_value = self.function.instruction(phi).result.expect("a value");
        if differing.is_some() && !self.function.users(phi_value).has_one_use() {
            return None;
        }

        let clone = self.function.clone_instruction(first_def);
        self.function.set_flags(clone, flags.flags());
        if let Some(at) = differing {
            let ty = self.type_of(self.function.instruction(first_def).operands[at]);
            let mut operands = Vec::with_capacity(incoming.len() * 2);
            for &(value, from) in &incoming {
                let def = self.definition_of(value).expect("a getelementptr");
                operands.push(self.function.instruction(def).operands[at]);
                operands.push(Operand::Block(from));
            }
            let new_phi = self.insert_before(phi, Opcode::Phi, ty, operands, Default::default());
            let value = Operand::Value(self.function.instruction(new_phi).result.expect("a value"));
            self.function.set_operand(clone, at, value);
        }
        let block = self.function.parent(gep).expect("a placed instruction");
        let at = self.first_insertion_point(block);
        self.function.insert(clone, llrm_mir::edit::Position::Before(at)).expect("a placed instruction");
        self.worklist.add(clone);
        Some(Operand::Value(self.function.instruction(clone).result.expect("a value")))
    }
}
