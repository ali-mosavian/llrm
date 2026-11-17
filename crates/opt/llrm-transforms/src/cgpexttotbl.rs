//! A zero extension of an `i8` vector in a loop's header, written as the
//! shuffle that `tbl` makes: `AArch64TargetLowering::
//! optimizeExtendOrTruncateConversion` (the zext arm) and
//! `createTblShuffleForZExt`.
//!
//! # How it works
//! `zext <16 x i8> %x to <16 x i32>` becomes `shufflevector %x, <0, poison...>`
//! whose mask puts each byte in the low byte of a lane (the other bytes read
//! the zero), a bitcast to the wide lanes and, where the extension was wider
//! than the lanes of that shuffle, a `zext` of those. The DAG turns the shuffle
//! into a zero extension of the vector's elements (`ZERO_EXTEND_VECTOR_INREG`),
//! `tbl` for the table of the extension masks, which a loop reuses.
//!
//! Only where the extension is in the header of its loop and the function is
//! not optimized for size: the masks are loaded once for the loop but make
//! code the pin does not want elsewhere. Not where the extension to the next
//! power of two is free for its users (`uaddw`, `umlal`), or where a `mul` of a
//! sign extension takes it in (`smull`).
//!
//! # Example
//! In a loop header, `zext <8 x i8> %v to <8 x i32>` is `shufflevector <8 x i8>
//! %v, <8 x i8> <0, poison...>, <32 x i32> <0, 8, 8, 8, 1, 8, 8, 8, ...>`,
//! `bitcast <32 x i8> to <8 x i32>`.
//!
//! # DEVIATE rows
//! - TBL-ARMS the `uitofp`, `sitofp`, `fptoui` and `trunc` arms of the hook are
//!   not made. Exit: 2026-11-27 (rows in `docs/working/above-main-rows.md`).
//! - TBL-DOT the `partial_reduce` user (`udot`) that stops the pin's zext arm
//!   is not known here: the intrinsic has no recipe yet.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::{ext_users, vector_of};
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::tti::{CastKind, CostKind, Tti};
use llrm_mir::types::{Type, TypeId};

/// `createTblShuffleMask`, little endian: the mask of the shuffle that spreads
/// `lanes` bytes into lanes `dst_bits` wide, or `None` where the pin gives up
/// (`dst_bits` not a byte multiple, 16 or less, or over 64).
fn tbl_mask(
    src_bits: u32,
    dst_bits: u32,
    lanes: u32,
) -> Option<Vec<i32>> {
    if dst_bits % 8 != 0 || dst_bits <= 16 || dst_bits > 64 {
        return None;
    }
    let factor = (dst_bits / src_bits) as usize;
    let mut mask = vec![lanes as i32; lanes as usize * factor];
    for (source, at) in (0..mask.len()).step_by(factor).enumerate() {
        mask[at] = source as i32;
    }
    Some(mask)
}

