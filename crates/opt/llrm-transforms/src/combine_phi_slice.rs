//! `SliceUpIllegalIntegerPHI`: a phi of an integer type the target does not
//! compute natively, read only through `trunc` and `trunc(lshr C)`, becomes one
//! phi per piece read, the truncation moved into each incoming block.
//!
//! Translation of `InstCombinerImpl::SliceUpIllegalIntegerPHI`
//! ([InstCombinePHI.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombinePHI.cpp#L1069))
//! at d1106deb71cc and its call in `visitPHINode` (`!DL.isLegalInteger`).
//!
//! # How it works
//! The phi and the phis it feeds are collected, each user a `trunc` or a
//! one-use `lshr` by a constant under the width whose user is a `trunc`; any
//! other user, or an `invoke` defined in its own incoming block, ends it. Each
//! (phi, shift, width) read gets a phi of that width whose incoming values are
//! the truncated (shifted) values, built before the incoming block's
//! terminator; every `trunc` then reads its piece, and the old phis read
//! poison.
//!
//! # Example
//! ```text
//! %p = phi i64 [ %a, %l ], [ %b, %r ] ; %t = trunc i64 %p to i32
//!   ==>  %a.t = trunc i64 %a to i32 (in %l) ; %b.t = trunc i64 %b to i32 (in %r)
//!        %t = phi i32 [ %a.t, %l ], [ %b.t, %r ]
//! ```
//!
//! MIRROR: the user scan, the order of the records, the reuse of a piece by
//! (phi, shift, width), the poison left behind. The reuse of an already lowered
//! incoming phi (`ExtractedVals[LoweredPHIRecord(PN, ..)]` in the loop) reads
//! the key of the phi being built, which is not yet stored, so it never applies
//! and is not translated.
//!
//! DEVIATE: no check that a predecessor `hasInsertionPt()` (a `catchswitch`
//! block): MIR has no such phi predecessor. Exit: when EH pads are translated.
//!
//! # Used by
//! `combine_phi_args` (`visit_phi`).

use std::collections::HashMap;

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, defining};
use crate::combine_shared::query;

/// A `trunc` of one phi (`PHIUsageRecord`), at the shift it reads.
struct Usage {
    phi_id: usize,
    shift: u32,
    trunc: InstId,
}

impl Combiner<'_> {
    /// `SliceUpIllegalIntegerPHI`: the poison `first` is replaced with once
    /// its pieces are made, or `None` when a user is not a `trunc` read.
    pub(crate) fn slice_up_illegal_integer_phi(
        &mut self,
        first: InstId,
    ) -> Option<Operand> {
        let mut usages: Vec<Usage> = Vec::new();
        let mut phis: Vec<InstId> = vec![first];
        let mut id = 0;
        while id < phis.len() {
            let result = self.function.instruction(phis[id]).result?;
            let users: Vec<InstId> = self.function.users(result).iter().map(|one| one.user).collect();
            for user in users {
                let inst = self.function.instruction(user);
                match inst.opcode {
                    Opcode::Phi => {
                        if !phis.contains(&user) {
                            phis.push(user);
                        }
                    }
                    Opcode::Cast(CastOp::Trunc) => usages.push(Usage { phi_id: id, shift: 0, trunc: user }),
                    // otherwise an lshr by a constant that only one trunc reads
                    Opcode::Binary(BinaryOp::LShr) => {
                        let (amount, bits) = (inst.operands[1], self.context.types.int_bits(inst.ty)?);
                        let only = self.function.users(inst.result?).only()?.user;
                        if self.function.instruction(only).opcode != Opcode::Cast(CastOp::Trunc) {
                            return None;
                        }
                        let (shift, _) = pattern::apint(&query(self, None), amount)?;
                        if shift >= u128::from(bits) {
                            return None;
                        }
                        usages.push(Usage { phi_id: id, shift: shift as u32, trunc: only });
                    }
                    _ => return None,
                }
            }
            id += 1;
        }
        // An invoke defined in its own incoming block needs the truncate on a
        // critical edge, which cannot be split here.
        for &phi in &phis {
            for (value, from) in pattern::phi_incoming(self.function.instruction(phi)) {
                if let Some(def) = self.definition_of(value)
                    && matches!(self.function.instruction(def).opcode, Opcode::Invoke(_))
                    && self.function.parent(def) == Some(from)
                {
                    return None;
                }
            }
        }
        let poison = self.poison_value_of(first);
        if usages.is_empty() {
            return Some(poison);
        }
        let bits_of = |this: &Self, inst: InstId| this.context.types.int_bits(this.function.instruction(inst).ty);
        usages.sort_by_key(|one| (one.phi_id, one.shift, bits_of(self, one.trunc)));
        // each piece made, by (phi, shift, width)
        let mut pieces: HashMap<(InstId, u32, u32), InstId> = HashMap::new();
        let mut at = 0;
        while at < usages.len() {
            let Usage { phi_id, shift, trunc } = Usage { ..usages[at] };
            at += 1;
            let phi = phis[phi_id];
            let ty = self.function.instruction(trunc).ty;
            let width = bits_of(self, trunc)?;
            let piece = match pieces.get(&(phi, shift, width)) {
                Some(&piece) => piece,
                None => {
                    let piece = self.insert_before(phi, Opcode::Phi, ty, Vec::new(), Flags::default());
                    let piece_value = self.value_of(piece);
                    let own = self.value_of(phi);
                    let incoming: Vec<(Operand, BlockId)> =
                        pattern::phi_incoming(self.function.instruction(phi)).collect();
                    let mut by_block: HashMap<BlockId, Operand> = HashMap::new();
                    let mut operands = Vec::with_capacity(2 * incoming.len());
                    for (value, from) in incoming {
                        let known = by_block.get(&from).copied();
                        let taken = if let Some(known) = known {
                            known
                        } else if value == own {
                            piece_value
                        } else {
                            // extract in the predecessor
                            let end = self.function.terminator(from)?;
                            let mut piece_in = value;
                            if shift != 0 {
                                let by = self.int(self.type_of(value), u128::from(shift));
                                piece_in = self.create_binop(end, BinaryOp::LShr, piece_in, by, Flags::default());
                            }
                            piece_in = self.create_cast(end, CastOp::Trunc, piece_in, ty, Flags::default());
                            // an incoming phi that is being sliced too must be
                            // read for this piece as well
                            if let Some(old) = self.definition_of(value)
                                && let Some(ref_id) = phis.iter().position(|&one| one == old)
                                && let Some(made) = defining(self.function, piece_in)
                            {
                                usages.push(Usage { phi_id: ref_id, shift, trunc: made });
                            }
                            piece_in
                        };
                        by_block.insert(from, taken);
                        operands.push(taken);
                        operands.push(Operand::Block(from));
                    }
                    self.function.set_operands(piece, operands);
                    pieces.insert((phi, shift, width), piece);
                    piece
                }
            };
            let piece_value = self.value_of(piece);
            self.replace_inst_with(trunc, piece_value);
        }
        // the rest of the uses, self uses and the lshrs, read poison
        for &phi in &phis[1..] {
            self.replace_inst_with(phi, poison);
        }
        Some(poison)
    }

    /// `PoisonValue::get(phi's type)`.
    fn poison_value_of(
        &self,
        inst: InstId,
    ) -> Operand {
        let ty = self.function.instruction(inst).ty;
        Operand::Constant(
            self.context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
        )
    }
}
