//! Bytes of 32-bit x86 code an IR operation lowers to, for `CostKind::CodeSize`
//! in the view the inliner counts a callee in (`Tti::inline_size_view`).
//!
//! Each price is the length of the instruction the operation expands to (Intel
//! SDM vol. 2, instruction format): opcode, ModRM, SIB, displacement and
//! immediate, with operands in registers. The memory operand of a load or store
//! is priced by [`address`] on top of the access, an immediate by its width.
//!
//! | operation | form | bytes |
//! |---|---|---|
//! | add, sub, and, or, xor | `add r,r`; `add r,imm8`; `add r,imm32` | 2; 3; 6 |
//! | shift by a constant | `shl r,1`; `shl r,imm8` | 2; 3 |
//! | shift by a variable | `mov ecx,r; shl r,cl` | 4 |
//! | multiply | `imul r,r`; `imul r,r,imm8`; `imul r,r,imm32` | 3; 3; 6 |
//! | divide, 32 bit | `cdq; idiv r` and the move of the divisor | 5 |
//! | compare | `cmp r,r`; `cmp r,imm` | 3 |
//! | select | `jcc; mov` (`cmov` where the CPU has it) | 4; 3 |
//! | load, store | `mov r,[m]`; `mov [m],r` | 2 |
//! | store of a constant | `mov [m],imm32` | 2 + 4 |
//! | sign or zero extension | `movsx`, `movzx` | 3 |
//! | phi, branch | `mov r,r`; `jcc rel8` | 2 |
//! | `memcpy` of n bytes | a `mov r,[m]; mov [m],r` pair per dword (6), `rep movsl` form (20) from 4 pairs | |
//! | direct call | `call rel32`, a push per argument, `add esp,n` | 5 + 2 each + 3 |
//!
//! A 16-bit operation takes the 66h prefix, a value of 64 bits is its pieces
//! and a carry. The query sees an operation's operands only as far as
//! `OperandInfo` says, so a variable-length copy and a constant one are priced
//! alike.

use llrm_mir::tti::{AddressBase, AddressForm, ArithOp, CastKind, CmpSel, Cost, MemoryOp, OperandInfo, Ty};

/// The pieces of a value of `ty` in the integer registers (32 bits wide, 64 in long mode: `quad`), and the
/// prefix bytes it takes: the 66h of a 16-bit one, the REX.W of a 64-bit one in long mode, the F2/F3 and 0Fh of
/// an SSE scalar.
fn pieces(
    ty: Ty,
    quad: bool,
) -> (i64, i64) {
    match ty {
        Ty::Int(bits) if quad => (i64::from(bits.div_ceil(64).max(1)), i64::from(bits == 16 || bits == 64)),
        Ty::Int(bits) => (i64::from(bits.div_ceil(32).max(1)), i64::from(bits == 16)),
        Ty::Pointer if quad => (1, 1),
        Ty::Float | Ty::Double if quad => (1, 2),
        Ty::Pointer | Ty::Float => (1, 0),
        Ty::Double => (2, 0),
        Ty::X87 => (3, 0),
    }
}

/// The immediate of an operation with a constant right operand: a byte where
/// the value fits a sign-extended byte, the operand's width otherwise.
fn immediate_bytes(
    right: OperandInfo,
    ty: Ty,
) -> i64 {
    let fits_byte = right.value.is_some_and(|value| (-128..=127).contains(&value));
    match ty {
        Ty::Int(bits) if bits <= 8 => 1,
        _ if fits_byte => 1,
        Ty::Int(16) => 2,
        _ => 4,
    }
}

/// Bytes of an arithmetic operation.
pub fn arithmetic(
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    quad: bool,
) -> Cost {
    use ArithOp::*;
    if ty.is_float() || ty == Ty::X87 {
        return Cost::of(if op == FRem { 25 } else if quad && ty != Ty::X87 { 4 } else { 2 });
    }
    let (parts, prefix) = pieces(ty, quad);
    let power = right.power_of_two || right.negated_power_of_two;
    Cost::of(match op {
        Add | Sub | And | Or | Xor => {
            parts * if right.constant { 2 + immediate_bytes(right, ty) } else { 2 } + prefix
        }
        Shl | LShr | AShr if right.constant => {
            parts * if right.value == Some(1) { 2 } else { 3 } + if parts > 1 { 2 } else { 0 } + prefix
        }
        Shl | LShr | AShr => 4 * parts + if parts > 1 { 10 } else { 0 } + prefix,
        Mul if power => 3 + parts,
        Mul if parts > 1 => 15,
        Mul if right.constant => 2 + immediate_bytes(right, ty) + prefix,
        Mul => 3 + prefix,
        SDiv | UDiv | SRem | URem if parts > 1 => 22,
        SDiv | UDiv | SRem | URem if right.constant => if power { 8 } else { 16 },
        SDiv | UDiv | SRem | URem => 5 + prefix + if matches!(op, SRem | URem) { 2 } else { 0 },
        _ => 2,
    })
}

/// Bytes of a cast.
pub fn cast(
    op: CastKind,
    to: Ty,
    from: Ty,
) -> Cost {
    use CastKind::*;
    let float = |ty: Ty| ty.is_float() || ty == Ty::X87;
    Cost::of(match op {
        Trunc | PtrToInt | IntToPtr | FPTrunc | FPExt => 0,
        BitCast if float(to) == float(from) => 0,
        BitCast => 8,
        ZExt if (to, from) == (Ty::Int(64), Ty::Int(32)) => 2,
        SExt if (to, from) == (Ty::Int(64), Ty::Int(32)) => 3,
        ZExt | SExt if from == Ty::Int(1) => 5,
        ZExt | SExt => 3,
        SIToFP => 7,
        UIToFP => 14,
        FPToSI | FPToUI => 20,
    })
}

