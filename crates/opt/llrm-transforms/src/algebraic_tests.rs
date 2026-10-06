//! Adapted from llrm-core's `optimize/algebraic_tests.rs`, the port of
//! `tests/test_algebraic.py`, each body now MIR text the interpreter runs
//! before and after.
//!
//! Skipped:
//! - The guards naming flags, merges, memory operands or a result wider
//!   than its operands: no instruction here has them.
//! - test_shared_shift_distinguishes_a_word_tie_from_a_partial_write,
//!   test_extracted_halves_recombine_to_the_original_value,
//!   test_joined_halves_are_consumed_as_halves,
//!   test_recombination_follows_only_exact_word_copies,
//!   test_signed_recombination_compares_copy_sources_symmetrically,
//!   test_constant_word_concatenation,
//!   test_product_projection_retains_observed_outputs, and the zero
//!   difference tests: split halves, copies and the neg instruction.
//! - The NBODY, HARR, MATRIX, NDMAX, HOTLPX, SPILL, ADDRM and NESTED
//!   fixtures: BC objects and emitted x86. ADDRM's shared scale and NBODY's
//!   shift pair are `shared_shifts_*` and `shifts_*` here.

use llrm_mir::interpret::Val;
use llrm_mir::module::Module;

use crate::testing::{parsed, printed, results};

/// The layout's legal integers, a 486's.
const LEGAL: &str = "n8:16:32";

/// `module` printed without its layout.
fn bare(module: &Module) -> String {
    printed(&Module { datalayout: None, ..module.clone() })
}

/// `text` through `Algebraic` under the verifier and preserved-analyses
/// check.
fn simplified(text: &str) -> (Module, Module) {
    let mut before = parsed(text);
    before.datalayout = Some(LEGAL.to_owned());
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(super::Algebraic);
    passes.run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap_or_else(|error| panic!("{error}\n{text}"));
    (before, after)
}

/// Edge values of a `width`-bit integer, and a few between.
fn edges(width: u32) -> Vec<i128> {
    let top = 1_i128 << (width - 1);
    vec![0, 1, -1, 2, -2, 3, 7, -7, 100, -100, top - 1, -top, top - 2, -top + 1]
}

fn singles(values: &[i128]) -> Vec<Vec<i128>> {
    values.iter().map(|&one| vec![one]).collect()
}

/// `text` simplified, answering as before on `inputs` wherever the
/// original was not poison; its text.
fn checked(text: &str, inputs: &[Vec<i128>]) -> String {
    let (before, after) = simplified(text);
    let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
    let expected = results(&before, &inputs);
    let refined = results(&after, &inputs).into_iter().zip(&expected).map(|(got, wanted)| if *wanted == Val::Poison { Val::Poison } else { got });
    assert_eq!(refined.collect::<Vec<_>>(), expected, "{}", bare(&after));
    bare(&after)
}

/// `text` left as it was.
fn unchanged(text: &str) {
    let (before, after) = simplified(text);
    assert_eq!(bare(&after), bare(&before));
}

fn unary(width: u32, body: &str) -> String {
    format!("define i{width} @f(i{width} %x) {{\nb0:\n{body}}}\n")
}

#[test]
fn test_offset_composition_preserves_modular_values() {
    for (op, combined) in [("add", 14), ("sub", -26)] {
        let text = unary(16, &format!("  %m = add i16 %x, -6\n  %r = {op} i16 %m, 20\n  ret i16 %r\n"));
        assert_eq!(checked(&text, &singles(&edges(16))), unary(16, &format!("  %0 = add i16 %x, {combined}\n  ret i16 %0\n")));
        unchanged(&unary(16, &format!("  %m = add i16 %x, -6\n  %r = {op} i16 %m, 20\n  %s = add i16 %m, %r\n  ret i16 %s\n")));
    }
}

#[test]
fn test_associative_bitwise_constants_combine() {
    for (op, first, last, combined) in [("and", 0xF0F3_u16, 0x3FFF_u16, 0x30F3_u16), ("or", 0xF003, 0x0F30, 0xFF33), ("xor", 0xFFFF, 0x0031, 0xFFCE)] {
        let (first, last, combined) = (first as i16, last as i16, combined as i16);
        let text = unary(16, &format!("  %m = {op} i16 %x, {first}\n  %r = {op} i16 {last}, %m\n  ret i16 %r\n"));
        assert_eq!(checked(&text, &singles(&edges(16))), unary(16, &format!("  %0 = {op} i16 %x, {combined}\n  ret i16 %0\n")));
        unchanged(&unary(16, &format!("  %m = {op} i16 %x, {first}\n  %r = {op} i16 %m, {last}\n  %s = add i16 %m, %r\n  ret i16 %s\n")));
    }
}

