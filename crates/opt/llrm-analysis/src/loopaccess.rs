//! `getPointersDiff`, `isConsecutiveAccess`, `getPtrStride` and the pointer
//! walks `LoopAccessInfo` is built on.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`: the
//! distance between two pointers in units of an element, from their constant
//! offsets off a common base, else from ScalarEvolution's constant
//! difference of the two; the constant stride of a pointer that is an affine
//! add recurrence of the loop, and whether it can wrap; the pointers a
//! phi or select forks an access into (`visitPointers`, `findForkedSCEVs`).
//!
//! # DEVIATE 24 (no predicated SCEV), 25 (no symbolic strides)
//! There is no `PredicatedScalarEvolution`: `getPtrStride` and `isNoWrap`
//! never add a predicate (their `Predicates` argument is null), and an
//! overflow they would have assumed is a pointer that cannot be checked. No
//! `Stride == 1` is ever speculated: `SymbolicStrideMap` stays empty
//! (`collectStridedAccess` records nothing) and `replaceSymbolicStrideSCEV` is
//! `getSCEV`. Exit: PredicatedScalarEvolution.
//!
//! # Example
//! `getelementptr i32, ptr %p, i64 1` is consecutive after `%p`.

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags as IrFlags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_support::hash::{HashMap, HashSet};

use crate::scev::expr::{Kind, Scev, Ty, Use};
use crate::scev::{Cx, Flags, ScalarEvolution};

/// `SymbolicStrideMap`: the pointers with a symbolic stride, and the stride
/// (D25: never filled).
pub type SymbolicStrideMap = HashMap<Operand, Scev>;

/// `NullPointerIsDefined`: the function says a null pointer is a valid one
/// (`null_pointer_is_valid`), whatever the address space.
pub(crate) fn null_pointer_is_defined(cx: &Cx) -> bool {
    cx.function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "null_pointer_is_valid"))
}

