//! CodeGenPrepare's `splitMergedValStore`: a store of two values merged into
//! one wider integer is two stores
//! ([CodeGenPrepare.cpp#L8608](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8608)).
//!
//! # How it works
//! `std::make_pair(int, float)` passed by reference is, after SROA, a store of
//! `zext(bitcast f) | (zext(i) << 32)`. The target decides whether two stores
//! beat the merge (`isMultiStoresCheaperThanBitsMerge`; x86: a float and an
//! integer). The store is simple (not volatile), the value an integer of whole
//! bytes with halves of whole bytes, the `or` commutative, and the `zext`s and
//! the `shl` (by half the width) each read once. The low half goes to the
//! address and the high half half a width above (the other way round on a
//! big-endian target), at the alignment the offset allows.
//!
//! ```text
//!  %lz = zext i32 %b to i64                 store i32 %b, ptr %p, align 8
//!  %hz = zext i32 %i to i64                 %g = getelementptr i32, ptr %p, i32 1
//!  %hs = shl i64 %hz, 32            ==>     store i32 %i, ptr %g, align 4
//!  %o  = or i64 %lz, %hs
//!  store i64 %o, ptr %p, align 8
//! ```
//!
//! MIRROR: the whole function, in the pin's order of tests.
//!
//! DEVIATE SPLIT1: the `-force-split-store` option (off by default) is not
//! read. Exit: none.
//!
//! DEVIATE SPLIT2: the `or`, `shl` and `zext`s the store read go with it; the
//! pin leaves them for the dead code pass that follows.
//!
//! # Example
//! `test_a_float_and_an_integer_merged_into_a_store_are_two_stores` in
//! `splitstore_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, before the address-mode sinking (the stores it makes
//! are sunk in turn).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::lowering::{Domain, Lowering};
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::Type;

/// The instruction defining `operand`, if it is the only reader's.
fn defined_once(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(def) = function.value(value).def else { return None };
    function.users(value).has_n_uses(1).then_some(def)
}

/// `m_OneUse(m_ZExt(m_Value(V)))`: the `zext` and what it extends.
fn zext_of(
    function: &Function,
    operand: Operand,
) -> Option<(InstId, Operand)> {
    let def = defined_once(function, operand)?;
    (function.instruction(def).opcode == Opcode::Cast(CastOp::ZExt))
        .then(|| (def, function.instruction(def).operands[0]))
}

/// `m_OneUse(m_Shl(m_OneUse(m_ZExt(m_Value(V))), m_SpecificInt(half)))`: the
/// `shl`, the `zext` and what it extends.
fn shifted_zext_of(
    function: &Function,
    context: &Context,
    operand: Operand,
    half: u32,
) -> Option<(InstId, InstId, Operand)> {
    let shl = defined_once(function, operand)?;
    let one = function.instruction(shl);
    if one.opcode != Opcode::Binary(BinaryOp::Shl) {
        return None;
    }
    let &[value, Operand::Constant(amount)] = &one.operands[..] else { return None };
    if context.get(amount).kind != ConstantKind::Int(u128::from(half)) {
        return None;
    }
    let (zext, extended) = zext_of(function, value)?;
    Some((shl, zext, extended))
}

/// What `value` is to the target: that of the operand of a `bitcast`, else its
/// own.
fn domain(
    function: &Function,
    context: &Context,
    value: Operand,
) -> Domain {
    let mut at = value;
    if let Operand::Value(v) = value
        && let ValueDef::Instruction(def) = function.value(v).def
        && function.instruction(def).opcode == Opcode::Cast(CastOp::BitCast)
    {
        at = function.instruction(def).operands[0];
    }
    match function.operand_type(context, at).map(|ty| context.types.get(ty)) {
        Some(Type::Int(_)) => Domain::Integer,
        Some(Type::Float(_)) => Domain::Float,
        _ => Domain::Other,
    }
}

/// `commonAlignment(align, offset)`: the alignment an address `offset` bytes
/// above one of `align` has.
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    let both = align | offset;
    both & both.wrapping_neg()
}