#[test]
fn test_mixed_bitwise_operations_stay_apart() {
    unchanged(&unary(16, "  %m = and i16 %x, 255\n  %r = or i16 %m, 4096\n  ret i16 %r\n"));
}

#[test]
fn test_scaled_chain_preserves_modular_values() {
    let text = unary(16, "  %m = mul i16 %x, -32767\n  %r = shl i16 %m, 1\n  ret i16 %r\n");
    assert_eq!(checked(&text, &singles(&edges(16))), unary(16, "  %0 = mul i16 %x, 2\n  ret i16 %0\n"));
    unchanged(&unary(16, "  %m = mul i16 %x, -32767\n  %r = shl i16 %m, 1\n  %s = add i16 %m, %r\n  ret i16 %s\n"));
}

/// NBODY computed other*4 with two shifts.
#[test]
fn test_shifts_combine_short_of_the_width() {
    let text = unary(16, "  %m = shl i16 %x, 1\n  %r = shl i16 %m, 1\n  ret i16 %r\n");
    assert_eq!(checked(&text, &singles(&edges(16))), unary(16, "  %0 = shl i16 %x, 2\n  ret i16 %0\n"));
    unchanged(&unary(16, "  %m = shl i16 %x, 1\n  %r = shl i16 %m, 1\n  %s = add i16 %m, %r\n  ret i16 %s\n"));
    // A count past the width is poison, and stays.
    unchanged(&unary(16, "  %m = shl i16 %x, 16\n  %r = shl i16 %m, 1\n  ret i16 %r\n"));
}

/// Shifted out entirely is zero, not a shift by the width, which is poison.
#[test]
fn test_shifts_reaching_the_width_are_zero() {
    let text = unary(16, "  %m = shl i16 %x, 15\n  %r = shl i16 %m, 1\n  ret i16 %r\n");
    assert_eq!(checked(&text, &singles(&edges(16))), unary(16, "  ret i16 0\n"));
}

/// ADDRM rebuilt i*4 after using i*2.
#[test]
fn test_shared_shifts_reuse_a_smaller_used_scale_in_the_block() {
    let text = unary(16, "  %a = shl i16 %x, 1\n  %b = shl i16 %x, 3\n  %s = add i16 %a, %b\n  ret i16 %s\n");
    assert_eq!(checked(&text, &singles(&edges(16))), unary(16, "  %a = shl i16 %x, 1\n  %b = shl i16 %a, 2\n  %s = add i16 %a, %b\n  ret i16 %s\n"));
    // Nothing reads the smaller one.
    unchanged(&unary(16, "  %a = shl i16 %x, 1\n  %b = shl i16 %x, 3\n  ret i16 %b\n"));
    // It is in another block.
    unchanged(&unary(16, "  %a = shl i16 %x, 1\n  br label %b1\n\nb1:\n  %b = shl i16 %x, 3\n  %s = add i16 %a, %b\n  ret i16 %s\n"));
}

#[test]
fn test_negating_a_single_use_difference_reverses_it() {
    let text = "define i32 @f(i32 %a, i32 %b) {\nb0:\n  %d = sub i32 %a, %b\n  %n = sub i32 0, %d\n  ret i32 %n\n}\n";
    let values = edges(32);
    let pairs: Vec<Vec<i128>> = values.iter().flat_map(|&a| values.iter().map(move |&b| vec![a, b])).collect();
    assert_eq!(checked(text, &pairs), "define i32 @f(i32 %a, i32 %b) {\nb0:\n  %0 = sub i32 %b, %a\n  ret i32 %0\n}\n");
    unchanged("define i32 @f(i32 %a, i32 %b) {\nb0:\n  %d = sub i32 %a, %b\n  %n = sub i32 0, %d\n  %s = add i32 %d, %n\n  ret i32 %s\n}\n");
}

#[test]
fn test_signed_power_division_preserves_quotient_and_remainder() {
    for (width, divisor) in [(32_u32, 2_i128), (32, 16), (32, 512), (32, 262144), (16, 2), (16, 16), (16, 16384), (8, 4)] {
        let top = 1_i128 << (width - 1);
        let mut dividends = edges(width);
        dividends.extend([-divisor - 1, -divisor, -divisor + 1, divisor - 1, divisor, divisor + 1].iter().map(|n| (n + top).rem_euclid(2 * top) - top));
        for op in ["sdiv", "srem"] {
            let text = unary(width, &format!("  %r = {op} i{width} %x, {divisor}\n  ret i{width} %r\n"));
            let done = checked(&text, &singles(&dividends));
            assert!(!done.contains(op), "{done}");
        }
    }
}

