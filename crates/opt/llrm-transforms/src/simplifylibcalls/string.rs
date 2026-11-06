//! The `strlen` and `strnlen` arms of `LibCallSimplifier`:
//! `optimizeStringLength` ([L971](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyLibCalls.cpp#L971)),
//! `optimizeStrLen` (L1092) and `optimizeStrNLen` (L1099), at d1106deb71cc.
//!
//! # Deviations
//! - DEVIATE S1 (owner w2, exit 2026-12-01): `annotateNonNullNoUndefBasedOnAccess`
//!   is not translated.
//! - DEVIATE S2 (owner w2, exit 2026-12-01): the `strlen(s + x)` and
//!   `strlen(select)` arms are untranslated; those calls stay.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::Operand;
use llrm_mir::opcode::{CallInfo, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Unit;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::nonzero::is_known_non_zero;

use super::compare::{build, first_byte, string_length};
use super::{Rewrite, Site};

/// `isOnlyUsedInZeroEqualityComparison`: every user is `icmp eq|ne X, 0`.
fn only_in_zero_equality(
    unit: &Unit,
    site: &Site,
) -> bool {
    let Some(result) = unit.function.instruction(site.call).result else { return true };
    unit.function.users(result).iter().all(|used| {
        let user = unit.function.instruction(used.user);
        matches!(user.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
            && matches!(
                user.operands[1],
                Operand::Constant(id) if matches!(unit.context.get(id).kind, ConstantKind::Int(0) | ConstantKind::Null)
            )
    })
}

fn constant_bound(
    unit: &Unit,
    bound: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = bound else { return None };
    match unit.context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// `B.CreateBinaryIntrinsic(umin, length, bound)` before the call.
fn umin(
    unit: &mut Unit,
    site: &Site,
    ty: TypeId,
    length: Operand,
    bound: Operand,
) -> Operand {
    let function_type =
        unit.context.types.intern(Type::Function { returns: ty, parameters: vec![ty, ty], variadic: false });
    let bits = unit.context.types.int_bits(ty).expect("an integer");
    let callee = unit.declared.declare(&format!("llvm.umin.i{bits}"), function_type);
    let pointer = unit.context.types.ptr(0);
    let callee = Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); 2],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    build(unit, site, Opcode::Call(Box::new(info)), ty, vec![length, bound, callee])
}

/// `optimizeStringLength` for 8-bit characters: the arms that need neither a
/// GEP offset nor a select.
fn string_length_of(
    unit: &mut Unit,
    site: &Site,
    source: Operand,
    bound: Option<Operand>,
) -> Option<Rewrite> {
    let ty = unit.function.instruction(site.call).ty;
    if only_in_zero_equality(unit, site) {
        let non_zero = bound.is_none_or(|bound| {
            let mut query = Query::bare(unit.context, unit.function, unit.layout);
            query.globals = site.env.globals;
            is_known_non_zero(&query, bound, 0)
        });
        if non_zero {
            return Some(first_byte(unit, site, source, false));
        }
    }
    if let Some(bound) = bound.and_then(|bound| constant_bound(unit, bound)) {
        if bound == 0 {
            return Some(Rewrite::Value(Operand::Constant(unit.context.int(ty, 0))));
        }
        if bound == 1 {
            let byte = unit.context.types.int(8);
            let load = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![source]);
            let zero = Operand::Constant(unit.context.int(byte, 0));
            let i1 = unit.context.types.int(1);
            let differs = build(unit, site, Opcode::ICmp(IntPredicate::Ne), i1, vec![load, zero]);
            let wide = build(unit, site, Opcode::Cast(llrm_mir::opcode::CastOp::ZExt), ty, vec![differs]);
            return Some(Rewrite::Value(wide));
        }
    }
    let length = string_length(unit, site, source);
    if length == 0 {
        return None;
    }
    let length_constant = Operand::Constant(unit.context.int(ty, i128::from(length - 1)));
    let Some(bound) = bound else { return Some(Rewrite::Value(length_constant)) };
    if let Some(bound) = constant_bound(unit, bound) {
        return Some(Rewrite::Value(Operand::Constant(unit.context.int(ty, (u128::from(length - 1)).min(bound) as i128))));
    }
    Some(Rewrite::Value(umin(unit, site, ty, length_constant, bound)))
}

/// `optimizeStrLen`.
pub(super) fn str_len(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.clone();
    string_length_of(unit, site, *operands.first()?, None)
}

/// `optimizeStrNLen`.
pub(super) fn str_n_len(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.clone();
    string_length_of(unit, site, *operands.first()?, Some(*operands.get(1)?))
}