/// `splitMergedValStore(store)`: whether it was split.
fn split(
    context: &Context,
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
    store: InstId,
) -> bool {
    let one = function.instruction(store).clone();
    let Opcode::Store { align, volatile } = one.opcode else { return false };
    let &[value, pointer] = &one.operands[..] else { return false };
    let Some(store_ty) = function.operand_type(context, value) else { return false };
    let Type::Int(bits) = *context.types.get(store_ty) else { return false };
    // Whole bytes, in halves of whole bytes (`typeSizeEqualsStoreSize`).
    if bits == 0 || bits % 16 != 0 || volatile {
        return false;
    }
    let half = bits / 2;
    let Some(or) = (match value {
        Operand::Value(v) => match function.value(v).def {
            ValueDef::Instruction(def) if function.instruction(def).opcode == Opcode::Binary(BinaryOp::Or) => Some(def),
            _ => None,
        },
        _ => None,
    }) else {
        return false;
    };
    // `m_c_Or`: either operand may be the shifted one.
    let &[first, second] = &function.instruction(or).operands[..] else { return false };
    let found = [(first, second), (second, first)]
        .into_iter()
        .find_map(
            |(low, high)| {
                let (low_zext, low_value) = zext_of(function, low)?;
                let (shl, high_zext, high_value) = shifted_zext_of(function, context, high, half)?;
                Some((low_zext, low_value, shl, high_zext, high_value))
            },
        );
    let Some((low_zext, low_value, shl, high_zext, high_value)) = found else { return false };
    let narrow = |operand: Operand| {
        function
            .operand_type(context, operand)
            .and_then(|ty| context.types.int_bits(ty))
            .is_some_and(|width| width <= half)
    };
    if !narrow(low_value) || !narrow(high_value) {
        return false;
    }
    if !lowering.is_multi_stores_cheaper_than_bits_merge(
        domain(function, context, low_value),
        domain(function, context, high_value),
    ) {
        return false;
    }

    let split_ty = context.types.int(half);
    let i32_ty = context.types.int(32);
    let void = one.ty;
    let block = function.parent(store).expect("a placed store");
    // A bitcast of another block is made again beside the store, for the
    // combiner to merge with the stores (`LBC->getParent() != SI.getParent()`).
    let here = |function: &mut Function, operand: Operand| -> Operand {
        if let Operand::Value(v) = operand
            && let ValueDef::Instruction(def) = function.value(v).def
            && function.instruction(def).opcode == Opcode::Cast(CastOp::BitCast)
            && function.parent(def) != Some(block)
        {
            let cast = function.clone_instruction(def);
            function.insert(cast, Position::Before(store)).expect("a position before the store");
            return Operand::Value(function.instruction(cast).result.expect("a cast has a value"));
        }
        operand
    };
    let (low_value, high_value) = (here(function, low_value), here(function, high_value));
    let base = align.unwrap_or_else(|| layout.align(&context.types, store_ty));
    let little = !layout.big_endian;
    let mut halves = Vec::new();
    for (value, upper) in [(low_value, false), (high_value, true)] {
        // `CreateZExtOrBitCast`: the halves are integers no wider than it.
        let widened = if function.operand_type(context, value) == Some(split_ty) {
            value
        } else {
            let cast =
                function.create_instruction(Opcode::Cast(CastOp::ZExt), split_ty, vec![value], Flags::default(), None);
            function.insert(cast, Position::Before(store)).expect("a position before the store");
            Operand::Value(function.instruction(cast).result.expect("a cast has a value"))
        };
        let (mut address, mut aligned) = (pointer, base);
        if (little && upper) || (!little && !upper) {
            let pointer_ty = function.operand_type(context, pointer).expect("a typed address");
            let one_i32 = Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(1) }));
            let gep = function.create_instruction(
                Opcode::GetElementPtr { source: split_ty },
                pointer_ty,
                vec![pointer, one_i32],
                Flags::default(),
                None,
            );
            function.insert(gep, Position::Before(store)).expect("a position before the store");
            address = Operand::Value(function.instruction(gep).result.expect("a gep has a value"));
            aligned = common_alignment(base, u64::from(half / 8));
        }
        halves.push((widened, address, aligned));
    }
    for (widened, address, aligned) in halves {
        let made = function.create_instruction(
            Opcode::Store { align: Some(aligned), volatile: false },
            void,
            vec![widened, address],
            Flags::default(),
            None,
        );
        function.insert(made, Position::Before(store)).expect("a position before the store");
    }
    function.erase(store).expect("a store defines nothing");
    // DEVIATE SPLIT2.
    for dead in [or, shl, low_zext, high_zext] {
        if function.instruction(dead).result.is_some_and(|result| function.users(result).is_empty()) {
            function.erase(dead).expect("what nothing reads goes");
        }
    }
    true
}

/// `splitMergedValStore` for each store of `function`; whether any was split.
pub fn merged_stores_split(
    context: &Context,
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
) -> bool {
    let stores: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
        .collect();
    let mut changed = false;
    for store in stores {
        if !function.is_erased(store) {
            changed |= split(context, layout, lowering, function, store);
        }
    }
    changed
}

#[cfg(test)]
#[path = "splitstore_tests.rs"]
mod tests;
