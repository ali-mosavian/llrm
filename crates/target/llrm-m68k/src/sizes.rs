//! Bytes of the code an IR operation lowers to, for `CostKind::CodeSize`.
//!
//! Each price is the length of the instruction forms the operation expands to
//! (MC68000UM chapter 8, effective-address extension words), with operands in
//! registers and a displacement or immediate counted as one extension word.
//! The 68000 has no 32-bit `muls.l`/`divs.l`: those are a library call
//! (`jsr` 6 B, two pushes 4 B, cleanup 2 B), the 68020 has them at 4 B.
//!
//! | operation | forms | bytes |
//! |---|---|---|
//! | add, sub, and, or, xor | `add.l %d1,%d0` | 2 |
//! | the same with a constant | `addq`/`addi` | 2 + 2 or 4 |
//! | shift, rotate | `lsl.l %d1,%d0`, `lsl.l #n,%d0` | 2 |
//! | multiply, 16 bit | `muls.w %d1,%d0` | 2 |
//! | multiply, 32 bit | 68020 `muls.l %d1,%d0` | 4 |
//! | divide, 16 bit | `ext.l %d0; divs.w %d1,%d0` | 4 |
//! | divide, 32 bit | 68020 `divs.l %d1,%d0` | 4 |
//! | compare | `cmp.l %d1,%d0` | 2 |
//! | select | `cmp; bcc.b; move.l` | 6 |
//! | load, store | `move.l (%a0),%d0`, `move.l %d0,(8,%a0)` | 4 |
//! | branch | `bcc.w`, `bra.b` | 3 |
//! | sign or zero extension | `ext.l`, `moveq #0` + `move.b` | 2 to 4 |
//!
//! A value wider than 32 bits is its 32-bit pieces and a carry.

use llrm_mir::tti::{ArithOp, CastKind, CmpSel, Cost, MemoryOp, OperandInfo, Ty};

/// The 32-bit pieces of a value of `ty`.
fn pieces(ty: Ty) -> i64 {
    let bits = match ty {
        Ty::Int(bits) => bits,
        Ty::Pointer | Ty::Float => 32,
        Ty::Double => 64,
        Ty::X87 => 96,
    };
    i64::from(bits.div_ceil(32).max(1))
}

/// A library call with `arguments` words pushed: `jsr abs.l`, the pushes, the
/// `addq`/`lea` that pops them.
fn libcall(arguments: i64) -> i64 {
    6 + 2 * arguments + 2
}

/// Bytes of an arithmetic operation, `cpu_has_long_muldiv` the 68020's
/// `muls.l`/`divs.l`.
pub fn arithmetic(
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    cpu_has_long_muldiv: bool,
) -> Cost {
    let parts = pieces(ty);
    let wide = parts > 1;
    let immediate = if right.constant { 2 } else { 0 };
    let bytes = match op {
        ArithOp::Add | ArithOp::Sub | ArithOp::And | ArithOp::Or | ArithOp::Xor => (2 + immediate) * parts,
        ArithOp::Shl | ArithOp::LShr | ArithOp::AShr => 2 * parts + if wide { 2 } else { 0 },
        ArithOp::Mul if wide => libcall(parts * 2),
        ArithOp::Mul if matches!(ty, Ty::Int(bits) if bits > 16) => {
            if cpu_has_long_muldiv { 4 } else { libcall(2) }
        }
        ArithOp::Mul => 2,
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem if wide => libcall(parts * 2),
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
            if matches!(ty, Ty::Int(bits) if bits > 16) =>
        {
            if cpu_has_long_muldiv { 4 } else { libcall(2) }
        }
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem => 4,
        // soft float: a call
        ArithOp::FAdd | ArithOp::FSub | ArithOp::FMul | ArithOp::FDiv | ArithOp::FRem => libcall(parts * 2),
        ArithOp::FNeg => 6,
    };
    Cost::of(bytes)
}

/// Bytes of a compare or select.
pub fn cmp_sel(
    op: CmpSel,
    ty: Ty,
) -> Cost {
    let parts = pieces(ty);
    Cost::of(match op {
        CmpSel::ICmp => 2 * parts,
        CmpSel::FCmp => libcall(parts * 2),
        CmpSel::Select => 4 + 2 * parts,
    })
}

/// Bytes of a load or store.
pub fn memory(
    _op: MemoryOp,
    ty: Ty,
    indexed: bool,
) -> Cost {
    Cost::of((4 + if indexed { 2 } else { 0 }) * pieces(ty))
}

/// Bytes of a branch, return or phi.
pub fn control_flow(phi: bool) -> Cost {
    Cost::of(if phi { 0 } else { 3 })
}

/// Bytes of a cast.
pub fn cast(
    op: CastKind,
    to: Ty,
    from: Ty,
) -> Cost {
    let (to_parts, from_parts) = (pieces(to), pieces(from));
    Cost::of(match op {
        CastKind::Trunc | CastKind::BitCast | CastKind::PtrToInt | CastKind::IntToPtr => 0,
        CastKind::SExt => {
            if to_parts > from_parts { 2 + 2 * (to_parts - from_parts) } else { 2 }
        }
        CastKind::ZExt => {
            if to_parts > from_parts { 2 * (to_parts - from_parts) + 2 } else { 4 }
        }
        _ => libcall(pieces(from)),
    })
}
