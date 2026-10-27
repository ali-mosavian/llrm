//! InstCombine's memory intrinsics: a `memcpy` or `memmove` of 1, 2, 4 or 8
//! bytes is one integer load and one store. Translated from
//! `InstCombinerImpl::SimplifyAnyMemTransfer`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp)).
//!
//! # How it works
//! A transfer of a constant length of 1, 2, 4 or 8 bytes becomes `load iN`
//! from the source and `store iN` to the destination, each at the alignment
//! known of its pointer, volatile as the transfer was; a single load and store
//! is right for an overlap, so a `memmove` goes the same way. A transfer from
//! an undef or poison source that is not volatile is erased. The C front end's
//! aggregate assignment is a `memcpy` (#3621); this is what lets SROA and
//! promotion see a copy of a small struct as a value.
//!
//! # Example
//! ```text
//! call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 4, i1 false)
//!   ==>  %v = load i32, ptr %s            ; store i32 %v, ptr %d
//! ```
//!
//! MIRROR: the size test (at most 8 and a power of two), the integer type, the
//! volatile bit carried over.
//!
//! DEVIATE 1: MIR gives a transfer's pointers no `align` attribute, so the
//! alignments are `pointer_alignment` of each side
//! (`Value::getPointerAlignment`), not `getKnownAlignment` with the assumption
//! cache and the dominator tree. DEVIATE 2: no
//! `isModSet(AA->getModRefInfoMask(Dest))` constant-memory test, and no atomic
//! transfers (MIR has none); no TBAA or loop metadata to carry (MIR transfers
//! have none).
//!
//! # Used by
//! `combine` (its `visit`, the `Call` opcode).

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Attribute, Flags, Opcode};
use llrm_mir::valuetracking::known::pointer_alignment;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

impl Combiner<'_> {
    /// `InstCombinerImpl::visitCallInst`, for the intrinsics translated so far.
    pub fn visit_call(
        &mut self,
        call: InstId,
    ) -> Visit {
        match self.intrinsic_of(call) {
            Some(Intrinsic::MemCpy | Intrinsic::MemMove) => self.simplify_any_mem_transfer(call),
            Some(Intrinsic::MemSet) => self.raise_alignments(call, 1),
            Some(Intrinsic::MinMax { signed, max }) => self.narrow_min_max(call, signed, max),
            None => self.transform_const_expr_cast_call(call),
            _ => Visit::None,
        }
    }

    /// The first step of `SimplifyAnyMemTransfer` and `SimplifyAnyMemSet`: the
    /// transfer's own destination (and, with `pointers` 2, source) alignment
    /// is raised to what is known of the pointer.
    fn raise_alignments(
        &mut self,
        call: InstId,
        pointers: usize,
    ) -> Visit {
        let operands = self.function.instruction(call).operands.clone();
        let mut changed = false;
        for index in 0..pointers.min(operands.len()) {
            let known = pointer_alignment(&query(self, Some(call)), operands[index]);
            let Opcode::Call(info) = &self.function.instruction(call).opcode else { return Visit::None };
            let current = info
                .argument_attrs
                .get(index)
                .into_iter()
                .flatten()
                .find_map(|attr| match attr {
                    Attribute::Int(name, value) if name == "align" => Some(*value),
                    _ => None,
                })
                .unwrap_or(1);
            if known > current {
                self.function.set_argument_align(call, index, known);
                changed = true;
            }
        }
        if changed { Visit::Changed } else { Visit::None }
    }

    /// `SimplifyAnyMemTransfer`.
    fn simplify_any_mem_transfer(
        &mut self,
        call: InstId,
    ) -> Visit {
        if self.raise_alignments(call, 2) == Visit::Changed {
            return Visit::Changed;
        }
        let operands = self.function.instruction(call).operands.clone();
        let [destination, source, length, volatile, ..] = operands[..] else { return Visit::None };
        let constant = |operand: Operand| match operand {
            Operand::Constant(id) => match self.context.get(id).kind {
                ConstantKind::Int(bits) => Some(bits),
                ConstantKind::Zero | ConstantKind::Null => Some(0),
                _ => None,
            },
            _ => None,
        };
        let volatile = constant(volatile) != Some(0);
        // A source that is provably undef copies nothing.
        if !volatile
            && matches!(
                source,
                Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Undef | ConstantKind::Poison)
            )
        {
            self.erase(call);
            return Visit::Erased;
        }
        let Some(size) = constant(length) else { return Visit::None };
        if size == 0 || size > 8 || !size.is_power_of_two() {
            return Visit::None;
        }
        let (source_align, destination_align) = {
            let q = query(self, Some(call));
            (pointer_alignment(&q, source).max(1), pointer_alignment(&q, destination).max(1))
        };
        let ty = self.context.types.int(size as u32 * 8);
        let void = self.context.types.void();
        let load = self.insert_before(
            call,
            Opcode::Load { align: Some(source_align), volatile },
            ty,
            vec![source],
            Flags::default(),
        );
        let loaded = Operand::Value(self.function.instruction(load).result.expect("a load has a result"));
        self.insert_before(
            call,
            Opcode::Store { align: Some(destination_align), volatile },
            void,
            vec![loaded, destination],
            Flags::default(),
        );
        self.erase(call);
        Visit::Erased
    }
}

#[cfg(test)]
#[path = "combine_memintrinsics_tests.rs"]
mod tests;
