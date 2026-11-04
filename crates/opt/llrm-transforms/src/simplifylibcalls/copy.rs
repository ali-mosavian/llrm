//! The `strcpy` and `stpcpy` arms of `LibCallSimplifier` that copy a string of
//! known length with `llvm.memcpy`: `optimizeStrCpy` (:749) and
//! `optimizeStpCpy` (:770), at d1106deb71cc.

use llrm_mir::edit::Position;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::Unit;

use super::{Rewrite, Site, string_length};
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