/// Whether `zext` has the form the pin rewrites and what it is rewritten to
/// (`dst_elements`: the lane width of the shuffle's bitcast).
fn plan(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    zext: InstId,
) -> Option<(u32, u32, u32)> {
    let instruction = function.instruction(zext);
    let src_ty = function.operand_type(context, instruction.operands[0])?;
    let (Type::Vector { element: src_elt, count }, Type::Vector { element: dst_elt, .. }) =
        (context.types.get(src_ty).clone(), context.types.get(instruction.ty).clone())
    else {
        return None;
    };
    let (src_bits, dst_bits) = (context.types.int_bits(src_elt)?, context.types.int_bits(dst_elt)?);
    if src_bits != 8 || dst_bits % 8 != 0 {
        return None;
    }
    let mut shuffled = dst_bits;
    // The extension to the next power of two, with the rest folded into the
    // user, is already one instruction.
    let truncated_elt = context.types.intern(Type::Int(dst_bits / 2));
    let truncated = context.types.intern(Type::Vector { element: truncated_elt, count });
    let users = ext_users(context, function, function.instruction(zext).result?);
    let free = tti
        .vector_ext_cost(
            CastKind::ZExt,
            vector_of(context, instruction.ty)?,
            vector_of(context, truncated)?,
            vector_of(context, src_ty)?,
            &users,
            CostKind::SizeAndLatency,
        )
        .is_some_and(|cost| cost.valid && cost.value == 0);
    if free {
        if src_bits * 2 >= dst_bits / 2 {
            return None;
        }
        shuffled = dst_bits / 2;
    }
    // `mul(zext, sext)` is `smull` with one extension implicit.
    if src_bits * 4 <= dst_bits
        && function.users(function.instruction(zext).result?).iter().all(|one| {
            let user = function.instruction(one.user);
            matches!(user.opcode, Opcode::Binary(BinaryOp::Mul))
                && matches!(
                    user.operands[1 - one.index as usize],
                    Operand::Value(other) if matches!(
                        function.value(other).def,
                        llrm_mir::module::ValueDef::Instruction(def)
                            if matches!(function.instruction(def).opcode, Opcode::Cast(CastOp::SExt))
                    )
                )
        })
    {
        return None;
    }
    if shuffled >= 64 {
        return None;
    }
    Some((src_bits, shuffled, count))
}

/// The zero extensions of `function` that are in `headers` rewritten; whether
/// any was. `headers`: the blocks that are the header of their loop.
pub fn zexts_through_tbl(
    context: &Context,
    function: &mut Function,
    tti: &dyn Tti,
    headers: &[BlockId],
) -> bool {
    let mut changed = false;
    for &header in headers {
        let zexts: Vec<InstId> = function
            .iter_block(header)
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Cast(CastOp::ZExt)))
            .collect();
        for zext in zexts {
            let Some((src_bits, shuffled, lanes)) = plan(context, function, tti, zext) else { continue };
            let Some(mask) = tbl_mask(src_bits, shuffled, lanes) else { continue };
            let wide_ty = function.instruction(zext).ty;
            let source = function.instruction(zext).operands[0];
            let Some(src_ty) = function.operand_type(context, source) else { continue };
            let Type::Vector { element: i8_ty, .. } = context.types.get(src_ty).clone() else { continue };
            let shuffled_elt = context.types.intern(Type::Int(shuffled));
            let shuffled_ty = context.types.intern(Type::Vector { element: shuffled_elt, count: lanes });
            let bytes_ty = context.types.intern(Type::Vector { element: i8_ty, count: mask.len() as u32 });
            let index_ty = context.types.intern(Type::Int(32));
            let at = Position::Before(zext);
            let mut insert = |function: &mut Function, opcode: Opcode, ty: TypeId, operands: Vec<Operand>| {
                let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
                function.insert(inst, at).expect("a placed instruction");
                Operand::Value(function.instruction(inst).result.expect("a value"))
            };
            let poison = Operand::Constant(context.constant(Constant { ty: src_ty, kind: ConstantKind::Poison }));
            let zero_byte = Operand::Constant(context.constant(Constant { ty: i8_ty, kind: ConstantKind::Int(0) }));
            let first = Operand::Constant(context.constant(Constant { ty: index_ty, kind: ConstantKind::Int(0) }));
            let first_zero = insert(function, Opcode::InsertElement, src_ty, vec![poison, zero_byte, first]);
            let spread = insert(function, Opcode::ShuffleVector(mask), bytes_ty, vec![source, first_zero]);
            let mut result = insert(function, Opcode::Cast(CastOp::BitCast), shuffled_ty, vec![spread]);
            if shuffled_ty != wide_ty {
                result = insert(function, Opcode::Cast(CastOp::ZExt), wide_ty, vec![result]);
            }
            let old = function.instruction(zext).result.expect("a value");
            function.replace_all_uses_with(old, result);
            function.erase(zext).expect("the old extension");
            changed = true;
        }
    }
    changed
}
