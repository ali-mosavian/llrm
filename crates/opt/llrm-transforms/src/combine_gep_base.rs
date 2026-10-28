//! `CommonPointerBase` and `EmitGEPOffsets`: the pointer two GEP chains share
//! and the offset each adds to it.
//!
//! Translation of
//! [`CommonPointerBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L2244)
//! and
//! [`InstCombinerImpl::EmitGEPOffsets`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L238)
//! at d1106deb71cc. `foldGEPICmp` asks them today; `OptimizePointerDifference`
//! will.
//!
//! MIRROR: the chains are walked from each side to the first pointer they
//! share, their flags are folded with `intersectForOffsetAdd`, a chain is
//! expensive past two multi-use GEPs after the first, and the offsets of
//! one-use GEPs are merged into the next multi-use GEP, which is rewritten to
//! reuse the sum.
//!
//! DEVIATE 1: no vector GEPs: MIR has none.
//!
//! DEVIATE 2: `RewriteGEPs` is always true, the only way `foldGEPICmp` calls
//! it. Exit: `OptimizePointerDifference`, which passes false.
//!
//! # Example
//! ```text
//! %b = getelementptr inbounds nuw i8, ptr %p, i64 %x
//! %c = getelementptr inbounds i8, ptr %b, i64 -8
//! ```
//! shares `%p` with `getelementptr i8, ptr %p, i64 24`; the offsets are `%x`
//! plus `-8`, and `24`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags};
use llrm_mir::types::TypeId;

use crate::combine::Combiner;
use crate::combine_gep::GepFlags;

/// What `CommonPointerBase::compute` found: `ptr` is `None` when the chains
/// share no pointer.
pub(crate) struct CommonPointerBase {
    pub ptr: Option<Operand>,
    /// The GEPs from the left pointer down to `ptr`, outermost first.
    pub lhs_geps: Vec<InstId>,
    pub rhs_geps: Vec<InstId>,
    pub lhs_flags: GepFlags,
    pub rhs_flags: GepFlags,
}

impl Combiner<'_> {
    /// The `getelementptr` that defines `v`.
    pub(crate) fn defining_gep(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        self.definition_of(v).filter(|&def| self.gep_of(def).is_some())
    }

    /// `CommonPointerBase::compute`.
    pub(crate) fn common_pointer_base(
        &self,
        lhs: Operand,
        rhs: Operand,
    ) -> CommonPointerBase {
        let mut base = CommonPointerBase {
            ptr: None,
            lhs_geps: Vec::new(),
            rhs_geps: Vec::new(),
            lhs_flags: GepFlags::ALL,
            rhs_flags: GepFlags::ALL,
        };
        if self.type_of(lhs) != self.type_of(rhs) {
            return base;
        }
        // Every pointer the left chain passes through.
        let mut pointers = std::collections::HashSet::new();
        let mut at = lhs;
        loop {
            pointers.insert(at);
            match self.defining_gep(at) {
                Some(gep) => at = self.gep_of(gep).expect("a getelementptr").ptr,
                None => break,
            }
        }
        // Down the right chain to the first of them.
        let mut at = rhs;
        while !pointers.contains(&at) {
            let Some(gep) = self.defining_gep(at) else { return base };
            let (ptr, flags) = {
                let found = self.gep_of(gep).expect("a getelementptr");
                (found.ptr, GepFlags::of(found.flags))
            };
            base.rhs_flags =
                if base.rhs_geps.is_empty() { flags } else { base.rhs_flags.intersect_for_offset_add(flags) };
            base.rhs_geps.push(gep);
            at = ptr;
        }
        base.ptr = Some(at);
        // And the left chain down to it.
        let mut at = lhs;
        while at != base.ptr.expect("set above") {
            let gep = self.defining_gep(at).expect("the left chain reaches the shared pointer");
            let (ptr, flags) = {
                let found = self.gep_of(gep).expect("a getelementptr");
                (found.ptr, GepFlags::of(found.flags))
            };
            base.lhs_flags =
                if base.lhs_geps.is_empty() { flags } else { base.lhs_flags.intersect_for_offset_add(flags) };
            base.lhs_geps.push(gep);
            at = ptr;
        }
        base
    }

    /// `CommonPointerBase::isExpensive`: more than two multi-use GEPs besides
    /// the first of each chain, which would be expanded to offsets of their
    /// own.
    pub(crate) fn is_expensive(
        &self,
        base: &CommonPointerBase,
    ) -> bool {
        let extra_multi_use = |geps: &[InstId]| {
            let multi_use = geps.iter().filter(|&&gep| !self.has_one_use(self.value_of(gep))).count();
            multi_use.saturating_sub(1)
        };
        extra_multi_use(&base.lhs_geps) + extra_multi_use(&base.rhs_geps) > 2
    }

    /// `EmitGEPOffsets(GEPs, NW, IdxTy, /*RewriteGEPs=*/true)`: the sum of the
    /// offsets of `geps`, added before `at`. A multi-use GEP is rewritten to a
    /// `ptradd` of its offset, with those of the one-use GEPs below it
    /// merged in, so the sum reuses it.
    pub(crate) fn emit_gep_offsets(
        &mut self,
        at: InstId,
        geps: &[InstId],
        flags: GepFlags,
        index_ty: TypeId,
    ) -> Operand {
        let add = |this: &mut Self, sum: Option<Operand>, offset: Operand| match sum {
            Some(sum) => {
                let mut wrap = Flags::default();
                if flags.nuw {
                    wrap.insert(Flags::NUW);
                }
                if flags.in_bounds {
                    wrap.insert(Flags::NSW);
                }
                this.create_binop(at, BinaryOp::Add, sum, offset, wrap)
            }
            None => offset,
        };
        let mut sum = None;
        let mut one_use_sum = None;
        let mut one_use_base = None;
        let mut one_use_flags = GepFlags::ALL;
        for &gep in geps.iter().rev() {
            // At the GEP, so it can be rewritten; the sums go at `at`.
            let mut offset = self.emit_gep_offset(gep, false);
            let found = self.gep_of(gep).expect("a getelementptr");
            let gep_flags = GepFlags::of(found.flags);
            if self.has_one_use(self.value_of(gep)) {
                // Merged into the next multi-use GEP.
                one_use_sum = Some(add(self, one_use_sum, offset));
                one_use_flags = one_use_flags.intersect_for_offset_add(gep_flags);
                one_use_base.get_or_insert(found.ptr);
                continue;
            }
            if let Some(merged) = one_use_sum {
                offset = add(self, Some(merged), offset);
            }
            let byte_gep_of_it =
                self.context.types.int_bits(found.source) == Some(8) && found.indices.first() == Some(&offset);
            if !byte_gep_of_it {
                let rewritten_flags = one_use_flags.intersect_for_offset_add(gep_flags);
                let ptr = one_use_base.unwrap_or(found.ptr);
                let new = self.create_ptr_add(gep, ptr, offset, rewritten_flags.flags());
                self.replace_inst_with(gep, new);
                self.erase(gep);
            }
            sum = Some(add(self, sum, offset));
            one_use_sum = None;
            one_use_base = None;
            one_use_flags = GepFlags::ALL;
        }
        if let Some(merged) = one_use_sum {
            sum = Some(add(self, sum, merged));
        }
        sum.unwrap_or_else(|| self.int(index_ty, 0))
    }
}
