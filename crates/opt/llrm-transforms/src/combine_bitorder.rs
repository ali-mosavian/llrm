//! A byte or bit reversal written as shifts, masks and `or`s is the `bswap` or
//! `bitreverse` intrinsic, and a bitwise operation on a reversed value is the
//! reversal of the operation: LLVM's `matchBSwapOrBitReverse` (with
//! `recognizeBSwapOrBitReverseIdiom` and `collectBitParts` of `Local.cpp`) and
//! `foldBitwiseLogicWithIntrinsics` of InstCombine.
//!
//! # How it works
//! `collect_bit_parts` walks the expression under an `or` through `or`,
//! constant shifts, constant masks, `zext`, `trunc`, `bswap`, `bitreverse` and
//! funnel shifts by a constant, and says, for each result bit, which bit of one
//! provider value it is (or that it is zero). If the map is a byte reversal or
//! a bit reversal of the low `n` bits, the expression is that intrinsic of the
//! provider, truncated to `n` bits and masked where bits are zero.
//!
//! # Example
//! ```text
//!   (x << 24) | ((x & 0xff00) << 8) | ((x >> 8) & 0xff00) | (x >> 24)
//!                       ==>  call i32 @llvm.bswap.i32(i32 %x)
//! ```
//!
//! DEVIATE 1: no vectors.
//!
//! DEVIATE 2: only `visitOr` asks; the calls of `bswap` and `bitreverse`
//! (`visitCallInst`, :2619 and :2748) and `visitAnd`'s place of
//! `foldBitwiseLogicWithIntrinsics` (:2956, after folds not yet translated)
//! are not. In `visitAnd` and `visitXor` it runs after the folds translated.

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_support::hash::HashMap;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

/// `BitPartRecursionMaxDepth`.
const MAX_DEPTH: u32 = 48;
/// `BitPart::Unset`.
const UNSET: i8 = -1;

/// `m_Intrinsic`: `v` is a call of `llvm.<base>.<type>`, with its arguments.
/// The callee's name is the module's, or that of a declaration this pass made
/// (the `bswap` an earlier `or` became, whose declaration the module has only
/// after the pass).
fn call_of<'a>(
    q: &Query<'a>,
    declared: &Declared,
    v: Operand,
) -> Option<(String, &'a [Operand])> {
    let (_, instruction) = q.instruction_of(v)?;
    if !matches!(instruction.opcode, Opcode::Call(_)) {
        return None;
    }
    let Operand::Constant(callee) = *instruction.operands.last()? else { return None };
    let ConstantKind::Global(global) = q.context().get(callee).kind else { return None };
    let name = match q.globals.get(global.0 as usize) {
        Some(found) => found.name.as_deref()?,
        None => declared.pending_name(global)?,
    };
    let base = name.strip_prefix("llvm.")?.split('.').next()?;
    Some((base.to_owned(), &instruction.operands[..instruction.operands.len() - 1]))
}

/// A potential constituent of a reversal: `provenance[a] = b` says bit `a` of
/// `provider` becomes bit `b` of the result.
#[derive(Clone)]
struct BitPart {
    provider: Operand,
    provenance: Vec<i8>,
}

impl BitPart {
    fn new(
        provider: Operand,
        width: u32,
    ) -> Self {
        Self { provider, provenance: vec![UNSET; width as usize] }
    }
}

struct Collect<'q, 'a> {
    q: &'q Query<'a>,
    declared: &'q Declared,
    bitreversals: bool,
    /// `BPS`: each value analysed, None where the analysis failed.
    parts: HashMap<Operand, Option<BitPart>>,
    found_root: bool,
}

impl Collect<'_, '_> {
    fn width(
        &self,
        v: Operand,
    ) -> Option<u32> {
        let ty = self.q.function.operand_type(self.q.context(), v)?;
        self.q.context().types.int_bits(ty)
    }

    /// `collectBitParts`.
    fn run(
        &mut self,
        v: Operand,
        depth: u32,
    ) -> Option<BitPart> {
        if let Some(done) = self.parts.get(&v) {
            return done.clone();
        }
        // Inserted before it is computed: a value met again while it is being
        // analysed (a phi) has failed.
        self.parts.insert(v, None);
        let result = self.analyse(v, depth);
        self.parts.insert(v, result.clone());
        result
    }

