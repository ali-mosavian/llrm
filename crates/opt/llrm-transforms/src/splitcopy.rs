//! A `memcpy` into a local nothing else can reach becomes the loads and
//! stores of the bytes the local is read as.
//!
//! # How it works
//! The copy must be an `llvm.memcpy` of a constant, positive length, not
//! volatile, between two pointers that resolve to a known offset into
//! different objects, the destination an `alloca`.
//!
//! The destination's users are walked through `gep` and `addrspacecast`. Each
//! one must be a non-volatile load, a store into it, a lifetime marker, the
//! copy itself, or another `memcpy` that writes it. Any other use (passed to a
//! call, stored as a value, compared, read as the source of another `memcpy`)
//! leaves the copy whole, because someone else may read bytes nobody named.
//!
//! The loads inside the copied range are its leaves: an offset and a type
//! each. Leaves must not overlap unless equal, and no load may straddle the
//! range's edge. Each leaf becomes a load from the source and a store to the
//! destination in place of the copy; a byte nothing reads is not copied. A
//! destination with no leaf drops the copy.
//!
//! ```text
//!   memcpy(%to, %from, 8)            %v0 = load i16, %from
//!   %a = load i16, %to        ==>    store %v0, %to
//!   %b = load i16, %to+4             %v1 = load i16, %from+4
//!                                    store %v1, %to+4
//!                                    (the loads of %to stay)
//! ```
//!
//! The pass repeats until no copy splits. A destination another copy reads is
//! split once that later copy has been, so a copy of a copy is the loads of
//! the first source.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//!   %from = alloca [8 x i8]
//!   %to = alloca [8 x i8]
//!   store i16 %x, ptr %from
//!   %p = getelementptr i8, ptr %from, i16 4
//!   store i16 7, ptr %p
//!   call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
//!   %a = load i16, ptr %to
//!   %q = getelementptr i8, ptr %to, i16 4
//!   %b = load i16, ptr %q
//!   %r = add i16 %a, %b
//!   ret i16 %r
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//!   %from = alloca [8 x i8]
//!   %to = alloca [8 x i8]
//!   store i16 %x, ptr %from
//!   %p = getelementptr i8, ptr %from, i16 4
//!   store i16 7, ptr %p
//!   %1 = load i16, ptr %from
//!   store i16 %1, ptr %to
//!   %2 = getelementptr i8, ptr %from, i16 4
//!   %3 = load i16, ptr %2
//!   %4 = getelementptr i8, ptr %to, i16 4
//!   store i16 %3, ptr %4
//!   %a = load i16, ptr %to
//!   %q = getelementptr i8, ptr %to, i16 4
//!   %b = load i16, ptr %q
//!   %r = add i16 %a, %b
//!   ret i16 %r
//! }
//! ```
//! `%to` is read as two words, at 0 and 4, so the 8-byte `memcpy` is a load and
//! a store of each; the bytes nothing reads are not copied. `Sroa` then
//! promotes the words.
//!
//! From unit test
//! `test_a_memcpy_into_a_private_local_is_the_loads_of_what_is_read` in
//! `splitcopy_tests.rs`.
//!
//! # Runs
//! Inside [`crate::promote`]'s `Sroa`, before it scalarizes the aggregate's
//! leaves, so the new loads and stores are promoted by the same run. `Sroa`
//! is a structural-boundary pass: it runs at the start of the body's
//! pipeline and again after unroll and peel copy a loop. It is
//! off by `-fno-tree-sra` and at
//! -O0.
//!
//! # See also
//! LLVM's SROA slicing an alloca's memcpys (`AllocaSlices`,
//! `visitMemTransferInst`).

use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::underlying;

use crate::counting;

/// What a copy's destination is read as: a leaf's offset in the copied
/// bytes, and its type.
type Leaf = (i64, TypeId);

/// `function` with each splittable memcpy replaced. Whether it changed.
pub fn split(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut changed = false;
    while let Some((copy, leaves)) = {
        let unit = Unit::within(context, layout, function, outer);
        splittable(&unit)
    } {
        replace(context, function, copy, &leaves);
        changed = true;
    }
    changed
}

/// The first memcpy to split and the leaves its destination is read as.
fn splittable(unit: &Unit) -> Option<(InstId, Vec<Leaf>)> {
    unit.function
        .walk()
        .find_map(
            |(_, inst)| {
                if unit.intrinsic(inst) != Some(Intrinsic::MemCpy) {
                    return None;
                }
                let [to, from, length, volatile, _] = unit.function.instruction(inst).operands[..] else { return None };
                let (bytes, volatile) =
                    (llrm_mir::intrinsics::length(unit.context, length)? as i64, constant(unit.context, volatile)?);
                if volatile != 0 || bytes <= 0 {
                    return None;
                }
                let (target, at) = underlying(unit.context, unit.layout, unit.function, to);
                let (source, _) = underlying(unit.context, unit.layout, unit.function, from);
                let (Operand::Value(slot), Some(at)) = (target, at) else { return None };
                if !is_alloca(unit.function, slot) || source == target {
                    return None;
                }
                let leaves = leaves(unit, slot, (at, at + bytes), inst)?;
                Some((inst, leaves.into_iter().map(|(low, ty)| (low - at, ty)).collect()))
            },
        )
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<i64> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => i64::try_from(bits).ok(),
            _ => None,
        },
        _ => None,
    }
}