/// The divisor is a number consts proves, not a constant operand.
#[test]
fn test_a_divisor_known_through_a_phi_is_a_power_of_two() {
    let text = "define i32 @f(i32 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %d = phi i32 [ 16, %b0 ], [ 16, %b1 ]
  %q = sdiv i32 %x, %d
  ret i32 %q
}
";
    let inputs: Vec<Vec<i128>> = edges(32).into_iter().flat_map(|x| [vec![x, 0], vec![x, 1]]).collect();
    assert!(!checked(text, &inputs).contains("sdiv"));
}

/// Not a positive power of two below the sign bit, possibly zero, or unsigned.
#[test]
fn test_other_divisions_stay() {
    for divisor in ["6", "-4", "-32768", "0"] {
        for op in ["sdiv", "srem"] {
            unchanged(&unary(16, &format!("  %r = {op} i16 %x, {divisor}\n  ret i16 %r\n")));
        }
    }
    unchanged("define i16 @f(i16 %x, i16 %y) {\nb0:\n  %r = sdiv i16 %x, %y\n  ret i16 %r\n}\n");
    unchanged(&unary(16, "  %r = udiv i16 %x, 6\n  ret i16 %r\n"));
}

/// A division by one is the dividend, a remainder by one is zero: no power-of-two
/// rewrite is needed (`llrm-mir`'s instcombine had both, #237).
#[test]
fn test_division_and_remainder_by_one_are_decided() {
    for op in ["sdiv", "udiv"] {
        let out = checked(&unary(16, &format!("  %r = {op} i16 %x, 1\n  ret i16 %r\n")), &singles(&edges(16)));
        assert!(out.contains("ret i16 %x") && !out.contains(op), "{out}");
    }
    for op in ["srem", "urem"] {
        let out = checked(&unary(16, &format!("  %r = {op} i16 %x, 1\n  ret i16 %r\n")), &singles(&edges(16)));
        assert!(out.contains("ret i16 0") && !out.contains(op), "{out}");
    }
}

/// No legal integer is 64 bits wide, so its shifts and adds lower to more
/// than the division: T076's fixed `/4` ran some 16 instructions for one
/// `idiv`.
#[test]
fn test_a_division_wider_than_a_legal_integer_stays() {
    unchanged(&unary(64, "  %r = sdiv i64 %x, 4\n  ret i64 %r\n"));
}

/// A fixed-point product or quotient by a whole number is the integer
/// one: T075's `c[i, j] * fix(k)` and `fix(n) / 4` were an `imul` pair and
/// `shrd`, and a wrapping division, where one multiply and a shift did.
#[test]
fn test_fixed_point_by_a_whole_number_is_integer_arithmetic() {
    let values = edges(32);
    for (name, factor, op) in [("smul", 768, "mul"), ("smul", -512, "mul"), ("sdiv", 768, "sdiv"), ("sdiv", -1024, "sdiv")] {
        let text = format!("declare i32 @llvm.{name}.fix.i32(i32, i32, i32)\n\n{}", unary(32, &format!("  %r = call i32 @llvm.{name}.fix.i32(i32 %x, i32 {factor}, i32 8)\n  ret i32 %r\n")));
        let done = checked(&text, &singles(&values));
        assert!(!done.contains("call i32") && (done.contains(op) || done.contains("ashr")), "{done}");
    }
    // By -1.0 the quotient of the least value wraps; `sdiv` would be undefined.
    unchanged(&format!("declare i32 @llvm.sdiv.fix.i32(i32, i32, i32)\n\n{}", unary(32, "  %r = call i32 @llvm.sdiv.fix.i32(i32 %x, i32 -256, i32 8)\n  ret i32 %r\n")));
    unchanged(&format!("declare i32 @llvm.smul.fix.i32(i32, i32, i32)\n\n{}", unary(32, "  %r = call i32 @llvm.smul.fix.i32(i32 %x, i32 384, i32 8)\n  ret i32 %r\n")));
}

/// The pass needs no constant propagation to drop `x + 0`.
#[test]
fn test_integer_identities() {
    for width in [16_u32, 32] {
        for (op, number, answer) in [
            ("add", 0, None),
            ("sub", 0, None),
            ("mul", 1, None),
            ("or", 0, None),
            ("xor", 0, None),
            ("and", -1, None),
            ("and", 0, Some(0)),
            ("mul", 0, Some(0)),
            ("or", -1, Some(-1)),
            ("shl", 0, None),
            ("lshr", 0, None),
            ("ashr", 0, None),
        ] {
            let text = unary(width, &format!("  %r = {op} i{width} %x, {number}\n  ret i{width} %r\n"));
            let answer = answer.map_or("%x".to_owned(), |one: i32| one.to_string());
            assert_eq!(checked(&text, &singles(&edges(width))), unary(width, &format!("  ret i{width} {answer}\n")), "{op} {number}");
        }
    }
    // A constant on the left, where the operation commutes.
    assert_eq!(checked(&unary(16, "  %r = add i16 0, %x\n  ret i16 %r\n"), &singles(&edges(16))), unary(16, "  ret i16 %x\n"));
    unchanged(&unary(16, "  %r = sub i16 0, %x\n  ret i16 %r\n"));
    unchanged(&unary(16, "  %r = shl i16 0, %x\n  %s = add i16 %r, 1\n  ret i16 %s\n"));
}