    fn analyse(
        &mut self,
        v: Operand,
        depth: u32,
    ) -> Option<BitPart> {
        let width = self.width(v)?;
        // Can't do integers wider than 128 bits.
        if width > 128 {
            return None;
        }
        if depth == MAX_DEPTH {
            return None;
        }
        let q = self.q;
        if q.instruction_of(v).is_some() {
            // An `or` is an inner node: both sides from the same provider.
            if let Some((_, _, x, y)) = pattern::binary(q, v, BinaryOp::Or) {
                let a = self.run(x, depth + 1)?;
                let b = self.run(y, depth + 1)?;
                if a.provider != b.provider {
                    return None;
                }
                let mut merged = BitPart::new(a.provider, width);
                for at in 0..width as usize {
                    let (left, right) = (a.provenance[at], b.provenance[at]);
                    if left != UNSET && right != UNSET && left != right {
                        return None;
                    }
                    merged.provenance[at] = if left == UNSET { right } else { left };
                }
                return Some(merged);
            }
            // A logical shift by a constant: recurse, then shift the result.
            for (op, left) in [(BinaryOp::Shl, true), (BinaryOp::LShr, false)] {
                let Some((_, _, x, amount)) = pattern::binary(q, v, op) else { continue };
                let Some((shift, _)) = q.int_constant(amount) else { continue };
                // The shift amount must be defined.
                if shift >= u128::from(width) {
                    return None;
                }
                let shift = shift as usize;
                // For bswap only, whole bytes: an early exit.
                if !self.bitreversals && shift % 8 != 0 {
                    return None;
                }
                let mut result = self.run(x, depth + 1)?;
                let p = &mut result.provenance;
                if left {
                    p.truncate(p.len() - shift);
                    p.splice(0..0, std::iter::repeat_n(UNSET, shift));
                } else {
                    p.drain(..shift);
                    p.extend(std::iter::repeat_n(UNSET, shift));
                }
                return Some(result);
            }
            // An `and` with a mask that clears bits: recurse, then unset them.
            if let Some((_, _, x, mask)) = pattern::binary(q, v, BinaryOp::And)
                && let Some((mask, _)) = q.int_constant(mask)
            {
                // For bswap only, a multiple of 8 bits: an early exit.
                if !self.bitreversals && mask.count_ones() % 8 != 0 {
                    return None;
                }
                let mut result = self.run(x, depth + 1)?;
                for at in 0..width as usize {
                    if mask >> at & 1 == 0 {
                        result.provenance[at] = UNSET;
                    }
                }
                return Some(result);
            }
            if let Some((_, _, x)) = pattern::cast(q, v, CastOp::ZExt) {
                let res = self.run(x, depth + 1)?;
                let narrow = self.width(x)? as usize;
                let mut result = BitPart::new(res.provider, width);
                result.provenance[..narrow].copy_from_slice(&res.provenance[..narrow]);
                return Some(result);
            }
            if let Some((_, _, x)) = pattern::cast(q, v, CastOp::Trunc) {
                let res = self.run(x, depth + 1)?;
                let mut result = BitPart::new(res.provider, width);
                result.provenance.copy_from_slice(&res.provenance[..width as usize]);
                return Some(result);
            }
            // A `bitreverse`, most likely a partial one matched before.
            let call = call_of(q, self.declared, v);
            if let Some((_, args)) = call.as_ref().filter(|(base, _)| base == "bitreverse") {
                let res = self.run(args[0], depth + 1)?;
                let mut result = BitPart::new(res.provider, width);
                for at in 0..width as usize {
                    result.provenance[width as usize - 1 - at] = res.provenance[at];
                }
                return Some(result);
            }
            // A `bswap`, likewise.
            if let Some((_, args)) = call.as_ref().filter(|(base, _)| base == "bswap") {
                let res = self.run(args[0], depth + 1)?;
                let mut result = BitPart::new(res.provider, width);
                for byte in 0..width as usize / 8 {
                    let at = byte * 8;
                    for bit in 0..8 {
                        result.provenance[width as usize - 8 - at + bit] = res.provenance[at + bit];
                    }
                }
                return Some(result);
            }
            // Funnel shifts by a constant: `fshr` is an `fshl` by the other
            // way round.
            for (base, right) in [("fshl", false), ("fshr", true)] {
                let Some((_, args)) = call.as_ref().filter(|(name, _)| name == base) else { continue };
                let [x, y, amount] = **args else { continue };
                let Some((amount, _)) = q.int_constant(amount) else { continue };
                let mut modulo = (amount % u128::from(width)) as usize;
                if right {
                    modulo = width as usize - modulo;
                }
                if !self.bitreversals && modulo % 8 != 0 {
                    return None;
                }
                let lhs = self.run(x, depth + 1)?;
                let rhs = self.run(y, depth + 1)?;
                if lhs.provider != rhs.provider {
                    return None;
                }
                let start = width as usize - modulo;
                let mut result = BitPart::new(lhs.provider, width);
                for at in 0..start {
                    result.provenance[at + modulo] = lhs.provenance[at];
                }
                for at in 0..modulo {
                    result.provenance[at] = rhs.provenance[at + start];
                }
                return Some(result);
            }
        }
        // Once a root is found, these are never merged back together.
        if self.found_root {
            return None;
        }
        // Not a shift, `or`, `and` or the like: the root input.
        self.found_root = true;
        let mut result = BitPart::new(v, width);
        for (at, one) in result.provenance.iter_mut().enumerate() {
            *one = at as i8;
        }
        Some(result)
    }
}

