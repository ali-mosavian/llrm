use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `SimplifyDemandedInstructionBits` of an integer
/// instruction.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let instruction = c.function.instruction(inst);
    if instruction.result.is_none() || c.context.types.int_bits(instruction.ty).is_none() {
        return Visit::None;
    }
    c.simplify_demanded_instruction_bits(inst)
}

/// `@f` after `SimplifyDemandedUseBits` alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `combined` without the verifier: MIR has no `abs`, `bswap` or `fshl`
/// intrinsic, and `verify` refuses their declarations; `simplify` and the
/// ValueTracking translations still read them.
fn combined_unverified(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    llrm_mir::print::module(&module)
}

/// `@f` with the body `lines` over `args`, returning `ret`'s operand.
fn function(
    args: &str,
    ret_ty: &str,
    lines: &[&str],
    ret: &str,
) -> String {
    let body: String = lines.iter().map(|line| format!("  {line}\n")).collect();
    format!("define {ret_ty} @f({args}) {{\nb0:\n{body}  ret {ret_ty} {ret}\n}}\n")
}

/// An `or` with 255 under an `and` that reads only bits 8..15: the `or` adds
/// nothing the `and` reads, so the `and` reads `%x`.
#[test]
fn an_or_whose_bits_are_not_read_is_its_other_operand() {
    let text = function("i32 %x", "i32", &["%a = or i32 %x, 255", "%b = and i32 %a, -256"], "%b");
    assert_eq!(combined(&text), function("i32 %x", "i32", &["%b = and i32 %x, -256"], "%b"));
}