#[test]
fn test_a_symbol_plus_zero_is_the_symbol() {
    let (_, after) = simplified("@g = global i16 0\n\ndefine i16 @f() {\nb0:\n  %r = add i16 0, ptrtoint (ptr @g to i16)\n  ret i16 %r\n}\n");
    assert!(printed(&after).contains("  ret i16 ptrtoint (ptr @g to i16)\n"), "{}", printed(&after));
}

/// Two integer casts are one, or none, and answer as before at every edge
/// value: `trunc(ext x)` is `x` or one cast, `trunc(trunc x)` one `trunc`,
/// an extension of a `zext` or of a like one one extension. `ext(trunc x)`
/// and `zext(sext x)` stay two. Only `ext(trunc(ext x))` was folded.
#[test]
fn test_a_cast_pair_is_one_cast_or_none() {
    let casts = |text: &str| text.lines().filter(|line| ["trunc ", "zext ", "sext "].iter().any(|op| line.contains(op))).count();
    for source in [8, 16, 32] {
        for middle in [8, 16, 32] {
            for result in [8, 16, 32] {
                for first in ["trunc", "zext", "sext"] {
                    for second in ["trunc", "zext", "sext"] {
                        let fits = |op: &str, from: u32, to: u32| if op == "trunc" { to < from } else { to > from };
                        if !fits(first, source, middle) || !fits(second, middle, result) {
                            continue;
                        }
                        let text = format!("define i{result} @f(i{source} %x) {{\nb0:\n  %m = {first} i{source} %x to i{middle}\n  %r = {second} i{middle} %m to i{result}\n  ret i{result} %r\n}}\n");
                        let after = checked(&text, &singles(&edges(source)));
                        let stays = (first == "trunc" && second != "trunc") || (first, second) == ("sext", "zext");
                        assert_eq!(casts(&after), if stays { 2 } else { usize::from(source != result) }, "{text}{after}");
                    }
                }
            }
        }
    }
}

/// An extension of a truncated extension is the first extension's bits
/// where the truncation kept them: two pair steps.
#[test]
fn test_reextending_an_already_extended_low_byte_reads_the_extension() {
    for op in ["zext", "sext"] {
        for established in [16, 32] {
            let text = format!("define i16 @f(i8 %x) {{\nb0:\n  %m = {op} i8 %x to i{established}\n  %v = trunc i{established} %m to i8\n  %r = {op} i8 %v to i16\n  ret i16 %r\n}}\n");
            assert_eq!(checked(&text, &singles(&edges(8))), format!("define i16 @f(i8 %x) {{\nb0:\n  %r = {op} i8 %x to i16\n  ret i16 %r\n}}\n"));
        }
    }
}

#[test]
fn test_an_offset_scaled_then_offset_distributes() {
    for (scale, factor) in [("mul i16 %i, 5", "mul i16 %x, 5"), ("shl i16 %i, 2", "shl i16 %x, 2")] {
        let text = unary(16, &format!("  %i = add i16 %x, 3\n  %m = {scale}\n  %r = add i16 %m, 7\n  ret i16 %r\n"));
        let moved = if scale.starts_with("mul") { 22 } else { 19 };
        assert_eq!(checked(&text, &singles(&edges(16))), unary(16, &format!("  %0 = {factor}\n  %1 = add i16 %0, {moved}\n  ret i16 %1\n")));
    }
    unchanged(&unary(16, "  %i = add i16 %x, 3\n  %m = mul i16 %i, 5\n  %r = add i16 %m, 7\n  %s = add i16 %i, %r\n  ret i16 %s\n"));
}

/// Without `nsw` kept, a sum the original never overflowed does not
/// become poison when its constants combine.
#[test]
fn test_combined_constants_drop_overflow_flags() {
    let text = unary(16, "  %m = add nsw i16 %x, 30000\n  %r = add nsw i16 %m, 30000\n  ret i16 %r\n");
    // Where neither original sum overflows.
    let done = checked(&text, &singles(&[-32768, -30000, -27233]));
    assert_eq!(done, unary(16, "  %0 = add i16 %x, -5536\n  ret i16 %0\n"));
}