fn correct_for_bswap(
    from: u32,
    to: u32,
    width: u32,
) -> bool {
    if from % 8 != to % 8 {
        return false;
    }
    // Byte indices: a byte reversal.
    // unsigned wrap, as the pin's `BitWidth / 8 - To / 8 - 1`
    from >> 3 == (width >> 3).wrapping_sub(to >> 3).wrapping_sub(1)
}

fn correct_for_bitreverse(
    from: u32,
    to: u32,
    width: u32,
) -> bool {
    from == width.wrapping_sub(to).wrapping_sub(1)
}

/// `matchBSwapOrBitReverse`: `inst` (an `or`) is a `bswap` or `bitreverse` of
/// one value, or of its low bits, where the pin recognises it.
pub fn match_bswap_or_bitreverse(
    c: &mut Combiner,
    inst: InstId,
    match_bswaps: bool,
    match_bitreversals: bool,
) -> Option<Visit> {
    if !match_bswaps && !match_bitreversals {
        return None;
    }
    let value = Operand::Value(c.function.instruction(inst).result.expect("an or"));
    let ty = c.type_of(value);
    let width = c.context.types.int_bits(ty)?;
    // Can't do a single bit, nor wider than 128.
    if width == 1 || width > 128 {
        return None;
    }
    let (provider, provenance) = {
        let q = query(c, Some(inst));
        let mut collect = Collect {
            q: &q,
            declared: &*c.declared,
            bitreversals: match_bitreversals,
            parts: HashMap::default(),
            found_root: false,
        };
        let res = collect.run(value, 0)?;
        (res.provider, res.provenance)
    };
    // If the upper bits are zero, perform it as a truncated operation.
    let mut used = provenance.len();
    while used > 0 && provenance[used - 1] == UNSET {
        used -= 1;
    }
    if used == 0 {
        return None;
    }
    let demanded_width = used as u32;
    let provenance = &provenance[..used];
    // Is the permutation a byte swap (an even number of bytes) or a bit
    // reversal?
    let mut mask: u128 = llrm_mir::context::mask(demanded_width);
    let mut ok_bswap = match_bswaps && demanded_width % 16 == 0;
    let mut ok_bitreverse = match_bitreversals;
    for (at, &from) in provenance.iter().enumerate() {
        if !(ok_bswap || ok_bitreverse) {
            break;
        }
        if from == UNSET {
            mask &= !(1u128 << at);
            continue;
        }
        ok_bswap &= correct_for_bswap(from as u32, at as u32, demanded_width);
        ok_bitreverse &= correct_for_bitreverse(from as u32, at as u32, demanded_width);
    }
    let base = if ok_bswap {
        "bswap"
    } else if ok_bitreverse {
        "bitreverse"
    } else {
        return None;
    };
    let demanded_ty = c.context.types.int(demanded_width);
    let mut provider_value = provider;
    // The provider may need truncating (or widening).
    let provider_width = c.context.types.int_bits(c.type_of(provider))?;
    if provider_width > demanded_width {
        provider_value = c.create_cast(inst, CastOp::Trunc, provider_value, demanded_ty, Flags::default());
    } else if provider_width < demanded_width {
        provider_value = c.create_cast(inst, CastOp::ZExt, provider_value, demanded_ty, Flags::default());
    }
    let mut result = c.create_intrinsic(inst, base, demanded_ty, &[provider_value]);
    if mask != llrm_mir::context::mask(demanded_width) {
        let mask = c.int(demanded_ty, mask);
        result = c.create_binop(inst, BinaryOp::And, result, mask, Flags::default());
    }
    // And back to the result type.
    if demanded_ty != ty {
        result = c.create_cast(inst, CastOp::ZExt, result, ty, Flags::default());
    }
    Some(Visit::Replace(result))
}