/// The inner mask has bits the outer `and` does not read: they are cleared
/// from the constant, and the outer `and` then reads only bits the inner mask
/// keeps, so it is the inner `and`.
#[test]
fn an_and_constant_loses_the_bits_no_user_reads() {
    let text = function("i16 %x", "i16", &["%a = and i16 %x, 4660", "%b = and i16 %a, 255"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%a = and i16 %x, 52"], "%a"));
}

/// An `xor` of two values with disjoint known bits is an inclusive `or`.
#[test]
fn an_xor_of_disjoint_masks_is_a_disjoint_or() {
    let text = function(
        "i16 %x, i16 %y",
        "i16",
        &["%a = and i16 %x, 240", "%b = and i16 %y, 15", "%c = xor i16 %a, %b"],
        "%c",
    );
    assert_eq!(
        combined(&text),
        function(
            "i16 %x, i16 %y",
            "i16",
            &["%a = and i16 %x, 240", "%b = and i16 %y, 15", "%0 = or disjoint i16 %a, %b"],
            "%0",
        )
    );
}

/// An `xor` constant that is all ones in the bits read becomes -1: a `not`.
#[test]
fn an_xor_constant_with_unread_bits_becomes_a_not() {
    let text = function("i16 %x", "i16", &["%a = xor i16 %x, 255", "%b = and i16 %a, 255"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%a = xor i16 %x, -1", "%b = and i16 %a, 255"], "%b"));
}

/// The low byte of a shifted-and-truncated value is asked: the shift moves
/// below the truncation.
#[test]
fn a_trunc_of_a_shift_with_unread_high_bits_shifts_the_narrow_value() {
    let text =
        function("i32 %x", "i8", &["%l = lshr i32 %x, 3", "%t = trunc i32 %l to i8", "%r = and i8 %t, 15"], "%r");
    assert_eq!(
        combined(&text),
        function("i32 %x", "i8", &["%0 = trunc i32 %x to i8", "%1 = lshr i8 %0, 3", "%r = and i8 %1, 15"], "%r",)
    );
}

/// An `add` constant has bits above the highest bit read: they are cleared
/// and the `add` no longer carries `nuw`/`nsw`.
#[test]
fn an_add_constant_loses_the_bits_above_the_last_one_read() {
    let text = function("i16 %x", "i16", &["%a = add nuw nsw i16 %x, 4097", "%b = and i16 %a, 255"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%a = add i16 %x, 1", "%b = and i16 %a, 255"], "%b"));
}

/// A `sext` whose extension bits are not read is a `zext`; the `and` that
/// read only the low byte is then a no-op on it.
#[test]
fn a_sext_with_unread_extension_bits_is_a_zext() {
    let text = function("i8 %x", "i16", &["%s = sext i8 %x to i16", "%b = and i16 %s, 255"], "%b");
    assert_eq!(combined(&text), function("i8 %x", "i16", &["%0 = zext i8 %x to i16"], "%0"));
}

/// A `shl` of which only bits below the shift amount are read reads nothing
/// of its operand: the operand is `undef`, and the `and` is zero.
#[test]
fn a_shl_read_below_its_amount_reads_nothing_of_its_operand() {
    let text = function("i16 %x", "i16", &["%a = shl i16 %x, 4", "%b = and i16 %a, 15"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &[], "0"));
}

/// An `ashr` of which only the low bit is read is a `lshr`.
#[test]
fn an_ashr_read_at_its_low_bit_is_a_lshr() {
    let text = function("i16 %x, i16 %n", "i16", &["%a = ashr i16 %x, %n", "%b = and i16 %a, 1"], "%b");
    assert_eq!(
        combined(&text),
        function("i16 %x, i16 %n", "i16", &["%0 = lshr i16 %x, %n", "%b = and i16 %0, 1"], "%b")
    );
}

/// `or (shl x, 3), (lshr x, 13)` is a rotate; the shifts of a rotate are left
/// alone, so `or_is_rotate` says so.
#[test]
fn the_or_of_opposite_shifts_by_complementary_constants_is_a_rotate() {
    let rotate = rotation("i16 %x", &["%a = shl i16 %x, 3", "%b = lshr i16 %x, 13", "%c = or i16 %a, %b"]);
    assert!(rotate);
}

/// Shift amounts that do not sum to the width are not a rotate.
#[test]
fn the_or_of_opposite_shifts_that_do_not_sum_to_the_width_is_not_a_rotate() {
    let rotate = rotation("i16 %x", &["%a = shl i16 %x, 3", "%b = lshr i16 %x, 12", "%c = or i16 %a, %b"]);
    assert!(!rotate);
}

/// Shifts of two different values are not a rotate.
#[test]
fn the_or_of_shifts_of_two_values_is_not_a_rotate() {
    let rotate = rotation("i16 %x, i16 %y", &["%a = shl i16 %x, 3", "%b = lshr i16 %y, 13", "%c = or i16 %a, %b"]);
    assert!(!rotate);
}

/// A rotate by a masked variable amount: `(x << (n & 15)) | (x >> (-n & 15))`.
#[test]
fn the_or_of_shifts_by_a_masked_amount_and_its_negation_is_a_rotate() {
    let rotate = rotation(
        "i16 %x, i16 %n",
        &[
            "%m = and i16 %n, 15",
            "%neg = sub i16 0, %n",
            "%k = and i16 %neg, 15",
            "%a = shl i16 %x, %m",
            "%b = lshr i16 %x, %k",
            "%c = or i16 %a, %b",
        ],
    );
    assert!(rotate);
}

/// Whether `or_is_rotate` holds for the last `or` of `@f`, read through the
/// driver: the `or` is replaced by 1 when it does, 0 when it does not.
fn rotation(
    args: &str,
    lines: &[&str],
) -> bool {
    fn probe(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Or) {
            return Visit::None;
        }
        let ty = c.function.instruction(inst).ty;
        let found = c.or_is_rotate(inst);
        Visit::Replace(c.int(ty, u128::from(found)))
    }
    let text = function(args, "i16", lines, "%c");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, probe);
    printed(&module).contains("ret i16 1")
}

const DECLS: &str = "declare i16 @llvm.abs.i16(i16, i1)
declare i16 @llvm.bswap.i16(i16)
declare i16 @llvm.umax.i16(i16, i16)
declare i16 @llvm.umin.i16(i16, i16)
declare i16 @llvm.fshl.i16(i16, i16, i16)

";

/// `function` with the intrinsic declarations first. A call is never dead
/// here, so these tests read the rewritten user.
fn with_decls(text: String) -> String {
    format!("{DECLS}{text}")
}

/// An `or` of values with no common bits is `or disjoint`.
#[test]
fn an_or_of_values_with_no_common_bits_is_marked_disjoint() {
    let text =
        function("i16 %x, i16 %y", "i16", &["%a = and i16 %x, 240", "%b = and i16 %y, 15", "%c = or i16 %a, %b"], "%c");
    assert_eq!(
        combined(&text),
        function(
            "i16 %x, i16 %y",
            "i16",
            &["%a = and i16 %x, 240", "%b = and i16 %y, 15", "%c = or disjoint i16 %a, %b"],
            "%c",
        )
    );
}

/// `(x | 255) ^ 15` clears the bits the `xor` flips, all already set: an
/// `and` with the complement, which then reads only the bits 4..15 of the
/// `or` and shrinks its constant.
#[test]
fn an_xor_that_flips_bits_known_set_is_an_and() {
    let text = function("i16 %x", "i16", &["%o = or i16 %x, 255", "%r = xor i16 %o, 15"], "%r");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%o = or i16 %x, 240", "%0 = and i16 %o, -16"], "%0"));
}

/// A `select` constant with bits no user reads is shrunk to the bits read.
#[test]
fn a_select_constant_loses_the_bits_no_user_reads() {
    let text = function("i1 %c, i16 %x", "i16", &["%s = select i1 %c, i16 17, i16 %x", "%b = and i16 %s, 1"], "%b");
    assert_eq!(
        combined(&text),
        function("i1 %c, i16 %x", "i16", &["%s = select i1 %c, i16 1, i16 %x", "%b = and i16 %s, 1"], "%b")
    );
}

/// A `sext` of a value whose sign bit is known clear is a `zext`.
#[test]
fn a_sext_of_a_non_negative_value_is_a_zext() {
    let text = function("i8 %x", "i16", &["%m = and i8 %x, 127", "%s = sext i8 %m to i16"], "%s");
    assert_eq!(combined(&text), function("i8 %x", "i16", &["%m = and i8 %x, 127", "%0 = zext i8 %m to i16"], "%0"));
}

/// `(zext i1 p) + (sext i1 q)` with the low bit unread is `sext (~p & q)`.
#[test]
fn a_zext_plus_a_sext_of_bools_with_the_low_bit_unread_is_a_sext_of_logic() {
    let text = function(
        "i1 %p, i1 %q",
        "i16",
        &["%z = zext i1 %p to i16", "%s = sext i1 %q to i16", "%a = add i16 %z, %s", "%b = and i16 %a, -2"],
        "%b",
    );
    assert_eq!(
        combined(&text),
        function(
            "i1 %p, i1 %q",
            "i16",
            &["%0 = xor i1 %p, true", "%1 = and i1 %0, %q", "%2 = sext i1 %1 to i16", "%b = and i16 %2, -2"],
            "%b",
        )
    );
}

/// `255 - x` read in its low byte is `~x`.
#[test]
fn a_sub_from_a_low_mask_read_within_the_mask_is_a_not() {
    let text = function("i16 %x", "i16", &["%a = sub i16 255, %x", "%b = and i16 %a, 255"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = xor i16 %x, -1", "%b = and i16 %0, 255"], "%b"));
}

/// Bit 3 of `x * 8` is bit 0 of `x`, shifted: a `shl`.
#[test]
fn a_mul_read_at_one_bit_is_a_shl() {
    let text = function("i16 %x", "i16", &["%a = mul i16 %x, 8", "%b = and i16 %a, 8"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = shl i16 %x, 3", "%b = and i16 %0, 8"], "%b"));
}

/// The low two bits of `x * x` are `x & 1` and a zero; the `and` with 3 is
/// then a no-op.
#[test]
fn a_square_read_in_two_bits_is_the_low_bit_of_its_operand() {
    let text = function("i16 %x", "i16", &["%a = mul i16 %x, %x", "%b = and i16 %a, 3"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = and i16 %x, 1"], "%0"));
}

/// A `shl nsw` of a value that is all sign bits, read above the shifted-in
/// zeros, is its operand.
#[test]
fn a_shl_nsw_of_sign_bits_read_above_the_amount_is_its_operand() {
    let text =
        function("i1 %p", "i16", &["%e = sext i1 %p to i16", "%a = shl nsw i16 %e, 4", "%b = and i16 %a, -16"], "%b");
    assert_eq!(combined(&text), function("i1 %p", "i16", &["%e = sext i1 %p to i16", "%b = and i16 %e, -16"], "%b"));
}

/// `(x >> 4) << 4` read above bit 4 is `x`.
#[test]
fn a_shift_pair_of_equal_amounts_read_above_the_amount_is_the_operand() {
    let text = function("i16 %x", "i16", &["%r = lshr i16 %x, 4", "%l = shl i16 %r, 4", "%b = and i16 %l, -16"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%b = and i16 %x, -16"], "%b"));
}

/// `(x >> 2) << 4` read above bit 4 is `x << 2`.
#[test]
fn a_shift_pair_read_above_the_amounts_is_one_shift() {
    let text = function("i16 %x", "i16", &["%r = lshr i16 %x, 2", "%l = shl i16 %r, 4", "%b = and i16 %l, -16"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = shl i16 %x, 2", "%b = and i16 %0, -16"], "%b"));
}

/// A `lshr` of sign bits read only at bits that are sign bits is its operand.
#[test]
fn a_lshr_of_sign_bits_read_in_sign_bits_is_its_operand() {
    let text =
        function("i1 %p", "i16", &["%e = sext i1 %p to i16", "%l = lshr i16 %e, 4", "%b = and i16 %l, 15"], "%b");
    assert_eq!(combined(&text), function("i1 %p", "i16", &["%e = sext i1 %p to i16", "%b = and i16 %e, 15"], "%b"));
}

/// `(x * 8) >> 3` read below bit 13 is `x * 1`.
#[test]
fn a_lshr_of_a_mul_whose_factor_has_the_zeros_is_a_smaller_mul() {
    let text = function("i16 %x", "i16", &["%m = mul i16 %x, 8", "%l = lshr i16 %m, 3", "%b = and i16 %l, 255"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = mul i16 %x, 1", "%b = and i16 %0, 255"], "%b"));
}

/// An `ashr` whose shifted-in bits are not read is a `lshr`; the `and` that
/// read the bits it leaves is then a no-op.
#[test]
fn an_ashr_whose_shifted_in_bits_are_unread_is_a_lshr() {
    let text = function("i16 %x", "i16", &["%a = ashr exact i16 %x, 4", "%b = and i16 %a, 4095"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = lshr exact i16 %x, 4"], "%0"));
}

/// A `udiv` by 4 does not read the low two bits of the dividend.
#[test]
fn a_udiv_does_not_read_the_low_bits_the_divisor_zeroes() {
    let text = function("i16 %z", "i16", &["%y = or i16 %z, 3", "%a = udiv i16 %y, 4"], "%a");
    assert_eq!(combined(&text), function("i16 %z", "i16", &["%a = udiv i16 %z, 4"], "%a"));
}

/// `srem x, 16` read in the low four bits is `x`.
#[test]
fn a_srem_by_a_power_of_two_read_below_it_is_its_operand() {
    let text = function("i16 %x", "i16", &["%a = srem i16 %x, 16", "%b = and i16 %a, 15"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%b = and i16 %x, 15"], "%b"));
}

/// `abs x` read at bit 0 is `x`.
#[test]
fn an_abs_read_at_the_low_bit_is_its_operand() {
    let text = with_decls(function(
        "i16 %x",
        "i16",
        &["%a = call i16 @llvm.abs.i16(i16 %x, i1 false)", "%b = and i16 %a, 1"],
        "%b",
    ));
    let out = combined_unverified(&text);
    assert!(out.contains("%b = and i16 %x, 1"), "{out}");
}

/// One byte of a `bswap` is the input byte, shifted.
#[test]
fn a_bswap_read_in_one_byte_is_a_shift() {
    let text =
        with_decls(function("i16 %x", "i16", &["%a = call i16 @llvm.bswap.i16(i16 %x)", "%b = and i16 %a, 255"], "%b"));
    let out = combined_unverified(&text);
    assert!(out.contains("%0 = lshr i16 %x, 8"), "{out}");
    assert!(out.contains("ret i16 %0"), "{out}");
}

/// `umax(x, 15)` read above the constant's bits is `x`.
#[test]
fn a_umax_with_a_constant_read_above_its_bits_is_the_other_operand() {
    let text = with_decls(function(
        "i16 %x",
        "i16",
        &["%a = call i16 @llvm.umax.i16(i16 %x, i16 15)", "%b = and i16 %a, -16"],
        "%b",
    ));
    let out = combined_unverified(&text);
    assert!(out.contains("%b = and i16 %x, -16"), "{out}");
}

/// `umin(x, -16)` read above the constant's zero bits is `x`.
#[test]
fn a_umin_with_a_constant_read_above_its_zero_bits_is_the_other_operand() {
    let text = with_decls(function(
        "i16 %x",
        "i16",
        &["%a = call i16 @llvm.umin.i16(i16 %x, i16 -16)", "%b = and i16 %a, -16"],
        "%b",
    ));
    let out = combined_unverified(&text);
    assert!(out.contains("%b = and i16 %x, -16"), "{out}");
}

/// A rotate's operand is not narrowed to the bits one user reads: the `shl`
/// that feeds a rotate keeps its operand (`mask` below would shrink it).
#[test]
fn the_shifts_of_a_rotate_are_not_simplified() {
    let text = function(
        "i16 %y",
        "i16",
        &[
            "%x = or i16 %y, 255",
            "%a = shl i16 %x, 3",
            "%b = lshr i16 %x, 13",
            "%c = or i16 %a, %b",
            "%d = and i16 %c, -256",
        ],
        "%d",
    );
    assert!(combined(&text).contains("%a = shl i16 %x, 3"), "{}", combined(&text));
}

/// A value with two users: one reads only the bits an `or` does not set, so
/// that user reads the `or`'s other operand, and the `or` stays for the other.
#[test]
fn an_or_with_two_users_is_replaced_for_the_user_that_does_not_read_its_bits() {
    let text =
        function("i16 %x", "i16", &["%o = or i16 %x, 255", "%a = and i16 %o, -256", "%s = add i16 %a, %o"], "%s");
    assert_eq!(
        combined(&text),
        function("i16 %x", "i16", &["%o = or i16 %x, 255", "%a = and i16 %x, -256", "%s = add i16 %a, %o"], "%s")
    );
}

/// An `add` with two users, one of which reads only the low byte, where the
/// addend has none: that user reads the other operand.
#[test]
fn an_add_with_two_users_is_replaced_for_the_user_that_reads_bits_the_addend_lacks() {
    let text = function(
        "i16 %x, i16 %y",
        "i16",
        &["%m = and i16 %y, -256", "%s = add i16 %x, %m", "%b = and i16 %s, 255", "%c = xor i16 %s, %b"],
        "%c",
    );
    let out = combined(&text);
    assert!(out.contains("%b = and i16 %x, 255"), "{out}");
    assert!(out.contains("%c = xor i16 %s, %b"), "{out}");
}

/// The same for a `sub`.
#[test]
fn a_sub_with_two_users_is_replaced_for_the_user_that_reads_bits_the_subtrahend_lacks() {
    let text = function(
        "i16 %x, i16 %y",
        "i16",
        &["%m = and i16 %y, -256", "%s = sub i16 %x, %m", "%b = and i16 %s, 255", "%c = xor i16 %s, %b"],
        "%c",
    );
    let out = combined(&text);
    assert!(out.contains("%b = and i16 %x, 255"), "{out}");
    assert!(out.contains("%c = xor i16 %s, %b"), "{out}");
}

/// `(x << 4) >>s 4` is a sign extension in the register; a user that reads
/// only the bits below the extension reads `x`, the other user keeps it.
#[test]
fn a_sign_extension_in_register_with_two_users_is_its_operand_where_the_extension_is_unread() {
    let text = function(
        "i16 %x",
        "i16",
        &["%e = shl i16 %x, 4", "%a = ashr i16 %e, 4", "%b = and i16 %a, 4095", "%c = xor i16 %a, %b"],
        "%c",
    );
    let out = combined(&text);
    assert!(out.contains("%b = and i16 %x, 4095"), "{out}");
    assert!(out.contains("%c = xor i16 %a, %b"), "{out}");
}

/// `lshr (hi << 8 | lo), n` read in 8 bits, `n` 0 or 8: a select of the
/// halves (a zext of a byte masked with 255 is itself).
#[test]
fn a_shift_selecting_a_packed_half_is_a_select() {
    let text = function(
        "i8 %h, i8 %l, i16 %k",
        "i16",
        &[
            "%zh = zext i8 %h to i16",
            "%zl = zext i8 %l to i16",
            "%u = shl i16 %zh, 8",
            "%p = or disjoint i16 %u, %zl",
            "%n = and i16 %k, 8",
            "%r = lshr i16 %p, %n",
            "%q = and i16 %r, 255",
        ],
        "%q",
    );
    assert_eq!(
        combined(&text),
        function(
            "i8 %h, i8 %l, i16 %k",
            "i16",
            &[
                "%zh = zext i8 %h to i16",
                "%zl = zext i8 %l to i16",
                "%n = and i16 %k, 8",
                "%0 = icmp eq i16 %n, 0",
                "%1 = select i1 %0, i16 %zl, i16 %zh",
            ],
            "%1",
        )
    );
}

/// `(15 >> x) << 4` read above bit 4 is `240 >> x`; the mask then shrinks to
/// the bits that value can have.
#[test]
fn a_shl_of_a_lshr_of_a_constant_moves_the_constant_through_the_shift() {
    let text = function("i16 %x", "i16", &["%l = lshr i16 15, %x", "%s = shl i16 %l, 4", "%b = and i16 %s, -16"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = lshr i16 240, %x", "%b = and i16 %0, 240"], "%b"));
}

/// `(240 << x) >> 4` read in its low four bits is `15 << x`.
#[test]
fn a_lshr_of_a_shl_of_a_constant_moves_the_constant_through_the_shift() {
    let text = function("i16 %x", "i16", &["%l = shl i16 240, %x", "%s = lshr i16 %l, 4", "%b = and i16 %s, 15"], "%b");
    assert_eq!(combined(&text), function("i16 %x", "i16", &["%0 = shl i16 15, %x", "%b = and i16 %0, 15"], "%b"));
}

/// A rotate whose second half is read only in bits known zero takes the
/// constant 0 for it.
#[test]
fn a_rotate_reading_only_known_bits_takes_constant_operands() {
    let text = with_decls(function(
        "i16 %x",
        "i16",
        &["%c = and i16 %x, 255", "%f = call i16 @llvm.fshl.i16(i16 %c, i16 %c, i16 4)"],
        "%f",
    ));
    let out = combined_unverified(&text);
    assert!(out.contains("@llvm.fshl.i16(i16 %c, i16 0, i16 4)"), "{out}");
}