/// Bytes of a compare or a select; `cmov` where the CPU has it.
pub fn cmp_sel(
    op: CmpSel,
    ty: Ty,
    cmov: bool,
    quad: bool,
) -> Cost {
    let (parts, prefix) = pieces(ty, quad);
    Cost::of(match op {
        CmpSel::ICmp => 3 * parts + prefix,
        CmpSel::FCmp => 5,
        CmpSel::Select => (if cmov { 3 } else { 4 }) * parts + prefix,
    })
}

/// Bytes of the access itself; its address is [`address`].
pub fn memory(
    op: MemoryOp,
    ty: Ty,
    stored_constant: bool,
    quad: bool,
) -> Cost {
    let (parts, prefix) = pieces(ty, quad);
    let immediate = match (op, stored_constant, ty) {
        (MemoryOp::Store, true, Ty::Int(bits)) => i64::from(bits.div_ceil(8).min(4)) * parts,
        (MemoryOp::Store, true, _) => 2,
        _ => 0,
    };
    Cost::of(2 * parts + prefix + immediate)
}

/// Bytes a phi or a branch takes: a register copy (`mov r,r`), and a short jump.
pub fn control_flow(_phi: bool) -> Cost {
    Cost::of(2)
}

/// Bytes the address adds to an access: a global's absolute address, a frame
/// slot's SIB and displacement, a register's displacement, a scaled index.
pub fn address(form: AddressForm) -> Cost {
    let index = i64::from(form.indexed);
    let displacement = i64::from(form.displacement_bytes);
    Cost::of(match form.base {
        AddressBase::Global => 4 + index,
        AddressBase::Frame => 1 + displacement.max(1),
        AddressBase::Register => displacement + index,
    })
}

/// Bytes of a direct call of `arguments` arguments: the call, a push per
/// argument and the pop of them. In long mode the first six go in registers (a
/// `mov r64,r64`, 3 bytes) and nothing is popped.
pub fn call(
    arguments: i64,
    quad: bool,
) -> Cost {
    if quad {
        return Cost::of(5 + 3 * arguments.min(6) + 4 * (arguments - 6).max(0));
    }
    Cost::of(5 + 2 * arguments + if arguments > 0 { 3 } else { 0 })
}

/// Bytes of a `memcpy`, `memmove` or `memset` of `length` bytes: a load and a
/// store (or a store of the fill) per dword, the `rep movsl` / `rep stosl`
/// form once that is shorter; without a constant length that form with its
/// byte tail.
pub fn memory_intrinsic(
    family: &str,
    length: Option<u64>,
) -> Cost {
    let fill = family == "memset";
    let (rep, tail, piece) = if fill { (14, 28, 7) } else { (20, 38, 6) };
    let Some(length) = length else { return Cost::of(tail) };
    let pieces = (length / 4 + (length % 4).div_ceil(2)) as i64;
    Cost::of((pieces * piece).min(rep))
}

/// Bytes of the code a call to the intrinsic `name` is, where this knows it;
/// none for the rest.
pub fn intrinsic(name: &str) -> Option<Cost> {
    let family = name.strip_prefix("llvm.")?.split('.').next()?;
    Some(Cost::of(match family {
        "umin" | "umax" | "smin" | "smax" | "abs" => 6,
        "bswap" | "fabs" | "sqrt" => 2,
        "ctpop" | "ctlz" | "cttz" => 8,
        "fshl" | "fshr" => 4,
        "sadd" | "uadd" | "ssub" | "usub" => 5,
        "smul" | "umul" => 8,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dword_add_is_two_bytes_with_a_byte_immediate_three_and_with_a_word_one_six() {
        let add = |right| arithmetic(ArithOp::Add, Ty::Int(32), right, false);
        assert_eq!(add(OperandInfo::ANY), Cost::of(2));
        assert_eq!(add(OperandInfo::constant(5, 32)), Cost::of(3));
        assert_eq!(add(OperandInfo::constant(100_000, 32)), Cost::of(6));
    }

    #[test]
    fn an_absolute_global_address_is_four_bytes_a_register_none() {
        let form = |base| AddressForm { base, displacement_bytes: 0, indexed: false };
        assert_eq!(address(form(AddressBase::Global)), Cost::of(4));
        assert_eq!(address(form(AddressBase::Register)), Cost::of(0));
    }

    #[test]
    fn a_small_constant_copy_is_a_pair_of_moves_per_dword_and_a_long_one_the_rep_form() {
        assert_eq!(memory_intrinsic("memcpy", Some(8)), Cost::of(12));
        assert_eq!(memory_intrinsic("memcpy", Some(64)), Cost::of(20));
        assert_eq!(memory_intrinsic("memcpy", None), Cost::of(38));
    }

    #[test]
    fn a_long_mode_quad_add_takes_the_rex_prefix_and_is_one_piece_where_a_dword_machine_has_two() {
        let add = |quad| arithmetic(ArithOp::Add, Ty::Int(64), OperandInfo::ANY, quad);
        assert_eq!((add(false), add(true)), (Cost::of(4), Cost::of(3)));
        assert_eq!((call(3, true), call(8, true)), (Cost::of(14), Cost::of(31)));
    }

    #[test]
    fn a_direct_call_is_five_bytes_and_each_pushed_argument_two_with_the_pop() {
        assert_eq!((call(0, false), call(2, false)), (Cost::of(5), Cost::of(12)));
    }
}