/// `sum += (i & 7) << 2` over a counted loop, `n` trips.
fn masked_sum(mask_of: &str) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %t, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %m = and i16 {mask_of}, 7
  %k = shl i16 %m, 2
  %t = add i16 %s, %k
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

#[test]
fn test_a_mask_of_a_counter_moves_after_its_scale() {
    let trips = singles(&[0, 1, 5, 8, 9, 100, -1, -32768]);
    let done = checked(&masked_sum("%i"), &trips);
    assert!(done.contains("  %0 = shl i16 %i, 2\n  %1 = and i16 %0, 28\n  %t = add i16 %s, %1\n"), "{done}");
    // A mask of what does not advance keeps its scale.
    unchanged(&masked_sum("%n"));
}

/// `s += a + b` with `b` computed between the two sums.
#[test]
fn test_a_recurrence_moves_to_the_root_of_its_sum() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %inner = add nsw i16 %s, %a
  %b = mul i16 %a, 3
  %next = add nsw i16 %inner, %b
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}
";
    let inputs: Vec<Vec<i128>> = [0, 1, 7, 50].into_iter().flat_map(|n| edges(16).into_iter().map(move |a| vec![n, a])).collect();
    let done = checked(text, &inputs);
    assert!(done.contains("  %b = mul i16 %a, 3\n  %inner = add i16 %a, %b\n  %next = add i16 %s, %inner\n"), "{done}");
}

/// A sum that already reads its recurrence at the root is left as it is.
#[test]
fn test_a_recurrence_already_at_the_root_stays() {
    unchanged(
        "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %s = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %inner = add i16 %s, %a
  %next = add i16 %inner, %s
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %s
}
",
    );
}

/// Two truths `and`ed as frontend words are one `i1` `and`: rcflip kept
/// both extensions and the word `and` in its hot loop.
#[test]
fn logic_of_two_like_extensions_is_the_extension_of_the_logic() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = icmp eq i16 %x, 0
  %b = icmp eq i16 %y, 1
  %wa = sext i1 %a to i16
  %wb = sext i1 %b to i16
  %both = and i16 %wa, %wb
  ret i16 %both
}
";
    let inputs = [vec![0, 1], vec![0, 0], vec![1, 1], vec![1, 0]];
    let after = checked(text, &inputs);
    assert!(after.contains("and i1 %a, %b") && after.matches("sext").count() == 1, "{after}");
}

/// A phi of two sign-extended truths stayed bytes a branch then compared
/// against zero, as N$PQ4's `&&` did.
#[test]
fn a_phi_of_like_extensions_is_the_extension_of_a_phi() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %s = icmp eq i16 %x, 0
  br i1 %s, label %b1, label %b2

b1:
  %a = icmp eq i16 %y, 1
  %wa = sext i1 %a to i16
  br label %b3

b2:
  %b = icmp ult i16 %y, 5
  %wb = sext i1 %b to i16
  br label %b3

b3:
  %p = phi i16 [ %wa, %b1 ], [ %wb, %b2 ]
  ret i16 %p
}
";
    let inputs = [vec![0, 1], vec![0, 0], vec![1, 3], vec![1, 9]];
    let after = checked(text, &inputs);
    assert!(after.contains("phi i1") && after.matches("sext").count() == 1, "{after}");
}

/// `if !(a < b)` branched on `xor (icmp slt a b), true`: one compare of the
/// inverse predicate, so the exit is the compare counting passes look for.
#[test]
fn test_a_negated_compare_is_the_inverse_compare() {
    let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %c = icmp slt i16 %a, %b
  %n = xor i1 %c, true
  br i1 %n, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 2
}
";
    let printed = checked(text, &edges(16).iter().flat_map(|&a| edges(16).into_iter().map(move |b| vec![a, b])).collect::<Vec<_>>());
    assert!(printed.contains("icmp sge i16 %a, %b") && !printed.contains("xor"), "{printed}");
    // A compare read elsewhere keeps its own value.
    let shared = text.replace("ret i16 1", "%w = zext i1 %c to i16\n  ret i16 %w");
    let kept = checked(&shared, &[vec![1, 2], vec![2, 1]]);
    assert!(kept.contains("xor"), "{kept}");
}

/// Nib's `if !(a < b)`: the compare sign-extended to a byte, complemented and
/// tested against zero, and the same through `zext` and `xor 1`. One compare
/// of the inverse predicate, where it cost `setl; neg; xor; jne` and hid the
/// exit from the counting passes.
#[test]
fn test_a_negated_extended_boolean_is_the_inverse_compare() {
    let text = "define i16 @f(i16 %a, i16 %b) {
entry:
  %c = icmp slt i16 %a, %b
  %e = sext i1 %c to i8
  %n = xor i8 %e, -1
  %t = icmp ne i8 %n, 0
  br i1 %t, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 2
}
";
    let inputs = edges(16).iter().flat_map(|&a| edges(16).into_iter().map(move |b| vec![a, b])).collect::<Vec<_>>();
    let printed = checked(text, &inputs);
    assert!(printed.contains("icmp sge i16 %a, %b") && !printed.contains("xor") && !printed.contains("sext"), "{printed}");
    let unsigned = text.replace("sext i1 %c to i8", "zext i1 %c to i8").replace("xor i8 %e, -1", "xor i8 %e, 1");
    let printed = checked(&unsigned, &inputs);
    assert!(printed.contains("icmp sge i16 %a, %b") && !printed.contains("zext"), "{printed}");
}

