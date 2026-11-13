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
//! | the same with a constant | `addq` 1 to 8; `addi.w`, `addi.l` | 2; 2 + 2 or 4 |
//! | shift, rotate | `lsl.l %d1,%d0`, `lsl.l #n,%d0` | 2 |
//! | multiply, 16 bit | `muls.w %d1,%d0` | 2 |
//! | multiply, 32 bit | 68020 `muls.l %d1,%d0`; with a constant | 4; 8 |
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

/// The extension bytes of a constant `right` operand of an `add`-class
/// operation, per 32-bit piece: none for the `addq`/`subq` range 1 to 8, two
/// for an operation of at most 16 bits, four for a long one.
fn immediate(
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    parts: i64,
) -> i64 {
    if !right.constant {
        return 0;
    }
    if parts == 1
        && matches!(op, ArithOp::Add | ArithOp::Sub)
        && right.value.is_some_and(|value| (1..=8).contains(&value.unsigned_abs()))
    {
        return 0;
    }
    match ty {
        Ty::Int(bits) if bits <= 16 => 2,
        _ => 4,
    }
}

/// Bytes of an arithmetic operation, `cpu_has_long_muldiv` the 68020's
/// `muls.l`/`divs.l`.
pub fn arithmetic(
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    cpu_has_long_muldiv: bool,
    speed: bool,
) -> Cost {
    let parts = pieces(ty);
    let wide = parts > 1;
    if !wide && let Some(bytes) = constant_division(op, ty, right, cpu_has_long_muldiv, speed) {
        return Cost::of(bytes);
    }
    let bytes = match op {
        ArithOp::Add | ArithOp::Sub | ArithOp::And | ArithOp::Or | ArithOp::Xor => {
            (2 + immediate(op, ty, right, parts)) * parts
        }
        ArithOp::Shl | ArithOp::LShr | ArithOp::AShr => 2 * parts + if wide { 2 } else { 0 },
        ArithOp::Mul if wide => libcall(parts * 2),
        ArithOp::Mul if matches!(ty, Ty::Int(bits) if bits > 16) => {
            // `muls.l #imm,%dn` is the opcode, the extension word and a
            // 32-bit immediate; a power of two is a shift
            match (cpu_has_long_muldiv, right.constant) {
                (false, _) => libcall(2),
                (true, false) => 4,
                (true, true) if right.power_of_two => 2,
                (true, true) => 8,
            }
        }
        ArithOp::Mul => 2,
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem if wide => libcall(parts * 2),
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem if matches!(ty, Ty::Int(bits) if bits > 16) => {
            if cpu_has_long_muldiv {
                4
            } else {
                libcall(2)
            }
        }
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem => 4,
        // soft float: a call
        ArithOp::FAdd | ArithOp::FSub | ArithOp::FMul | ArithOp::FDiv | ArithOp::FRem => libcall(parts * 2),
        ArithOp::FNeg => 6,
    };
    Cost::of(bytes)
}

/// Bytes of a division or remainder by a constant, as the DAG lowers it, or
/// None where the plain divide stands. Measured on `a[1] = a[0] op K` at -O2
/// (the framing load and store, 2 B beyond the base copy, taken off):
///
/// | divisor | width | udiv | urem | sdiv | srem |
/// |---|---|---|---|---|---|
/// | power of two | 8 | 2 | 4 | 10 | 14 |
/// | power of two | 16, 32 | 2 | 4 | 18 | 24 |
/// | other | 8, 16 | 10 | 22 | 22 | 36 |
/// | other, `muls.l` | 32 | 12 | 28 | 24 | 38 |
///
/// A quotient by a non-power of two is the magic multiply (`muls.w #imm`, or
/// `muls.l` on the 68020), a 32-bit one on the 68000 the library call with the
/// constant pushed (4 B more than two registers). Only a speed caller (-O2,
/// -O3) is priced so: the -O1 and -Os bounds were fitted to the plain
/// divide's price, and -Os keeps `divs.l #imm`.
fn constant_division(
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    cpu_has_long_muldiv: bool,
    speed: bool,
) -> Option<i64> {
    let (signed, rem) = match op {
        ArithOp::UDiv => (false, false),
        ArithOp::URem => (false, true),
        ArithOp::SDiv => (true, false),
        ArithOp::SRem => (true, true),
        _ => return None,
    };
    let Ty::Int(bits) = ty else { return None };
    if !speed || !right.constant || right.value.is_some_and(|value| value.unsigned_abs() <= 1) {
        return None;
    }
    if right.power_of_two {
        return Some(match (signed, rem, bits <= 8) {
            (false, false, _) => 2,
            (false, true, _) => 4,
            (true, false, true) => 10,
            (true, true, true) => 14,
            (true, false, false) => 18,
            (true, true, false) => 24,
        });
    }
    if bits > 16 && !cpu_has_long_muldiv {
        return Some(libcall(2) + 4);
    }
    let long = bits > 16;
    Some(match (signed, rem) {
        (false, false) => 10 + if long { 2 } else { 0 },
        (false, true) => 22 + if long { 6 } else { 0 },
        (true, false) => 22 + if long { 2 } else { 0 },
        (true, true) => 36 + if long { 2 } else { 0 },
    })
}

