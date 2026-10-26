//! `matchBSwapOrBitReverse`: an expression that moves each bit of one value to
//! its mirror position is a `bswap` or a `bitreverse` of it.
//!
//! Translation of
//! [`recognizeBSwapOrBitReverseIdiom`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3767)
//! with `collectBitParts` (:3553) and of
//! [`InstCombinerImpl::matchBSwapOrBitReverse`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2974).
//!
//! # How it works
//! `collectBitParts` walks the `or`, constant shift, constant `and`, `zext`,
//! `trunc`, `bswap`, `bitreverse` and constant funnel shift nodes down to the
//! one value they are made of and records, for each result bit, which bit of
//! that value it is (or that it is zero). When every non-zero bit sits at its
//! mirror position, the expression is the intrinsic of the provider (of the
//! narrower type the zero upper bits leave, masked where bits are zero).
//!
//! ```text
//! %a = or (lshr %x, 1) & 0x5555.., (%x & 0x5555..) << 1   ; five stages
//! ...                                     ==>   %r = bitreverse %x
//! ```
//!
//! MIRROR: the node cases and their early exits, the memo that answers a
//! value met again (or met on a cycle) with what it held, the recursion limit
//! of 48, the truncated type, the mask and the extension back.
//!
//! DEVIATE 1: no vectors.
//!
//! # Used by
//! `visit_or` (before `matchFunnelShift`), and `visit_call` for `fshl`,
//! `fshr` and `bswap`.

use std::collections::BTreeMap;

use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// `BitPart::Unset`.
const UNSET: i16 = -1;
/// `BitPartRecursionMaxDepth`.
const MAX_DEPTH: u32 = 48;

/// A potential constituent of a `bswap` or `bitreverse`: bit `A` of the
/// provider becomes bit `B` of the expression where `provenance[B] = A`.
#[derive(Clone)]
struct BitPart {
    provider: Operand,
    provenance: Vec<i16>,
}

impl BitPart {
    fn unset(
        provider: Operand,
        width: usize,
    ) -> Self {
        BitPart { provider, provenance: vec![UNSET; width] }
    }

    fn identity(
        provider: Operand,
        width: usize,
    ) -> Self {
        BitPart { provider, provenance: (0..width as i16).collect() }
    }
}

type Memo = BTreeMap<Operand, Option<BitPart>>;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `collectBitParts`, memoised: the entry is put in empty before the node is
/// looked at, so a value met again while it is being worked on is a failure.
fn collect(
    c: &Combiner,
    v: Operand,
    bswaps: bool,
    reversals: bool,
    memo: &mut Memo,
    depth: u32,
    found_root: &mut bool,
) -> Option<BitPart> {
    if let Some(known) = memo.get(&v) {
        return known.clone();
    }
    memo.insert(v, None);
    let found = node(c, v, bswaps, reversals, memo, depth, found_root);
    memo.insert(v, found.clone());
    found
}