pub(crate) fn address_space(
    cx: &Cx,
    pointer: Operand,
) -> Option<u32> {
    let ty = cx.function.operand_type(&cx.context, pointer)?;
    match cx.context.types.get(ty) {
        Type::Pointer(space) => Some(*space),
        _ => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1805>
pub fn get_pointers_diff(
    cx: &Cx,
    se: &mut ScalarEvolution,
    elem_ty_a: TypeId,
    ptr_a: Operand,
    elem_ty_b: TypeId,
    ptr_b: Operand,
    strict_check: bool,
    check_type: bool,
) -> Option<i64> {
    // Make sure that A and B are different pointers.
    if ptr_a == ptr_b {
        return Some(0);
    }

    // Make sure that the element types are the same if required.
    if check_type && elem_ty_a != elem_ty_b {
        return None;
    }

    let asa = address_space(cx, ptr_a)?;
    let asb = address_space(cx, ptr_b)?;

    // Check that the address spaces match.
    if asa != asb {
        return None;
    }
    let idx_width = cx.layout.pointer(asa).index_bits;

    let query = cx.query(None);
    let (ptr_a1, offset_a) = strip_and_accumulate_constant_offsets(&query, ptr_a, idx_width, true);
    let (ptr_b1, offset_b) = strip_and_accumulate_constant_offsets(&query, ptr_b, idx_width, true);

    let val: Option<i64> = if ptr_a1 == ptr_b1 {
        // Retrieve the address space again as pointer stripping now tracks
        // through `addrspacecast`.
        let asa = address_space(cx, ptr_a1)?;
        let asb = address_space(cx, ptr_b1)?;
        // Check that the address spaces match and that the pointers are valid.
        if asa != asb {
            return None;
        }

        let idx_width = cx.layout.pointer(asa).index_bits;
        // `sextOrTrunc` from the width of the stripping, then the difference
        // in the new width.
        let source = cx.layout.pointer(address_space(cx, ptr_a)?).index_bits;
        let widen = |bits: u128| (signed(bits, source) as u128) & mask(idx_width);
        let difference = widen(offset_b).wrapping_sub(widen(offset_a)) & mask(idx_width);
        try_sext_value(difference, idx_width)
    } else {
        // Otherwise compute the distance with SCEV between the base pointers.
        let scev_a = se.get_scev(cx, ptr_a);
        let scev_b = se.get_scev(cx, ptr_b);
        let diff = se.compute_constant_difference(cx, scev_b, scev_a)?;
        try_sext_value(diff.bits, diff.width)
    };

    let val = val?;
    let size = cx.layout.store_size(&cx.context.types, elem_ty_a) as i64;
    let dist = val / size;

    // Ensure that the calculated distance matches the type-based one after
    // all the bitcasts removal in the provided pointers.
    (!strict_check || dist.wrapping_mul(size) == val).then_some(dist)
}

/// `APInt::trySExtValue`: the value, if it fits in 64 signed bits.
fn try_sext_value(
    bits: u128,
    width: u32,
) -> Option<i64> {
    i64::try_from(signed(bits, width)).ok()
}

/// `getLoadStorePointerOperand`.
pub(crate) fn load_store_pointer_operand(
    cx: &Cx,
    inst: InstId,
) -> Option<Operand> {
    let instruction = cx.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => Some(instruction.operands[0]),
        Opcode::Store { .. } => Some(instruction.operands[1]),
        _ => None,
    }
}

/// `getLoadStoreType`.
pub fn load_store_type(
    cx: &Cx,
    inst: InstId,
) -> Option<TypeId> {
    let instruction = cx.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => Some(instruction.ty),
        Opcode::Store { .. } => cx.function.operand_type(&cx.context, instruction.operands[0]),
        _ => None,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1915>
///
/// Whether the memory operations `a` and `b` are consecutive.
pub fn is_consecutive_access(
    cx: &Cx,
    se: &mut ScalarEvolution,
    a: InstId,
    b: InstId,
    check_type: bool,
) -> bool {
    let (Some(ptr_a), Some(ptr_b)) = (load_store_pointer_operand(cx, a), load_store_pointer_operand(cx, b)) else {
        return false;
    };
    let (Some(elem_ty_a), Some(elem_ty_b)) = (load_store_type(cx, a), load_store_type(cx, b)) else { return false };
    get_pointers_diff(cx, se, elem_ty_a, ptr_a, elem_ty_b, ptr_b, true, check_type) == Some(1)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L157
/// `replaceSymbolicStrideSCEV`: the SCEV of `ptr` (D25: no pointer has a
/// symbolic stride, so there is nothing to replace).
pub fn replace_symbolic_stride_scev(
    se: &mut ScalarEvolution,
    cx: &Cx,
    strides: &SymbolicStrideMap,
    ptr: Operand,
) -> Scev {
    debug_assert!(strides.is_empty(), "D25: symbolic strides are never recorded");
    se.get_scev(cx, ptr)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1050
/// `getStrideFromAddRec`: the constant stride of `ar` in elements of
/// `access_ty`, if it strides over the loop `lp` by a whole number of them.
pub fn get_stride_from_add_rec(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ar: Scev,
    lp: BlockId,
    access_ty: TypeId,
) -> Option<i64> {
    // The access function must stride over the innermost loop.
    let Kind::AddRec(_, header) = se.node(ar).kind else { return None };
    if lp != header {
        return None;
    }

    // Check the step is constant.
    let step = se.get_step_recurrence(cx, ar);

    // Calculate the pointer stride and check if it is constant.
    let step = se.match_apint(step)?;

    let size = cx.layout.alloc_size(&cx.context.types, access_ty) as i64;

    // Huge step value - give up.
    let step = try_sext_value(step.bits, step.width)?;

    // Strided access.
    (step % size == 0).then_some(step / size)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L3108
/// `LoopAccessInfo::blockNeedsPredication`: `block` of the loop `lp` does not
/// dominate its latch.
pub fn block_needs_predication(
    cx: &Cx,
    block: BlockId,
    lp: BlockId,
) -> bool {
    let found = cx.loops.get_loop_for(lp).expect("the header of a loop");
    let latch = found.get_loop_latch(cx.function).expect("Loop expected to have a single latch.");
    !cx.tree.dominates(block, latch)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1103
/// `isNoWrap`: whether the add recurrence `ar` of a pointer cannot wrap, from
/// its flags, from an `nusw` GEP that is dereferenced in every iteration, or
/// from a unit stride where null is not a valid pointer. `Predicates` is null
/// (D24): an overflow `isNoWrap` would have assumed is a wrap here.
pub fn is_no_wrap(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ar: Scev,
    ptr: Option<Operand>,
    access_ty: TypeId,
    lp: BlockId,
    stride: Option<i64>,
) -> bool {
    // FIXME: This should probably only return true for NUW.
    if se.get_no_wrap_flags(ar, crate::scev::expr::flags::MASK) != 0 {
        return true;
    }

    // An nusw getelementptr that is an AddRec cannot wrap. If it would wrap,
    // the distance between the previously accessed location and the wrapped
    // location will be larger than half the pointer index type space. In that
    // case, the GEP would be poison and any memory access dependent on it
    // would be immediate UB when executed.
    if let Some(Operand::Value(gep)) = ptr
        && let ValueDef::Instruction(gep_inst) = cx.function.value(gep).def
        && let instruction = cx.function.instruction(gep_inst)
        && matches!(instruction.opcode, Opcode::GetElementPtr { .. })
        && (instruction.flags.contains(IrFlags::NUSW) || instruction.flags.contains(IrFlags::INBOUNDS))
    {
        // For the above reasoning to apply, the pointer must be dereferenced in
        // every iteration.
        let found = cx.loops.get_loop_for(lp).expect("the header of a loop");
        let dereferenced = cx
            .function
            .users(gep)
            .into_iter()
            .any(
                |user| {
                    if load_store_pointer_operand(cx, user.user) != Some(Operand::Value(gep)) {
                        return false;
                    }
                    let Some(user_block) = cx.function.parent(user.user) else { return false };
                    if !found.contains(user_block) {
                        return false;
                    }
                    !block_needs_predication(cx, user_block, lp)
                },
            );
        if found.get_loop_latch(cx.function) == Some(lp) || dereferenced {
            return true;
        }
    }

    let stride = stride.or_else(|| get_stride_from_add_rec(se, cx, ar, lp, access_ty));
    if let Some(stride) = stride {
        // If the null pointer is undefined, then an access sequence which would
        // otherwise access it can be assumed not to unsigned wrap. Note that
        // this assumes the object in memory is aligned to the natural
        // alignment.
        if !null_pointer_is_defined(cx) && (stride == 1 || stride == -1) {
            return true;
        }
    }

    false
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1753
/// `getPtrStride`: the stride of the access through `ptr` over the loop `lp`
/// in elements of `access_ty`, 0 if `ptr` is loop-invariant; none where it is
/// not a constant stride, or may wrap (`should_check_wrap`).
pub fn get_ptr_stride(
    se: &mut ScalarEvolution,
    cx: &Cx,
    access_ty: TypeId,
    ptr: Operand,
    lp: BlockId,
    strides: &SymbolicStrideMap,
    should_check_wrap: bool,
) -> Option<i64> {
    let ptr_scev = replace_symbolic_stride_scev(se, cx, strides, ptr);
    if se.is_loop_invariant(cx, ptr_scev, Some(lp)) {
        return Some(0);
    }

    // Not an AddRecExpr pointer.
    if !matches!(se.node(ptr_scev).kind, Kind::AddRec(..)) {
        return None;
    }

    let stride = get_stride_from_add_rec(se, cx, ptr_scev, lp, access_ty)?;
    if !should_check_wrap {
        return Some(stride);
    }

    is_no_wrap(se, cx, ptr_scev, Some(ptr), access_ty, lp, Some(stride)).then_some(stride)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1159
/// `visitPointers`: `start`, or the pointers a phi inside the loop (but not
/// its header's) gathers, as SCEV does not look through them.
pub fn visit_pointers(
    cx: &Cx,
    start: Operand,
    lp: BlockId,
    mut add_pointer: impl FnMut(Operand),
) {
    let found = cx.loops.get_loop_for(lp).expect("the header of a loop");
    let mut visited = HashSet::default();
    let mut work_list = vec![start];

    while let Some(ptr) = work_list.pop() {
        if !visited.insert(ptr) {
            continue;
        }
        // SCEV does not look through non-header PHIs inside the loop. Such phis
        // can be analyzed by adding separate accesses for each incoming
        // pointer value.
        if let Operand::Value(value) = ptr
            && let ValueDef::Instruction(inst) = cx.function.value(value).def
            && let instruction = cx.function.instruction(inst)
            && matches!(instruction.opcode, Opcode::Phi)
            && let Some(block) = cx.function.parent(inst)
            && found.contains(block)
            && block != lp
        {
            work_list.extend(instruction.operands.chunks(2).map(|pair| pair[0]));
        } else {
            add_pointer(ptr);
        }
    }
}

/// A SCEV and whether the pointer behind it may be undef or poison
/// (`PointerIntPair<const SCEV *, 1, bool>`).
pub type ForkedScev = (Scev, bool);

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopAccessAnalysis.cpp#L1198
/// `findForkedSCEVs`: the SCEVs `ptr` stands for when a select or phi forks
/// it in two (a pointer, or an offset, or both of the sides of a sum), or its
/// own SCEV.
pub fn find_forked_scevs(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: BlockId,
    ptr: Operand,
    depth: u32,
) -> Vec<ForkedScev> {
    let may_be_undef_or_poison =
        |operand: Operand| !is_guaranteed_not_to_be_undef_or_poison(&cx.query(None), operand, 0);
    let mut scev_list = Vec::new();

    // If our Value is a SCEVAddRecExpr, loop invariant, not an instruction, or
    // we've exceeded our limit on recursion, just return whatever we have
    // regardless of whether it can be used for a forked pointer or not, along
    // with an indication of whether it might be a poison or undef value.
    let scev = se.get_scev(cx, ptr);
    let found = cx.loops.get_loop_for(lp).expect("the header of a loop");
    let instruction = match ptr {
        Operand::Value(value) => match cx.function.value(value).def {
            ValueDef::Instruction(inst) => Some(cx.function.instruction(inst)),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    };
    let Some(instruction) = instruction else {
        scev_list.push((scev, may_be_undef_or_poison(ptr)));
        return scev_list;
    };
    if matches!(se.node(scev).kind, Kind::AddRec(..)) || found.is_loop_invariant(cx.function, ptr) || depth == 0 {
        scev_list.push((scev, may_be_undef_or_poison(ptr)));
        return scev_list;
    }

    let depth = depth - 1;
    let needs_freeze = |forks: &[ForkedScev]| forks.iter().any(|fork| fork.1);

    match &instruction.opcode {
        Opcode::GetElementPtr { source } => {
            // We only handle base + single offset GEPs here for now. Not
            // dealing with preexisting gathers yet, so no vectors.
            if instruction.operands.len() != 2 || matches!(cx.context.types.get(*source), Type::Vector { .. }) {
                scev_list.push((scev, may_be_undef_or_poison(ptr)));
                return scev_list;
            }
            let mut base_scevs = find_forked_scevs(se, cx, lp, instruction.operands[0], depth);
            let mut offset_scevs = find_forked_scevs(se, cx, lp, instruction.operands[1], depth);

            // See if we need to freeze our fork...
            let freeze = needs_freeze(&base_scevs) || needs_freeze(&offset_scevs);

            // Check that we only have a single fork, on either the base or the
            // offset. Copy the SCEV across for the one without a fork in order
            // to generate the full SCEV for both sides of the GEP.
            if offset_scevs.len() == 2 && base_scevs.len() == 1 {
                base_scevs.push(base_scevs[0]);
            } else if base_scevs.len() == 2 && offset_scevs.len() == 1 {
                offset_scevs.push(offset_scevs[0]);
            } else {
                scev_list.push((scev, freeze));
                return scev_list;
            }

            let Some(ptr_ty) = cx.function.operand_type(cx.context, ptr) else { unreachable!("a pointer") };
            let Type::Pointer(space) = cx.context.types.get(ptr_ty) else { unreachable!("a pointer") };
            let int_ptr_ty = se.get_effective_scev_type(cx, Ty::Ptr(*space));

            // Find the size of the type being pointed to. We only have a
            // single index term (guarded above) so we don't need to index into
            // arrays or structures, just get the size of the scalar value.
            let size = se.get_size_of_expr(cx, int_ptr_ty, *source);

            for ((base, _), (offset, _)) in base_scevs.into_iter().zip(offset_scevs) {
                // Scale up the offsets by the size of the type, then add to
                // the bases.
                let extended = se.get_truncate_or_sign_extend(cx, offset, int_ptr_ty, 0);
                let scaled = se.get_mul_expr(cx, vec![Use::of(size), Use::of(extended)], Flags::default(), 0).scev;
                let sum = se.get_add_expr(cx, vec![Use::of(base), Use::of(scaled)], Flags::default(), 0).scev;
                scev_list.push((sum, freeze));
            }
        }
        Opcode::Select => {
            // A select means we've found a forked pointer, but we currently
            // only support a single select per pointer so if there's another
            // behind this then we just bail out and return the generic SCEV.
            let mut child_scevs = find_forked_scevs(se, cx, lp, instruction.operands[1], depth);
            child_scevs.extend(find_forked_scevs(se, cx, lp, instruction.operands[2], depth));
            if child_scevs.len() == 2 {
                scev_list.extend(child_scevs);
            } else {
                scev_list.push((scev, may_be_undef_or_poison(ptr)));
            }
        }
        Opcode::Phi => {
            // A phi means we've found a forked pointer, but we currently only
            // support a single phi per pointer so if there's another behind
            // this then we just bail out and return the generic SCEV.
            let mut child_scevs = Vec::new();
            if instruction.operands.len() == 4 {
                child_scevs.extend(find_forked_scevs(se, cx, lp, instruction.operands[0], depth));
                child_scevs.extend(find_forked_scevs(se, cx, lp, instruction.operands[2], depth));
            }
            if child_scevs.len() == 2 {
                scev_list.extend(child_scevs);
            } else {
                scev_list.push((scev, may_be_undef_or_poison(ptr)));
            }
        }
        Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)) => {
            let mut l_scevs = find_forked_scevs(se, cx, lp, instruction.operands[0], depth);
            let mut r_scevs = find_forked_scevs(se, cx, lp, instruction.operands[1], depth);

            // See if we need to freeze our fork...
            let freeze = needs_freeze(&l_scevs) || needs_freeze(&r_scevs);

            // Check that we only have a single fork, on either the left or
            // right side. Copy the SCEV across for the one without a fork in
            // order to generate the full SCEV for both sides of the BinOp.
            if l_scevs.len() == 2 && r_scevs.len() == 1 {
                r_scevs.push(r_scevs[0]);
            } else if r_scevs.len() == 2 && l_scevs.len() == 1 {
                l_scevs.push(l_scevs[0]);
            } else {
                scev_list.push((scev, freeze));
                return scev_list;
            }

            for ((left, _), (right, _)) in l_scevs.into_iter().zip(r_scevs) {
                let combined = match op {
                    BinaryOp::Add => se.get_add_expr(cx, vec![Use::of(left), Use::of(right)], Flags::default(), 0).scev,
                    _ => se.get_minus_scev(cx, Use::of(left), Use::of(right), 0, 0),
                };
                scev_list.push((combined, freeze));
            }
        }
        // Just return the current SCEV if we haven't handled the instruction
        // yet.
        _ => scev_list.push((scev, may_be_undef_or_poison(ptr))),
    }
    scev_list
}

#[cfg(test)]
mod tests {
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::module::InstId;
    use llrm_mir::opcode::Opcode;
    use llrm_mir::valuetracking::AssumptionCache;

    use super::*;

    /// `is_consecutive_access` of each pair of the stores of `@f`, by order.
    fn consecutive(
        stores: &str,
        pairs: &[(usize, usize)],
        check_type: bool,
    ) -> Vec<bool> {
        let text = format!("define void @f(ptr %p, ptr %q) {{\n{stores}  ret void\n}}\n");
        let module = llrm_mir::parse::module(&text).expect("a module");
        let (_, _, function) = module.functions().next().expect("a function");
        let layout = llrm_mir::datalayout::DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(&module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let stores: Vec<InstId> = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            .collect();
        let mut se = ScalarEvolution::new();
        pairs.iter().map(|&(a, b)| is_consecutive_access(&cx, &mut se, stores[a], stores[b], check_type)).collect()
    }

    /// Offsets off one base are compared in elements of the first access.
    #[test]
    fn test_consecutive_is_one_element_after_off_the_same_base() {
        let stores = "  %a = getelementptr i32, ptr %p, i64 3
  %b = getelementptr i32, ptr %p, i64 4
  %c = getelementptr i32, ptr %p, i64 6
  %d = getelementptr i8, ptr %p, i64 13
  store i32 0, ptr %a
  store i32 0, ptr %b
  store i32 0, ptr %c
  store i32 0, ptr %p
  store i32 0, ptr %d
  store i8 0, ptr %b
  store i32 0, ptr %q
";
        // a,b one apart; b,a one before; a,c three apart; p,p the same
        // pointer (0); a,d 4 bytes plus 1 is not a whole element (strict);
        // b with an i8 store differs in type; a,q another base, unknown.
        assert_eq!(
            consecutive(stores, &[(0, 1), (1, 0), (0, 2), (3, 3), (0, 4), (1, 5), (0, 6)], true),
            [true, false, false, false, false, false, false]
        );
        // Without the type check the i8 store at b is a whole i32 after a.
        assert_eq!(consecutive(stores, &[(0, 5)], false), [true]);
    }
}