/// The clocks of a division or remainder by a constant that is not a power of
/// two, as the DAG lowers it (the forms of `constant_division`'s table): the
/// magic multiply and what folds its sign and rebuilds the remainder. None
/// where the plain divide stands (a 32-bit divide without `muls.l`, an
/// operand that is not constant) or the tables cannot price a form.
pub fn constant_division_clocks(
    cpu: Cpu,
    op: ArithOp,
    ty: Ty,
    right: OperandInfo,
    cpu_has_long_muldiv: bool,
) -> Option<i64> {
    let (signed, rem) = match op {
        ArithOp::UDiv => (false, false),
        ArithOp::URem => (false, true),
        ArithOp::SDiv => (true, false),
        ArithOp::SRem => (true, true),
        _ => return None,
    };
    let Ty::Int(bits) = ty else { return None };
    if !right.constant || right.power_of_two || right.value.is_some_and(|value| value.unsigned_abs() <= 1) {
        return None;
    }
    let long = bits > 16;
    if long && !cpu_has_long_muldiv {
        return None;
    }
    let width = if long { "l" } else { "w" };
    let multiply = if signed { "muls" } else { "mulu" };
    // (form, bytes): the instructions of the lowering, a representative
    // divisor's.
    let mut forms: Vec<(String, u32)> = if long {
        vec![("move.l #1717986919, %d0".into(), 6), (format!("{multiply}.l %d0, %d1:%d2"), 4)]
    } else {
        vec![(format!("{multiply}.w #171, %d1"), 4), ("moveq #10, %d0".into(), 2)]
    };
    forms.push((format!("lsr.{width} %d0, %d1"), 2));
    if signed {
        // the quotient's sign shifted out and added back
        forms.extend([
            (format!("asr.{width} #1, %d1"), 2),
            ("move.l %d1, %d2".into(), 2),
            (format!("lsr.{width} #7, %d2"), 2),
            (format!("add.{width} %d2, %d1"), 2),
        ]);
    }
    if rem {
        // the quotient times the divisor by shifts and adds, subtracted
        forms.extend(
            [
                "move.l %d1, %d2",
                "lsl.l #2, %d2",
                "add.l %d1, %d2",
                "add.l %d2, %d2",
                "add.l %d1, %d2",
                "sub.l %d2, %d3",
            ]
            .map(|text| (text.to_string(), 2)),
        );
    }
    forms
        .iter()
        .map(|(text, bytes)| crate::timing::cost(cpu, text, *bytes, true).ok().map(|cost| i64::from(cost.clocks)))
        .sum()
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
            if to_parts > from_parts {
                2 + 2 * (to_parts - from_parts)
            } else {
                2
            }
        }
        CastKind::ZExt => {
            if to_parts > from_parts {
                2 * (to_parts - from_parts) + 2
            } else {
                4
            }
        }
        _ => libcall(pieces(from)),
    })
}

// ---- the call sequence and the frame ------------------------------------

use llrm_mir::tti::{CallShape, CostKind};

use crate::dagisel::hooks::Cpu;

/// `form` (GNU syntax, `bytes` long) on `cpu`, as the unit of `kind`.
fn form(
    cpu: Cpu,
    text: &str,
    bytes: i64,
    kind: CostKind,
) -> i64 {
    match kind {
        CostKind::CodeSize => bytes,
        _ => i64::from(
            crate::timing::cost(cpu, text, bytes as u32, true)
                .unwrap_or_else(|why| panic!("the m68k timing tables: {why}"))
                .clocks,
        ),
    }
}

/// The call, its arguments and the return it ends with: `jsr`, one
/// `move.l (n,%sp),-(%sp)` per stack word (a by-value aggregate is its words),
/// the `addq`/`lea` that pops them, and the callee's `rts`.
pub fn call_sequence(
    cpu: Cpu,
    shape: &CallShape,
    kind: CostKind,
) -> Cost {
    let words: i64 = shape
        .arguments
        .iter()
        .map(|argument| match argument.byval_bytes {
            Some(bytes) => ((bytes as i64 + 3) / 4).max(1),
            None => argument.ty.map_or(1, pieces),
        })
        .sum();
    let mut total = form(cpu, "jsr 0x1000:l", 6, kind) + form(cpu, "rts", 2, kind);
    total += words * form(cpu, "move.l (4,%sp), -(%sp)", 4, kind);
    if words > 0 {
        total += if words <= 2 {
            form(cpu, &format!("addq.l #{}, %sp", words * 4), 2, kind)
        } else {
            form(cpu, &format!("lea ({},%sp), %sp", words * 4), 4, kind)
        };
    }
    Cost::of(total)
}

/// The frame a callee with static allocas builds and tears down:
/// `link.w %a6,#-n` and `unlk %a6`. None: a leaf with no locals has none.
///
/// DEVIATE: the registers a callee saves are not in the call's shape; a
/// `movem` pair is not counted. Reason: `CallShape` carries no register
/// pressure. Exit 2026-11-20.
pub fn callee_frame(
    cpu: Cpu,
    shape: &CallShape,
    kind: CostKind,
) -> Option<Cost> {
    (shape.callee_frame_bytes > 0)
        .then(|| Cost::of(form(cpu, "link.w %a6, #-8", 4, kind) + form(cpu, "unlk %a6", 2, kind)))
}
