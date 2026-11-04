//! The `strcpy`, `stpcpy`, `strcat`, `strncat` and `strncpy` arms of
//! `LibCallSimplifier` that copy a string of known length with `llvm.memcpy`:
//! `optimizeStrCpy` (:749), `optimizeStpCpy` (:770), `optimizeStrCat` (:408),
//! `emitStrLenMemCpy` (:428), `optimizeStrNCat` (:449) and
//! `optimizeStringNCpy` (:879), at d1106deb71cc.

use llrm_mir::edit::Position;
use llrm_mir::libfunc::Routine;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::Unit;

use super::{NewCall, Rewrite, Site, build, constant_length, emit, string_length};
use crate::loopidiom::build::{Copy, create_mem_copy};

/// `B.CreateMemCpy(Dst, Align(1), Src, Align(1), Len)` before the call; `len`
/// bytes, the NUL included.
fn mem_copy(
    unit: &mut Unit,
    site: &Site,
    dest: Operand,
    source: Operand,
    len: u64,
) {
    let length = Operand::Constant(unit.context.int(site.size, i128::from(len)));
    create_mem_copy(
        unit.context,
        unit.declared,
        unit.function,
        site.call,
        Copy::Memcpy,
        dest,
        Some(1),
        source,
        Some(1),
        length,
    );
}

/// `optimizeStrCpy`: `strcpy(x, x)` is `x`, and a copy of a string of known
/// length is a `memcpy` of it with its NUL.
pub(super) fn str_cpy(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (dest, source) = (operands[0], operands[1]);
    if dest == source {
        return Some(Rewrite::Value(source));
    }
    let len = string_length(unit, site, source);
    if len == 0 {
        return None;
    }
    mem_copy(unit, site, dest, source, len);
    Some(Rewrite::Value(dest))
}

/// `optimizeStpCpy` for a source of known length: a `memcpy` of it, and the
/// address of the copied NUL. The `strcpy` of an unused result and
/// `stpcpy(x, x)`, which call a routine, are not translated (D6).
pub(super) fn stp_cpy(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (dest, source) = (operands[0], operands[1]);
    let result = unit.function.instruction(site.call).result?;
    if unit.function.users(result).is_empty() || dest == source {
        return None;
    }
    let len = string_length(unit, site, source);
    if len == 0 {
        return None;
    }
    let ty = unit.function.instruction(site.call).ty;
    let byte = unit.context.types.int(8);
    let end = Operand::Constant(unit.context.int(site.size, i128::from(len - 1)));
    let made = unit
        .function
        .create_instruction(Opcode::GetElementPtr { source: byte }, ty, vec![dest, end], Flags::INBOUNDS, None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed gep");
    mem_copy(unit, site, dest, source, len);
    Some(Rewrite::Value(Operand::Value(unit.function.instruction(made).result.expect("a result"))))
}

/// `emitStrLenMemCpy`: `memcpy(dest + strlen(dest), source, len + 1)` and
/// `dest`; none where the compile does not state `strlen`.
fn str_len_mem_copy(
    unit: &mut Unit,
    site: &Site,
    routines: &[Routine],
    source: Operand,
    dest: Operand,
    len: u64,
) -> Option<Rewrite> {
    let pointer = unit.context.types.ptr(0);
    let call = NewCall { standard: "strlen", args: vec![dest], returns: site.size, params: vec![pointer] };
    let measured = emit(unit, routines, call, site.call)?;
    let length = Operand::Value(unit.function.instruction(measured).result.expect("strlen returns a length"));
    let byte = unit.context.types.int(8);
    let made = unit
        .function
        .create_instruction(Opcode::GetElementPtr { source: byte }, pointer, vec![dest, length], Flags::INBOUNDS, None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed gep");
    let end = Operand::Value(unit.function.instruction(made).result.expect("a result"));
    mem_copy(unit, site, end, source, len + 1);
    Some(Rewrite::Value(dest))
}

/// `optimizeStrCat`: `strcat(x, "")` is `x`; otherwise `strlen` finds the end.
pub(super) fn str_cat(
    unit: &mut Unit,
    site: &Site,
    routines: &[Routine],
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (dest, source) = (operands[0], operands[1]);
    let len = string_length(unit, site, source).checked_sub(1)?;
    if len == 0 {
        return Some(Rewrite::Value(dest));
    }
    str_len_mem_copy(unit, site, routines, source, dest, len)
}

/// `optimizeStrNCat`: a constant bound at least the source's length is
/// `strcat`.
pub(super) fn str_n_cat(
    unit: &mut Unit,
    site: &Site,
    routines: &[Routine],
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (dest, source, size) = (operands[0], operands[1], operands[2]);
    let bound = constant_length(unit, size)?;
    if bound == 0 {
        return Some(Rewrite::Value(dest));
    }
    let len = string_length(unit, site, source).checked_sub(1)?;
    if len == 0 {
        return Some(Rewrite::Value(dest));
    }
    // A bound short of the source is not optimized.
    if bound < len {
        return None;
    }
    str_len_mem_copy(unit, site, routines, source, dest, len)
}

/// `optimizeStringNCpy` (`end` for `stpncpy`), for the bounds it copies with
/// one `memcpy`: a bound of 0 or 1, or one no longer than the source and its
/// NUL. The `memset` of an empty source and the NUL-padded copy are not
/// translated (D7).
pub(super) fn str_n_cpy(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (dest, source, size) = (operands[0], operands[1], operands[2]);
    let bound = constant_length(unit, size);
    if bound == Some(0) {
        return Some(Rewrite::Value(dest));
    }
    if bound == Some(1) {
        // strncpy(D, S, 1) is (*D = *S), D.
        let byte = unit.context.types.int(8);
        let first = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![source]);
        let void = unit.context.types.void();
        let store = unit
            .function
            .create_instruction(
                Opcode::Store { align: Some(1), volatile: false },
                void,
                vec![first, dest],
                Flags::default(),
                None,
            );
        unit.function.insert(store, Position::Before(site.call)).expect("a placed store");
        return Some(Rewrite::Value(dest));
    }
    let bound = bound?;
    let len = string_length(unit, site, source).checked_sub(1)?;
    if len == 0 || bound > len + 1 {
        return None;
    }
    mem_copy(unit, site, dest, source, bound);
    Some(Rewrite::Value(dest))
}
