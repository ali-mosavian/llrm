//! The `strchr`, `strpbrk`, `strspn`, `strcspn` and `strstr` arms of
//! `LibCallSimplifier` that fold to a value without a new call:
//! `optimizeStrChr` (:521), `optimizeStrPBrk` (:1119), `optimizeStrSpn`
//! (:1158), `optimizeStrCSpn` (:1179) and `optimizeStrStr` (:1203), at
//! d1106deb71cc.

use llrm_mir::constfold::null_value;
use llrm_mir::context::ConstantKind;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::Unit;

use super::{Rewrite, Site, string_info};

/// `Constant::getNullValue` of the call's pointer type.
fn null(
    unit: &Unit,
    site: &Site,
) -> Rewrite {
    let ty = unit.function.instruction(site.call).ty;
    Rewrite::Value(Operand::Constant(null_value(unit.context, ty)))
}

/// `B.CreateInBoundsGEP(i8, base, offset)` before the call.
fn offset(
    unit: &mut Unit,
    site: &Site,
    base: Operand,
    at: u64,
) -> Rewrite {
    let ty = unit.function.instruction(site.call).ty;
    let byte = unit.context.types.int(8);
    let wide = unit.context.types.int(64);
    let index = Operand::Constant(unit.context.int(wide, i128::from(at)));
    let made = unit
        .function
        .create_instruction(Opcode::GetElementPtr { source: byte }, ty, vec![base, index], Flags::INBOUNDS, None);
    unit.function.insert(made, llrm_mir::edit::Position::Before(site.call)).expect("a placed gep");
    Rewrite::Value(Operand::Value(unit.function.instruction(made).result.expect("a result")))
}

/// `isOnlyUsedInEqualityComparison(CI, With)`: every user, vacuously none, is
/// an equality `icmp` whose second operand is `with`.
fn only_equality_compared_with(
    unit: &Unit,
    site: &Site,
    with: Operand,
) -> bool {
    let Some(result) = unit.function.instruction(site.call).result else { return true };
    unit.function
        .users(result)
        .iter()
        .all(
            |used| {
                let user = unit.function.instruction(used.user);
                matches!(user.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)) && user.operands[1] == with
            },
        )
}

/// `optimizeStrChr`, for a constant string and a constant character; the
/// `memchr`, `strlen` and character-compare rewrites are not translated yet.
pub(super) fn str_chr(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (string, character) = (operands[0], operands[1]);
    if only_equality_compared_with(unit, site, string) {
        return None;
    }
    let Operand::Constant(id) = character else { return None };
    let ConstantKind::Int(bits) = unit.context.get(id).kind else { return None };
    let text = string_info(unit, site, string, true)?;
    // Compute the offset, handling a search for zero (a weird way to spell
    // strlen).
    let wanted = (bits & 0xFF) as u8;
    let found = if wanted == 0 { Some(text.len()) } else { text.iter().position(|&byte| u128::from(byte) == bits) };
    match found {
        None => Some(null(unit, site)),
        Some(at) => Some(offset(unit, site, string, at as u64)),
    }
}

/// `optimizeStrPBrk`; the `strchr` rewrite of a one-character set is not
/// translated yet.
pub(super) fn str_pbrk(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let first = string_info(unit, site, operands[0], true);
    let second = string_info(unit, site, operands[1], true);
    if first.as_ref().is_some_and(Vec::is_empty) || second.as_ref().is_some_and(Vec::is_empty) {
        return Some(null(unit, site));
    }
    let (first, second) = (first?, second?);
    match first.iter().position(|byte| second.contains(byte)) {
        None => Some(null(unit, site)),
        Some(at) => Some(offset(unit, site, operands[0], at as u64)),
    }
}

/// `optimizeStrSpn` (`span`) and `optimizeStrCSpn`; the `strlen` rewrite of
/// `strcspn(s, "")` is not translated yet.
pub(super) fn str_spn(
    unit: &mut Unit,
    site: &Site,
    span: bool,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let first = string_info(unit, site, operands[0], true);
    let second = string_info(unit, site, operands[1], true);
    let ty = unit.function.instruction(site.call).ty;
    let zero = Rewrite::Value(Operand::Constant(unit.context.int(ty, 0)));
    let empty = |text: &Option<Vec<u8>>| text.as_ref().is_some_and(Vec::is_empty);
    // strspn(s, "") and strspn("", s) are 0; strcspn("", s) is 0.
    if empty(&first) || (span && empty(&second)) {
        return Some(zero);
    }
    let (first, second) = (first?, second?);
    let at = if span {
        first.iter().position(|byte| !second.contains(byte))
    } else {
        first.iter().position(|byte| second.contains(byte))
    };
    Some(Rewrite::Value(Operand::Constant(unit.context.int(ty, at.unwrap_or(first.len()) as i128))))
}

/// `optimizeStrStr`; the `strncmp` and `strchr` rewrites are not translated
/// yet.
pub(super) fn str_str(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (haystack, needle) = (operands[0], operands[1]);
    if haystack == needle {
        return Some(Rewrite::Value(haystack));
    }
    if only_equality_compared_with(unit, site, haystack) {
        return None;
    }
    let find = string_info(unit, site, needle, true);
    if find.as_ref().is_some_and(Vec::is_empty) {
        return Some(Rewrite::Value(haystack));
    }
    let (search, find) = (string_info(unit, site, haystack, true)?, find?);
    match search.windows(find.len()).position(|window| window == find.as_slice()) {
        None => Some(null(unit, site)),
        Some(at) => Some(offset(unit, site, haystack, at as u64)),
    }
}