fn node(
    c: &Combiner,
    v: Operand,
    bswaps: bool,
    reversals: bool,
    memo: &mut Memo,
    depth: u32,
    found_root: &mut bool,
) -> Option<BitPart> {
    let width = c.context.types.int_bits(c.type_of(v))? as usize;
    // Can't do integers over 128 bits; prevent stack overflow.
    if width > 128 || depth == MAX_DEPTH {
        return None;
    }
    let q = query(c, None);
    if let Some((inst, instruction)) = q.instruction_of(v) {
        let operands = &instruction.operands;
        let mut sub = |x: Operand, memo: &mut Memo, found_root: &mut bool| {
            collect(c, x, bswaps, reversals, memo, depth + 1, found_root)
        };
        match &instruction.opcode {
            // An inner node of the bswap: both sources from one provider.
            Opcode::Binary(BinaryOp::Or) => {
                let a = sub(operands[0], memo, found_root)?;
                let b = sub(operands[1], memo, found_root)?;
                if a.provider != b.provider {
                    return None;
                }
                let mut result = BitPart::unset(a.provider, width);
                for bit in 0..width {
                    if a.provenance[bit] != UNSET
                        && b.provenance[bit] != UNSET
                        && a.provenance[bit] != b.provenance[bit]
                    {
                        return None;
                    }
                    result.provenance[bit] =
                        if a.provenance[bit] == UNSET { b.provenance[bit] } else { a.provenance[bit] };
                }
                return Some(result);
            }
            // A logical shift by a constant: recurse, then shift the result.
            Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr)) if pattern::apint(&q, operands[1]).is_some() => {
                let (amount, _) = pattern::apint(&q, operands[1])?;
                if amount >= width as u128 {
                    return None;
                }
                // For bswap-only, whole bytes, for an early exit.
                if !reversals && amount % 8 != 0 {
                    return None;
                }
                let amount = amount as usize;
                let mut result = sub(operands[0], memo, found_root)?;
                let moved = &mut result.provenance;
                if *op == BinaryOp::Shl {
                    moved.truncate(width - amount);
                    moved.splice(0..0, std::iter::repeat_n(UNSET, amount));
                } else {
                    moved.drain(..amount);
                    moved.extend(std::iter::repeat_n(UNSET, amount));
                }
                return Some(result);
            }
            // A mask that clears bits: recurse, then unset those bits.
            Opcode::Binary(BinaryOp::And) if pattern::apint(&q, operands[1]).is_some() => {
                let (mask_bits, _) = pattern::apint(&q, operands[1])?;
                if !reversals && mask_bits.count_ones() % 8 != 0 {
                    return None;
                }
                let mut result = sub(operands[0], memo, found_root)?;
                for bit in 0..width {
                    if mask_bits >> bit & 1 == 0 {
                        result.provenance[bit] = UNSET;
                    }
                }
                return Some(result);
            }
            Opcode::Cast(CastOp::ZExt) => {
                let found = sub(operands[0], memo, found_root)?;
                let narrow = c.context.types.int_bits(c.type_of(operands[0]))? as usize;
                let mut result = BitPart::unset(found.provider, width);
                result.provenance[..narrow].copy_from_slice(&found.provenance[..narrow]);
                return Some(result);
            }
            Opcode::Cast(CastOp::Trunc) => {
                let found = sub(operands[0], memo, found_root)?;
                let mut result = BitPart::unset(found.provider, width);
                result.provenance.copy_from_slice(&found.provenance[..width]);
                return Some(result);
            }
            Opcode::Call(_) => match pattern::call_intrinsic(&q, inst) {
                // Most likely a partial bitreverse matched before.
                Some(IntrinsicId::Bitreverse) => {
                    let found = sub(operands[0], memo, found_root)?;
                    let mut result = BitPart::unset(found.provider, width);
                    for bit in 0..width {
                        result.provenance[width - 1 - bit] = found.provenance[bit];
                    }
                    return Some(result);
                }
                // Most likely a partial bswap matched before.
                Some(IntrinsicId::Bswap) => {
                    let found = sub(operands[0], memo, found_root)?;
                    let mut result = BitPart::unset(found.provider, width);
                    for byte in 0..width / 8 {
                        let from = byte * 8;
                        for bit in 0..8 {
                            result.provenance[width - 8 - from + bit] = found.provenance[from + bit];
                        }
                    }
                    return Some(result);
                }
                // fshl(X, Y, Z) is (X << (Z % BW)) | (Y >> (BW - (Z % BW))),
                // and fshr is fshl with the amount flipped.
                Some(id @ (IntrinsicId::Fshl | IntrinsicId::Fshr)) if pattern::apint(&q, operands[2]).is_some() => {
                    let (amount, _) = pattern::apint(&q, operands[2])?;
                    let mut shift = (amount % width as u128) as usize;
                    if id == IntrinsicId::Fshr {
                        shift = width - shift;
                    }
                    if !reversals && shift % 8 != 0 {
                        return None;
                    }
                    let left = sub(operands[0], memo, found_root)?;
                    let right = sub(operands[1], memo, found_root)?;
                    if left.provider != right.provider {
                        return None;
                    }
                    let start_right = width - shift;
                    let mut result = BitPart::unset(left.provider, width);
                    for bit in 0..start_right {
                        result.provenance[bit + shift] = left.provenance[bit];
                    }
                    for bit in 0..shift {
                        result.provenance[bit] = right.provenance[bit + start_right];
                    }
                    return Some(result);
                }
                _ => {}
            },
            _ => {}
        }
    }
    // Something that isn't a shift, `or`, `and`...: the root input, once.
    if *found_root {
        return None;
    }
    *found_root = true;
    Some(BitPart::identity(v, width))
}