/// A call of the funnel shifts, `bswap` or `bitreverse` that `v` is, with its
/// arguments.
fn reorder<'a>(
    q: &Query<'a>,
    declared: &Declared,
    v: Operand,
) -> Option<(IntrinsicId, &'a [Operand])> {
    let (base, args) = call_of(q, declared, v)?;
    let id = match base.as_str() {
        "fshl" => IntrinsicId::Fshl,
        "fshr" => IntrinsicId::Fshr,
        "bswap" => IntrinsicId::Bswap,
        "bitreverse" => IntrinsicId::Bitreverse,
        _ => return None,
    };
    Some((id, args))
}

/// `foldBitwiseLogicWithIntrinsics`: `and`, `or` or `xor` of
/// `fshl(A, B, S)` and `fshl(C, D, S)` is `fshl` of the operations, and of two
/// `bswap`s or `bitreverse`s, or of one and a constant, is that of the
/// operation.
pub fn fold_bitwise_logic_with_intrinsics(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !has_one_use(c, op0) {
        return None;
    }
    let op = match c.function.instruction(inst).opcode {
        Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) => op,
        _ => return None,
    };
    let (id, x, y, constant) = {
        let q = query(c, Some(inst));
        let (id, x) = reorder(&q, &*c.declared, op0)?;
        let x = x.to_vec();
        let second = reorder(&q, &*c.declared, op1);
        if let Some((other, _)) = second
            && (!has_one_use(c, op1) || other != id)
        {
            return None;
        }
        // A constant right side, only for the reversals.
        let constant = match second {
            Some(_) => None,
            None => {
                if !matches!(id, IntrinsicId::Bswap | IntrinsicId::Bitreverse) {
                    return None;
                }
                Some(q.int_constant(op1)?)
            }
        };
        (id, x, second.map(|(_, args)| args.to_vec()), constant)
    };
    let ty = c.type_of(op0);
    match id {
        IntrinsicId::Fshl | IntrinsicId::Fshr => {
            let y = y?;
            if x[2] != y[2] {
                return None;
            }
            let first = c.create_binop(inst, op, x[0], y[0], Flags::default());
            let second = c.create_binop(inst, op, x[1], y[1], Flags::default());
            let base = if id == IntrinsicId::Fshl { "fshl" } else { "fshr" };
            Some(Visit::Replace(c.create_intrinsic(inst, base, ty, &[first, second, x[2]])))
        }
        _ => {
            let base = if id == IntrinsicId::Bswap { "bswap" } else { "bitreverse" };
            let other = match (&y, constant) {
                (Some(y), _) => y[0],
                (None, Some((bits, width))) => {
                    let moved = if id == IntrinsicId::Bswap {
                        (bits << (128 - width)).swap_bytes()
                    } else {
                        bits.reverse_bits() >> (128 - width)
                    };
                    c.int(ty, moved)
                }
                (None, None) => return None,
            };
            let inner = c.create_binop(inst, op, x[0], other, Flags::default());
            Some(Visit::Replace(c.create_intrinsic(inst, base, ty, &[inner])))
        }
    }
}

#[cfg(test)]
#[path = "combine_bitorder_tests.rs"]
mod tests;