/// `gep i8, ptr null, -4` stayed an instruction, and isel refused it as "an
/// address of no global" (examples/entries.nib, tally.nib). A byte offset
/// from a constant near address is the constant address.
#[test]
fn a_byte_offset_from_a_constant_address_is_that_address() {
    let text = "define ptr @f() {\nb0:\n  %p = getelementptr i8, ptr null, i16 -4\n  ret ptr %p\n}\n";
    assert!(text.contains("getelementptr i8, ptr null"), "the shape that was refused");
    let after = bare(&simplified(text).1);
    assert!(after.contains("ret ptr inttoptr (i16 -4 to ptr)"), "{after}");
    let again = "define ptr @f() {\nb0:\n  %p = getelementptr i8, ptr inttoptr (i16 1132 to ptr), i16 2\n  ret ptr %p\n}\n";
    assert!(bare(&simplified(again).1).contains("inttoptr (i16 1134 to ptr)"));
}

/// `*(int *)0x46c` read the address through a register: C's cast is an
/// `inttoptr` instruction, which InstCombine makes the constant address.
#[test]
fn inttoptr_of_a_constant_is_the_constant_address() {
    let text = "define i16 @f() {\nb0:\n  %p = inttoptr i16 1132 to ptr\n  %v = load i16, ptr %p\n  ret i16 %v\n}\n";
    assert!(text.contains("%p = inttoptr i16 1132 to ptr"), "the shape that was kept");
    let after = bare(&simplified(text).1);
    assert!(after.contains("load i16, ptr inttoptr (i16 1132 to ptr)") && !after.contains("%p ="), "{after}");
}

/// Two phis of one block that take the same values from the same
/// predecessors are one value. gvn's PRE made the second beside a phi that
/// stood, and SPHEREMAPLASMA's loop carried both (+234k estimated
/// instructions, #386).
#[test]
fn test_a_phi_that_repeats_an_earlier_one_of_its_block_is_that_phi() {
    let text = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %y, %b2 ], [ %x, %b1 ]
  %r = add i16 %p, %q
  ret i16 %r
}
";
    let inputs: Vec<Vec<i128>> = vec![vec![3, 5, 0], vec![3, 5, 1], vec![-1, 7, 1]];
    let out = checked(text, &inputs);
    assert_eq!(out.matches("phi").count(), 1, "{out}");
}

/// `zext i32 (trunc i8 (and i16 x, 255))` kept the `and`, then the `trunc`,
/// then extended: `and dx,0FFh; movzx edx,dl`. The `trunc` drops bits the
/// mask cleared, so the `zext` reads the mask; that mask is a byte read.
#[test]
fn test_a_mask_then_trunc_then_zext_reads_the_low_part_once() {
    let text = "define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 255\n  %t = trunc i16 %m to i8\n  %r = zext i8 %t to i32\n  ret i32 %r\n}\n";
    let (before, _) = simplified(text);
    assert!(printed(&before).contains("and i16 %x, 255") && printed(&before).contains("trunc i16"), "premise: the mask and the trunc are there");
    let after = checked(text, &singles(&edges(16)));
    assert_eq!(after, "define i32 @f(i16 %x) {\nb0:\n  %0 = trunc i16 %x to i8\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n}\n");
}

/// A `sext` of a masked value, whose sign bit the mask cleared, is a
/// `zext`: `movsx edx,dx` after the mask in the QB loop.
#[test]
fn test_a_sign_extension_of_a_masked_value_is_a_zero_extension() {
    let text = "define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 255\n  %r = sext i16 %m to i32\n  ret i32 %r\n}\n";
    assert!(!checked(text, &singles(&edges(16))).contains("sext"));
    unchanged("define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 32768\n  %r = sext i16 %m to i32\n  ret i32 %r\n}\n");
}

/// The mask stays where it is read twice, or keeps more than a byte or word.
#[test]
fn test_a_mask_that_is_not_a_low_part_or_is_shared_stays() {
    unchanged("define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 511\n  %r = zext i16 %m to i32\n  ret i32 %r\n}\n");
    unchanged("define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 255\n  %z = zext i16 %m to i32\n  %w = zext i16 %m to i32\n  %r = add i32 %z, %w\n  ret i32 %r\n}\n");
}