/// Whether bit `from` going to bit `to` is a byte swap of `width` bits.
fn bswap_bit(
    from: usize,
    to: usize,
    width: usize,
) -> bool {
    from % 8 == to % 8 && from / 8 == width / 8 - to / 8 - 1
}

fn bit_reverse_bit(
    from: usize,
    to: usize,
    width: usize,
) -> bool {
    from == width - to - 1
}

impl Combiner<'_> {
    /// `matchBSwapOrBitReverse(I, MatchBSwaps, MatchBitReversals)`: `inst`
    /// as the `bswap` or `bitreverse` (with a mask and extension where the
    /// result has zero bits) it is.
    pub(crate) fn match_bswap_or_bit_reverse(
        &mut self,
        inst: InstId,
        bswaps: bool,
        reversals: bool,
    ) -> Option<Visit> {
        if !bswaps && !reversals {
            return None;
        }
        let instruction = self.function.instruction(inst);
        let rootable = match &instruction.opcode {
            Opcode::Binary(BinaryOp::Or) => true,
            Opcode::Call(_) => matches!(
                pattern::call_intrinsic(&query(self, None), inst),
                Some(IntrinsicId::Fshl | IntrinsicId::Fshr | IntrinsicId::Bswap)
            ),
            _ => false,
        };
        if !rootable {
            return None;
        }
        let ty = instruction.ty;
        let bits = self.context.types.int_bits(ty)?;
        if bits == 1 || bits > 128 {
            return None;
        }
        let root = Operand::Value(instruction.result?);
        let mut found_root = false;
        let mut memo = Memo::new();
        let found = collect(self, root, bswaps, reversals, &mut memo, 0, &mut found_root)?;

        // If the upper bits are zero, perform it as a truncated operation.
        let mut provenance = found.provenance;
        while provenance.last() == Some(&UNSET) {
            provenance.pop();
        }
        if provenance.is_empty() {
            return None;
        }
        let demanded = provenance.len();
        if demanded as u32 > bits {
            return None;
        }

        // Is the permutation a bswap (an even number of bytes) or a bitreverse?
        let mut demanded_mask = mask(demanded as u32);
        let mut for_bswap = bswaps && demanded % 16 == 0;
        let mut for_reverse = reversals;
        for (bit, &from) in provenance.iter().enumerate() {
            if !for_bswap && !for_reverse {
                break;
            }
            if from == UNSET {
                demanded_mask &= !(1u128 << bit);
                continue;
            }
            for_bswap &= bswap_bit(from as usize, bit, demanded);
            for_reverse &= bit_reverse_bit(from as usize, bit, demanded);
        }
        let base = if for_bswap {
            "bswap"
        } else if for_reverse {
            "bitreverse"
        } else {
            return None;
        };

        let demanded_ty = if demanded as u32 == bits { ty } else { self.context.types.int(demanded as u32) };
        let mut provider = found.provider;
        let provider_bits = self.context.types.int_bits(self.type_of(provider))?;
        if provider_bits != demanded as u32 {
            let cast = if provider_bits > demanded as u32 { CastOp::Trunc } else { CastOp::ZExt };
            provider = self.create_cast(inst, cast, provider, demanded_ty, Flags::default());
        }
        let mut result = self.create_intrinsic(inst, base, demanded_ty, &[provider]);
        if demanded_mask != mask(demanded as u32) {
            let mask = self.int(demanded_ty, demanded_mask);
            result = self.create_binop(inst, BinaryOp::And, result, mask, Flags::default());
        }
        if demanded_ty != ty {
            result = self.create_cast(inst, CastOp::ZExt, result, ty, Flags::default());
        }
        Some(Visit::Replace(result))
    }

    /// The `visitCallInst` cases: `fshl`/`fshr` try both, a `bswap` root only
    /// a `bitreverse`.
    pub(crate) fn bswap_or_bit_reverse_of_call(
        &mut self,
        call: InstId,
        intrinsic: &Intrinsic,
    ) -> Visit {
        let (bswaps, reversals) = match intrinsic {
            Intrinsic::FunnelShift { .. } => (true, true),
            Intrinsic::ByteSwap => (false, true),
            _ => return Visit::None,
        };
        self.match_bswap_or_bit_reverse(call, bswaps, reversals).unwrap_or(Visit::None)
    }
}

#[cfg(test)]
#[path = "combine_bitparts_tests.rs"]
mod tests;
