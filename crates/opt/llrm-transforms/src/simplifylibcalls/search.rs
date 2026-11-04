//! The `strchr`, `strpbrk`, `strspn`, `strcspn` and `strstr` arms of
//! `LibCallSimplifier` that fold to a value without a new call:
//! `optimizeStrChr` (:521), `optimizeStrPBrk` (:1119), `optimizeStrSpn`
//! (:1158), `optimizeStrCSpn` (:1179), `optimizeStrStr` (:1203) and
//! `optimizeMemChr` (:1345) up to its bit-field rewrite, at d1106deb71cc.

use llrm_mir::constfold::null_value;
use llrm_mir::context::ConstantKind;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Unit;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::nonzero::is_known_non_zero;

use super::{Rewrite, Site, build, string_info};

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

/// The call's pointer null, as an operand.
fn null_operand(
    unit: &Unit,
    site: &Site,
) -> Operand {
    let ty = unit.function.instruction(site.call).ty;
    Operand::Constant(null_value(unit.context, ty))
}

/// `B.CreateInBoundsGEP(i8, base, index)` as an operand.
fn gep(
    unit: &mut Unit,
    site: &Site,
    base: Operand,
    index: Operand,
) -> Operand {
    let ty = unit.function.instruction(site.call).ty;
    let byte = unit.context.types.int(8);
    let made = unit
        .function
        .create_instruction(Opcode::GetElementPtr { source: byte }, ty, vec![base, index], Flags::INBOUNDS, None);
    unit.function.insert(made, llrm_mir::edit::Position::Before(site.call)).expect("a placed gep");
    Operand::Value(unit.function.instruction(made).result.expect("a result"))
}

/// `B.CreateTrunc(character, i8)`: the sought character's low byte.
fn low_byte(
    unit: &mut Unit,
    site: &Site,
    character: Operand,
) -> Operand {
    let byte = unit.context.types.int(8);
    build(unit, site, Opcode::Cast(CastOp::Trunc), byte, vec![character])
}

/// `memChrToCharCompare`: `memchr(A, C, N) == A` as `N && *A == C`, picking
/// the string or null (`size` is none for `strchr`).
fn char_compare(
    unit: &mut Unit,
    site: &Site,
    size: Option<Operand>,
) -> Rewrite {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (source, character) = (operands[0], operands[1]);
    let (byte, flag) = (unit.context.types.int(8), unit.context.types.int(1));
    let first = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![source]);
    let wanted = low_byte(unit, site, character);
    let mut found = build(unit, site, Opcode::ICmp(IntPredicate::Eq), flag, vec![first, wanted]);
    if let Some(size) = size {
        let ty = unit.function.operand_type(unit.context, size).expect("a typed size");
        let zero = Operand::Constant(unit.context.int(ty, 0));
        let nonzero = build(unit, site, Opcode::ICmp(IntPredicate::Ne), flag, vec![size, zero]);
        // CreateLogicalAnd: a select on the left, false otherwise.
        let no = Operand::Constant(unit.context.int(flag, 0));
        found = build(unit, site, Opcode::Select, flag, vec![nonzero, found, no]);
    }
    let result = unit.function.instruction(site.call).ty;
    let none = null_operand(unit, site);
    Rewrite::Value(build(unit, site, Opcode::Select, result, vec![found, source, none]))
}

/// `optimizeMemChr`; the bit-field rewrite of a constant array and size
/// tested against null is not translated yet (D5).
pub(super) fn mem_chr(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let (source, character, size) = (operands[0], operands[1], operands[2]);
    let query = Query::bare(unit.context, unit.function, unit.layout);
    if is_known_non_zero(&query, size, 0) && only_equality_compared_with(unit, site, source) {
        return Some(char_compare(unit, site, Some(size)));
    }
    let wanted = match character {
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    let length = match size {
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits as u64),
            _ => None,
        },
        _ => None,
    };
    let (flag, byte) = (unit.context.types.int(1), unit.context.types.int(8));
    let none = null_operand(unit, site);
    match length {
        Some(0) => return Some(Rewrite::Value(none)),
        Some(1) => {
            // memchr(x, y, 1) --> *x == y ? x : null for any x and y.
            let first = build(unit, site, Opcode::Load { align: Some(1), volatile: false }, byte, vec![source]);
            let wanted = low_byte(unit, site, character);
            let found = build(unit, site, Opcode::ICmp(IntPredicate::Eq), flag, vec![first, wanted]);
            let result = unit.function.instruction(site.call).ty;
            return Some(Rewrite::Value(build(unit, site, Opcode::Select, result, vec![found, source, none])));
        }
        _ => {}
    }
    let mut text = string_info(unit, site, source, false)?;
    let result = unit.function.instruction(site.call).ty;
    let size_ty = unit.function.operand_type(unit.context, size)?;
    if let Some(wanted) = wanted {
        let Some(at) = text.iter().position(|&one| one == wanted as u8) else {
            // The character is not in the array: null whatever the size.
            return Some(Rewrite::Value(none));
        };
        // memchr(s, c, n) -> n <= Pos ? null : s + Pos
        let bound = Operand::Constant(unit.context.int(size_ty, at as i128));
        let small = build(unit, site, Opcode::ICmp(IntPredicate::Ule), flag, vec![size, bound]);
        let index = Operand::Constant(unit.context.int(unit.context.types.int(64), at as i128));
        let plus = gep(unit, site, source, index);
        return Some(Rewrite::Value(build(unit, site, Opcode::Select, result, vec![small, none, plus])));
    }
    if text.is_empty() {
        // The only valid size of an empty array is zero.
        return Some(Rewrite::Value(none));
    }
    if let Some(length) = length {
        text.truncate(length.min(text.len() as u64) as usize);
    }
    // At most two runs of one character: N != 0 && *S == C ? S : (N > Pos &&
    // S[Pos] == C ? S + Pos : null).
    let pos = text.iter().position(|&one| one != text[0]);
    let two_runs = pos.is_none_or(|pos| text[pos..].iter().all(|&one| one == text[pos]));
    if two_runs {
        let sought = low_byte(unit, site, character);
        let mut second = none;
        if let Some(pos) = pos {
            let index = Operand::Constant(unit.context.int(size_ty, pos as i128));
            let there = Operand::Constant(unit.context.int(byte, i128::from(text[pos])));
            let same = build(unit, site, Opcode::ICmp(IntPredicate::Eq), flag, vec![sought, there]);
            let past = build(unit, site, Opcode::ICmp(IntPredicate::Ugt), flag, vec![size, index]);
            let both = build(unit, site, Opcode::Binary(BinaryOp::And), flag, vec![same, past]);
            let plus = gep(unit, site, source, index);
            second = build(unit, site, Opcode::Select, result, vec![both, plus, none]);
        }
        let zero = Operand::Constant(unit.context.int(size_ty, 0));
        let head = Operand::Constant(unit.context.int(byte, i128::from(text[0])));
        let first = build(unit, site, Opcode::ICmp(IntPredicate::Eq), flag, vec![head, sought]);
        let present = build(unit, site, Opcode::ICmp(IntPredicate::Ne), flag, vec![size, zero]);
        let both = build(unit, site, Opcode::Binary(BinaryOp::And), flag, vec![present, first]);
        return Some(Rewrite::Value(build(unit, site, Opcode::Select, result, vec![both, source, second])));
    }
    if length.is_none() && only_equality_compared_with(unit, site, source) {
        // S is dereferenceable: memchr(S, C, N) == S is N && *S == C.
        return Some(char_compare(unit, site, Some(size)));
    }
    None
}
