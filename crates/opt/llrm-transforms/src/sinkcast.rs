//! CodeGenPrepare's `SinkCast` and `OptimizeNoopCopyExpression`: a cast read in
//! other blocks is made again in each, where isel takes it as a register
//! rename
//! ([CodeGenPrepare.cpp#L1432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1432)).
//!
//! # How it works
//! Isel works a block at a time: a cast whose result crosses a block keeps the
//! narrow and the wide value both live. Two kinds of cast are made again in the
//! blocks that read them (one copy per block; a phi's read is in the block it
//! comes from), and the original goes once nothing reads it:
//! - a `zext` or `sext` to a type no register holds (`i64` here), which splits
//!   into two halves anyway (`TypeExpandInteger`);
//! - a cast that changes nothing for the registers: both types are integers (a
//!   pointer is one) and, once an integer narrower than a register is promoted
//!   to the register, they are the same type (`OptimizeNoopCopyExpression`).
//!
//! ```text
//!  b1: %p = ptrtoint ptr %a to i16        b1: ...
//!      br label %b2                       b2: %p = ptrtoint ptr %a to i16
//!  b2: store i16 %p, ptr %q                   store i16 %p, ptr %q
//! ```
//!
//! MIRROR: `SinkCast`, the two entry conditions, the promotion of a narrow
//! integer to the register that holds it. The address-space cast (a free one
//! is a target's: x86 has none) is left alone, except as below.
//!
//! DEVIATE CGP3 (owner sdag-c; exit: a target hook for a free or cheap
//! address-space cast, as the pin's): a cast of an `inttoptr` (a
//! segment number) to a wider pointer, a selector and a zero offset, is made
//! again in each block that reads it. Held across a loop its zero offset sat in
//! a register (`xor bx,bx; es:[bx+si+88]`, nbody frame 94 -> 206 B); made in
//! the reading block the address matcher drops it.
//!
//! # Example
//! `test_a_pointer_cast_read_in_another_block_is_made_there` in
//! `sinkcast_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, before `optimizeExt`.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::types::Type;

/// The bits of an integer or pointer type: what `getValueType` makes of it.
fn value_bits(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Pointer(space) => Some(layout.pointer(*space).bits),
        _ => context.types.int_bits(ty),
    }
}

/// `getTypeToTransformTo` for a narrow integer: the register that holds it, as
/// `TypePromoteInteger` says; the type itself where a register holds it.
fn promoted(
    lowering: &dyn Lowering,
    bits: u32,
) -> u32 {
    if lowering.is_type_legal(bits) {
        return bits;
    }
    [8u32, 16, 32, 64].into_iter().find(|&legal| legal > bits && lowering.is_type_legal(legal)).unwrap_or(bits)
}

/// The widest register: a type wider than it is `TypeExpandInteger`.
fn widest(lowering: &dyn Lowering) -> u32 {
    [64u32, 32, 16, 8].into_iter().find(|&bits| lowering.is_type_legal(bits)).unwrap_or(0)
}

/// `SinkCast`: `cast` made again in each block that reads it. Whether any was.
fn sunk(
    function: &mut Function,
    cast: InstId,
) -> bool {
    let Some(result) = function.instruction(cast).result else { return false };
    let home = function.parent(cast).expect("a placed cast");
    let readers: Vec<(InstId, u32)> = function.users(result).iter().map(|one| (one.user, one.index)).collect();
    let mut made: Vec<(BlockId, Operand)> = Vec::new();
    let mut changed = false;
    for (reader, index) in readers {
        let mut block = function.parent(reader).expect("a placed reader");
        if function.instruction(reader).opcode == Opcode::Phi {
            let Operand::Block(from) = function.instruction(reader).operands[index as usize + 1] else { continue };
            block = from;
        }
        if block == home {
            continue;
        }
        let copy = match made.iter().find(|(seen, _)| *seen == block) {
            Some(&(_, copy)) => copy,
            None => {
                let first = function
                    .iter_block(block)
                    .find(|&one| function.instruction(one).opcode != Opcode::Phi)
                    .expect("a terminated block");
                let clone = function.clone_instruction(cast);
                function.insert(clone, Position::Before(first)).expect("a position before the first instruction");
                let copy = Operand::Value(function.instruction(clone).result.expect("a cast has a value"));
                made.push((block, copy));
                copy
            }
        };
        function.set_operand(reader, index as usize, copy);
        changed = true;
    }
    if function.users(result).is_empty() {
        function.erase(cast).expect("a cast nothing reads goes");
        changed = true;
    }
    changed
}

/// `SinkCast` for each cast the pin sinks; whether any was.
pub fn casts_sunk(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
) -> bool {
    let casts: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Cast(_)))
        .collect();
    let mut changed = false;
    for cast in casts {
        if function.is_erased(cast) {
            continue;
        }
        let one = function.instruction(cast).clone();
        let Opcode::Cast(kind) = one.opcode else { continue };
        // A constant source was folded, or placed away from its reader on
        // purpose.
        if matches!(one.operands[0], Operand::Constant(_)) {
            continue;
        }
        let Some(source) = function.operand_type(context, one.operands[0]) else { continue };
        let (Some(from), Some(to)) = (value_bits(context, layout, source), value_bits(context, layout, one.ty)) else {
            continue;
        };
        let integers = !matches!(
            kind,
            CastOp::FPToUI | CastOp::FPToSI | CastOp::UIToFP | CastOp::SIToFP
        );
        let noop = integers
            && kind != CastOp::AddrSpaceCast
            && from >= to
            && promoted(lowering, from) == promoted(lowering, to)
            && !matches!(kind, CastOp::FPExt | CastOp::FPTrunc);
        let expands = matches!(kind, CastOp::ZExt | CastOp::SExt) && to > widest(lowering);
        let farther = kind == CastOp::AddrSpaceCast
            && to > from
            && matches!(
                one.operands[0],
                Operand::Value(value) if matches!(
                    function.value(value).def,
                    ValueDef::Instruction(made) if function.instruction(made).opcode == Opcode::Cast(CastOp::IntToPtr)
                )
            );
        if (noop || expands || farther) && sunk(function, cast) {
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
#[path = "sinkcast_tests.rs"]
mod tests;