/// A mask of a width the layout does not compute natively stays: the byte
/// narrowing was hard-coded, whatever the target's `n`.
#[test]
fn test_a_mask_narrows_only_to_a_native_width() {
    let text = "define i32 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 255\n  %r = zext i16 %m to i32\n  ret i32 %r\n}\n";
    let run = |layout: &str| {
        let mut module = parsed(text);
        module.datalayout = Some(layout.to_owned());
        let mut passes = llrm_mir::passes::PassManager::default();
        passes.add(super::Algebraic);
        passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
        printed(&module)
    };
    assert!(run("n8:16:32").contains("trunc i16 %x to i8"));
    assert!(!run("n16:32").contains("trunc"));
}

/// C's `unsigned short i; i < 8` on a 32-bit `int`: `slt (zext i), 8`. Left
/// wide, no trip count, unroll or count-to-zero saw the loop (code32 crc ran
/// 832 instructions to code16's 477).
#[test]
fn test_a_compare_of_an_extension_is_a_compare_of_its_source() {
    let wide = |cast: &str, test: &str, constant: i64| format!("define i1 @f(i16 %x) {{\nb0:\n  %w = {cast} i16 %x to i32\n  %r = icmp {test} i32 %w, {constant}\n  ret i1 %r\n}}\n");
    let inputs = singles(&edges(16));
    // A constant in the source's range: its width, unsigned for a zext.
    assert!(checked(&wide("zext", "slt", 8), &inputs).contains("icmp ult i16 %x, 8"));
    assert!(checked(&wide("zext", "ule", 65535), &inputs).contains("icmp ule i16 %x, -1"));
    assert!(checked(&wide("sext", "slt", -3), &inputs).contains("icmp slt i16 %x, -3"));
    assert!(checked(&wide("sext", "eq", 100), &inputs).contains("icmp eq i16 %x, 100"));
    // Outside it, the answer is decided.
    assert!(checked(&wide("zext", "ult", 65536), &inputs).contains("ret i1 true"));
    assert!(checked(&wide("zext", "slt", -1), &inputs).contains("ret i1 false"));
    assert!(checked(&wide("zext", "ne", 70000), &inputs).contains("ret i1 true"));
    assert!(checked(&wide("sext", "sgt", 40000), &inputs).contains("ret i1 false"));
    // Unsigned order of a `sext` against a constant stays wide.
    unchanged(&wide("sext", "ult", 8));
}

/// `i < limit`, both `unsigned short` (sieve): the pair compares narrow.
#[test]
fn test_a_compare_of_two_extensions_is_a_compare_of_their_sources() {
    for (cast, test, narrow) in [("zext", "slt", "ult"), ("zext", "uge", "uge"), ("sext", "slt", "slt"), ("sext", "ult", "ult")] {
        let text = format!("define i1 @f(i16 %x, i16 %y) {{\nb0:\n  %a = {cast} i16 %x to i32\n  %b = {cast} i16 %y to i32\n  %r = icmp {test} i32 %a, %b\n  ret i1 %r\n}}\n");
        let inputs = edges(16).iter().flat_map(|&a| edges(16).into_iter().map(move |b| vec![a, b])).collect::<Vec<_>>();
        assert!(checked(&text, &inputs).contains(&format!("icmp {narrow} i16 %x, %y")), "{cast} {test}");
    }
    unchanged("define i1 @f(i16 %x, i8 %y) {\nb0:\n  %a = zext i16 %x to i32\n  %b = zext i8 %y to i32\n  %r = icmp ult i32 %a, %b\n  ret i1 %r\n}\n");
}

/// `(unsigned short)(m + i)` computed in `int`: `trunc (add (zext m), i)`.
/// The conversion and the wide add stayed in sieve's inner loop, a
/// `mov`/`movzx` pair a trip beside code16's `add`.
#[test]
fn test_a_truncated_sum_with_an_extension_of_its_width_is_narrow() {
    for op in ["add", "sub", "mul", "and", "or", "xor"] {
        let text = format!("define i16 @f(i16 %m, i32 %i) {{\nb0:\n  %w = zext i16 %m to i32\n  %s = {op} i32 %w, %i\n  %r = trunc i32 %s to i16\n  ret i16 %r\n}}\n");
        let inputs = edges(16).iter().flat_map(|&a| [0, 5, -9, 70000].map(move |b| vec![a, b])).collect::<Vec<_>>();
        let after = checked(&text, &inputs);
        assert!(after.contains(&format!("{op} i16 %m")) && !after.contains("zext") && !after.contains(&format!("{op} i32")), "{op}\n{after}");
    }
    // Two uses of the wide sum keep it.
    unchanged("define i16 @f(i16 %m, i32 %i) {\nb0:\n  %w = zext i16 %m to i32\n  %s = add i32 %w, %i\n  %r = trunc i32 %s to i16\n  %t = trunc i32 %s to i8\n  %u = zext i8 %t to i16\n  %v = add i16 %r, %u\n  ret i16 %v\n}\n");
}