fn is_alloca(
    function: &Function,
    value: ValueId,
) -> bool {
    matches!(
        function.value(value).def,
        ValueDef::Instruction(inst) if matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
    )
}

/// What `slot` is read as inside `range`, if everything that touches it is a
/// plain load, a store into it, a lifetime marker or the memcpy `copy` that
/// fills it: each load's offset and type, disjoint or equal.
fn leaves(
    unit: &Unit,
    slot: ValueId,
    range: (i64, i64),
    copy: InstId,
) -> Option<Vec<Leaf>> {
    let mut found: Vec<(i64, i64, TypeId)> = Vec::new();
    let mut work = vec![slot];
    let mut seen = vec![slot];
    while let Some(value) = work.pop() {
        for one in unit.function.users(value) {
            let instruction = unit.function.instruction(one.user);
            match &instruction.opcode {
                Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::AddrSpaceCast) => {
                    let derived = instruction.result?;
                    if underlying(unit.context, unit.layout, unit.function, Operand::Value(derived)).1.is_none() {
                        return None;
                    }
                    if !seen.contains(&derived) {
                        seen.push(derived);
                        work.push(derived);
                    }
                }
                Opcode::Load { volatile: false, .. } if one.index == 0 => {
                    let (_, at) = underlying(unit.context, unit.layout, unit.function, Operand::Value(value));
                    let width = i64::try_from(unit.layout.store_size(&unit.context.types, instruction.ty)).ok()?;
                    let at = at?;
                    found.push((at, at + width, instruction.ty));
                }
                Opcode::Store { volatile: false, .. } if one.index == 1 => {}
                Opcode::Call(_) => match unit.intrinsic(one.user) {
                    Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd) => {}
                    // The destination of a copy writes; its source reads bytes
                    // no one has named.
                    Some(Intrinsic::MemCpy) if one.index == 0 || one.user == copy => {}
                    _ => return None,
                },
                _ => return None,
            }
        }
    }
    let (low, high) = range;
    let mut inside: Vec<(i64, i64, TypeId)> = Vec::new();
    for (from, to, ty) in found {
        if to <= low || from >= high {
            continue;
        }
        if from < low || to > high {
            return None;
        }
        inside.push((from, to, ty));
    }
    inside.sort_by_key(|&(from, to, _)| (from, to));
    inside.dedup();
    for pair in inside.windows(2) {
        if pair[0].1 > pair[1].0 {
            return None;
        }
    }
    Some(inside.into_iter().map(|(from, _, ty)| (from, ty)).collect())
}

/// The memcpy `copy` as a load and a store per leaf.
fn replace(
    context: &mut Context,
    function: &mut Function,
    copy: InstId,
    leaves: &[Leaf],
) {
    let [to, from, ..] = function.instruction(copy).operands[..] else {
        unreachable!("a memcpy has a destination and a source")
    };
    let byte = context.types.int(8);
    let at = |function: &mut Function, context: &mut Context, pointer: Operand, offset: i64| -> Operand {
        if offset == 0 {
            return pointer;
        }
        let ty = function.operand_type(context, pointer).expect("a typed pointer");
        let offset = counting::constant(context, &BigInt::from(offset), 16);
        let gep = function.create_instruction(
            Opcode::GetElementPtr { source: byte },
            ty,
            vec![pointer, offset],
            Flags::default(),
            None,
        );
        function.insert(gep, Position::Before(copy)).expect("a placed copy");
        Operand::Value(function.instruction(gep).result.expect("a pointer"))
    };
    let void = context.types.void();
    for &(offset, ty) in leaves {
        let source = at(function, context, from, offset);
        let load = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            ty,
            vec![source],
            Flags::default(),
            None,
        );
        function.insert(load, Position::Before(copy)).expect("a placed copy");
        let value = Operand::Value(function.instruction(load).result.expect("a loaded value"));
        let destination = at(function, context, to, offset);
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![value, destination],
            Flags::default(),
            None,
        );
        function.insert(store, Position::Before(copy)).expect("a placed copy");
    }
    function.erase(copy).expect("a copy has no result");
}

#[cfg(test)]
#[path = "splitcopy_tests.rs"]
mod tests;
