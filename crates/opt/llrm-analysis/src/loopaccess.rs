//! `getPointersDiff` and `isConsecutiveAccess`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`: the
//! distance between two pointers in units of an element, from their constant
//! offsets off a common base, else from ScalarEvolution's constant
//! difference of the two.
//!
//! # Example
//! `getelementptr i32, ptr %p, i64 1` is consecutive after `%p`.

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;

use crate::scev::{Cx, ScalarEvolution};

fn address_space(
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
fn load_store_pointer_operand(
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
fn load_store_type(
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