/// A byte compared where the extension lives on for an address (grep's
/// `c == '|'` before `class[c]`) stays wide: narrowed it grew grep by 24
/// bytes. A counter's compare narrows whatever else reads the extension.
#[test]
fn test_a_compare_narrows_for_a_loop_carried_source_or_a_dying_extension() {
    let text = |source: &str| {
        format!(
            "define i32 @f(i8 %x) {{\nb0:\n  br label %b1\n\nb1:\n  %p = phi i8 [ %x, %b0 ], [ %q, %b1 ]\n  %w = zext i8 {source} to i32\n  %c = icmp ult i32 %w, 100\n  %q = add i8 %p, 1\n  br i1 %c, label %b1, label %b2\n\nb2:\n  ret i32 %w\n}}\n"
        )
    };
    // The phi is carried round the loop: narrow, though `%w` lives on.
    assert!(bare(&simplified(&text("%p")).1).contains("icmp ult i8 %p, 100"));
    // An argument is not: the extension stays its compare's operand.
    assert!(bare(&simplified(&text("%x")).1).contains("icmp ult i32 %w, 100"));
}

/// `udiv` and `urem` by a power of two are a logical shift and a mask. Selection had them as `div`, with the
/// divisor in a register and EDX cleared: forty clocks on a 486 for one.
#[test]
fn test_unsigned_power_division_is_a_shift_and_a_mask() {
    for (width, divisor) in [(32_u32, 2_i128), (32, 8), (32, 65536), (16, 4), (16, 16384), (8, 2)] {
        for (op, shown) in [("udiv", "lshr"), ("urem", "and")] {
            let text = unary(width, &format!("  %r = {op} i{width} %x, {divisor}\n  ret i{width} %r\n"));
            let done = checked(&text, &singles(&edges(width)));
            assert!(done.contains(shown) && !done.contains(op), "{done}");
        }
    }
}

/// A dividend proved non-negative needs no bias: `sdiv` and `srem` of it by a power of two are a shift and a
/// mask. The bias was five instructions where one did.
#[test]
fn test_a_signed_division_of_a_non_negative_dividend_is_unsigned() {
    let ranged = |op: &str, divisor: &str| format!("define i32 @f(i32 range(i32 0, 100000) %x) {{\nb0:\n  %r = {op} i32 %x, {divisor}\n  ret i32 %r\n}}\n");
    let inputs: Vec<Vec<i128>> = [0, 1, 7, 8, 9, 10, 99, 100, 99999].iter().map(|&one| vec![one]).collect();
    for (op, divisor, shown) in [("sdiv", "8", "lshr"), ("srem", "8", "and"), ("sdiv", "1024", "lshr")] {
        let done = checked(&ranged(op, divisor), &inputs);
        assert!(done.contains(shown) && !done.contains(op), "{done}");
    }
}

/// What a branch proves reaches the division: `rest > 0` holds in the loop that divides `rest` by ten.
#[test]
fn test_a_branch_that_proves_a_dividend_positive_makes_its_division_unsigned() {
    let text = "define i32 @f(i32 %x) {
b0:
  %c = icmp sgt i32 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %q = sdiv i32 %x, 10
  ret i32 %q

b2:
  ret i32 0
}
";
    let done = checked(&text.replace("sdiv i32 %x, 10", "sdiv i32 %x, 16"), &singles(&edges(32)));
    assert!(done.contains("lshr") && !done.contains("sdiv"), "{done}");
}

/// Not where the dividend may be negative, or the divisor is not a positive constant.
#[test]
fn test_a_signed_division_of_an_unknown_sign_stays_signed() {
    unchanged(&unary(32, "  %r = sdiv i32 %x, 16\n  ret i32 %r\n").replace("sdiv i32 %x, 16", "sdiv i32 %x, 10"));
    unchanged("define i32 @f(i32 range(i32 0, 100) %x) {\nb0:\n  %r = sdiv i32 %x, 10\n  ret i32 %r\n}\n");
    unchanged("define i32 @f(i32 range(i32 0, 100) %x) {\nb0:\n  %r = sdiv i32 %x, -10\n  ret i32 %r\n}\n");
    unchanged("define i32 @f(i32 range(i32 0, 100) %x, i32 %d) {\nb0:\n  %r = sdiv i32 %x, %d\n  ret i32 %r\n}\n");
}
