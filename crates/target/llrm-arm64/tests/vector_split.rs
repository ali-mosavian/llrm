//! A vector no register holds (`<4 x i64>`, `<8 x float>`) was refused ("vector
//! with no register class"): the type legalizer had no `TypeSplitVector`
//! (`SplitVectorResult`, `SplitVectorOperand`), so a loop widened to 256 bits
//! or a vector built from scalars could not be compiled.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn count(
    text: &str,
    mnemonic: &str,
    suffix: &str,
) -> usize {
    text.lines()
        .filter(|line| {
            line.trim().strip_prefix(mnemonic).is_some_and(|rest| rest.starts_with(' ') && rest.contains(suffix))
        })
        .count()
}

/// Load, add and store of `<4 x i64>`: two loads, two adds, two stores.
#[test]
fn a_256_bit_integer_vector_is_two_registers() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, ptr %b) {\n\
         \x20 %x = load <4 x i64>, ptr %a, align 32\n\
         \x20 %y = load <4 x i64>, ptr %b, align 32\n\
         \x20 %z = add <4 x i64> %x, %y\n\
         \x20 store <4 x i64> %z, ptr %d, align 32\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "add", ".2d"), 2, "{text}");
    assert!(text.contains("ldp q") && text.contains("stp q"), "{text}");
}

/// `<8 x float>` arithmetic: two `fmul v.4s`.
#[test]
fn a_256_bit_float_vector_is_two_registers() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, ptr %b) {\n\
         \x20 %x = load <8 x float>, ptr %a, align 32\n\
         \x20 %y = load <8 x float>, ptr %b, align 32\n\
         \x20 %z = fmul <8 x float> %x, %y\n\
         \x20 store <8 x float> %z, ptr %d, align 32\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "fmul", ".4s"), 2, "{text}");
}

/// Four scalars put together (the lanes of a scalarized load) and converted:
/// each half is `fmov` and `ins`, then one `scvtf v.2d` each.
#[test]
fn a_vector_built_from_scalars_is_split_and_converted() {
    let text = assembly(
        "define void @f(ptr %d, i64 %p, i64 %q, i64 %r, i64 %s) {\n\
         \x20 %v0 = insertelement <4 x i64> poison, i64 %p, i64 0\n\
         \x20 %v1 = insertelement <4 x i64> %v0, i64 %q, i64 1\n\
         \x20 %v2 = insertelement <4 x i64> %v1, i64 %r, i64 2\n\
         \x20 %v3 = insertelement <4 x i64> %v2, i64 %s, i64 3\n\
         \x20 %f = sitofp <4 x i64> %v3 to <4 x double>\n\
         \x20 store <4 x double> %f, ptr %d, align 32\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "scvtf", ".2d"), 2, "{text}");
    assert_eq!(count(&text, "ins", ".d["), 2, "{text}");
}

/// `sitofp <4 x i64> to <4 x float>` was refused (`CannotSelect("SIntToFp")`,
/// then no `UMOV`): the pin's `LowerVectorINT_TO_FP` unrolls a narrowing
/// conversion to `f32`, so it is four scalar `scvtf` and lane inserts.
#[test]
fn a_narrowing_vector_int_to_float_conversion_is_unrolled() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x i64>, ptr %a, align 32\n\
         \x20 %f = sitofp <4 x i64> %x to <4 x float>\n\
         \x20 store <4 x float> %f, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "scvtf", " s"), 4, "{text}");
    assert_eq!(count(&text, "ins", ".s["), 2, "{text}");
    assert_eq!(count(&text, "ins", ".d["), 1, "{text}");
    // the lanes are loaded as scalars (`scalarizeExtractedVectorLoad`), not
    // taken out of a vector register with `umov`/`fmov`
    assert_eq!(count(&text, "ldp", " x"), 2, "{text}");
    assert_eq!(count(&text, "umov", " x"), 0, "{text}");
    assert_eq!(count(&text, "fmov", " x"), 0, "{text}");
}

/// A lane of a vector built from scalars is the scalar
/// (`SelectionDAG::getNode` of `EXTRACT_VECTOR_ELT`): the lanes of the `i64`
/// vector were put into registers with `fmov`/`ins` only to be taken out again.
#[test]
fn a_lane_of_a_vector_built_from_scalars_is_the_scalar() {
    let text = assembly(
        "define void @f(ptr %d, i64 %p, i64 %q, i64 %r, i64 %s) {\n\
         \x20 %v0 = insertelement <4 x i64> poison, i64 %p, i64 0\n\
         \x20 %v1 = insertelement <4 x i64> %v0, i64 %q, i64 1\n\
         \x20 %v2 = insertelement <4 x i64> %v1, i64 %r, i64 2\n\
         \x20 %v3 = insertelement <4 x i64> %v2, i64 %s, i64 3\n\
         \x20 %f = sitofp <4 x i64> %v3 to <4 x float>\n\
         \x20 store <4 x float> %f, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "scvtf", " s"), 4, "{text}");
    assert_eq!(count(&text, "umov", " x"), 0, "{text}");
    assert_eq!(count(&text, "fmov", " d"), 0, "{text}");
}

/// A lane of a float vector stored as a scalar was refused (`instruction
/// extractelement`; then `ST1i32` had no instruction): lane 0 is `str s`, the
/// others `st1 { v.s }[n]`.
#[test]
fn a_stored_lane_of_a_vector_is_st1() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %y = fadd <4 x float> %x, %x\n\
         \x20 %p = extractelement <4 x float> %y, i64 2\n\
         \x20 store float %p, ptr %d, align 4\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "st1", "}[2], ["), 1, "{text}");
}

/// A vector of float zeros was `CannotSelect("BuildVector")`: the all-zeros
/// predicate read only integer lanes.
#[test]
fn a_float_vector_of_zeros_is_a_movi() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %y = fadd <4 x float> %x, zeroinitializer\n\
         \x20 store <4 x float> %y, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(text.contains("movi.2d"), "{text}");
}

/// `fmuladd` of a splatted scalar was a vector of copies and a plain `fmla`;
/// the multiplier is a lane (`FMLAv4i32_indexed`).
#[test]
fn a_multiply_add_by_a_splat_is_fmla_by_element() {
    let text = assembly(
        "declare <4 x float> @llvm.fmuladd.v4f32(<4 x float>, <4 x float>, <4 x float>)\n\
         define void @f(ptr %d, ptr %a, ptr %b, float %c) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %z = load <4 x float>, ptr %b, align 16\n\
         \x20 %i = insertelement <4 x float> poison, float %c, i64 0\n\
         \x20 %s = shufflevector <4 x float> %i, <4 x float> poison, <4 x i32> zeroinitializer\n\
         \x20 %m = call <4 x float> @llvm.fmuladd.v4f32(<4 x float> %x, <4 x float> %s, <4 x float> %z)\n\
         \x20 store <4 x float> %m, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "fmla", ".s[0]"), 1, "{text}");
}

/// A float vector multiplied by a splatted scalar selected `FMULv4i32_indexed`,
/// which had no MC form: `(float)a[i] * 2.5f` loops, compiled before the
/// narrowing convert was widened, were refused ("has no arm64 instruction").
#[test]
fn a_multiply_by_a_splat_is_fmul_by_element() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, float %c) {\n\
         \x20 %x = load <4 x float>, ptr %a, align 16\n\
         \x20 %i = insertelement <4 x float> poison, float %c, i64 0\n\
         \x20 %s = shufflevector <4 x float> %i, <4 x float> poison, <4 x i32> zeroinitializer\n\
         \x20 %m = fmul <4 x float> %x, %s\n\
         \x20 store <4 x float> %m, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "fmul", ".s[0]"), 1, "{text}");
}

/// `<4 x i16>` had no register class ("vector with no register class"): the
/// 64-bit integer vectors are `FPR64` (`addDRTypeForNEON`), loaded with `ldr
/// d`.
#[test]
fn a_zero_extended_64_bit_vector_is_ushll() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x i16>, ptr %a, align 8\n\
         \x20 %e = zext <4 x i16> %x to <4 x i32>\n\
         \x20 store <4 x i32> %e, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "ldr", " d"), 1, "{text}");
    assert_eq!(count(&text, "ushll", ".4s, v"), 1, "{text}");
}

/// `trunc <4 x i32> to <4 x i16>` had no instruction (`XTNv4i16`).
#[test]
fn a_truncated_vector_is_xtn() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <4 x i32>, ptr %a, align 16\n\
         \x20 %t = trunc <4 x i32> %x to <4 x i16>\n\
         \x20 store <4 x i16> %t, ptr %d, align 8\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "xtn", ".4h, v"), 1, "{text}");
    assert_eq!(count(&text, "str", " d"), 1, "{text}");
}

/// `sext <8 x i16> to <8 x i64>` was refused ("split result"): the pin splits
/// it in halves, extends in two steps (`SplitVecRes_ExtendOp`) and takes the
/// high half with `sshll2` (`SelectExtractHigh`), not a `mov d, v.d[1]`.
#[test]
fn a_sign_extension_to_four_times_the_lane_is_a_chain_of_sshll() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <8 x i16>, ptr %a, align 16\n\
         \x20 %e = sext <8 x i16> %x to <8 x i64>\n\
         \x20 store <8 x i64> %e, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "sshll2", ""), 3, "{text}");
    assert_eq!(count(&text, "sshll", ""), 3, "{text}");
    assert_eq!(count(&text, "mov", " d"), 0, "{text}");
}

/// A lane read as an integer (`scvtf s, x`) from a vector in a register is
/// copied across the register files (`fmov x, d`): `SCVTFUXSri` took the `v`
/// register as its operand ("operands are not those of its form").
#[test]
fn a_vector_lane_converted_as_an_integer_is_moved_to_a_general_register() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <2 x i64>, ptr %a, align 16\n\
         \x20 %y = add <2 x i64> %x, %x\n\
         \x20 %p = extractelement <2 x i64> %y, i64 0\n\
         \x20 %f = sitofp i64 %p to float\n\
         \x20 store float %f, ptr %d, align 4\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "fmov", " x"), 1, "{text}");
    assert_eq!(count(&text, "scvtf", " s"), 1, "{text}");
}

/// `mul <2 x i64>` of two extensions was refused (`CannotSelect("Mul")`: NEON
/// has no 64-bit lane multiply); the pin's `LowerMUL` finds `SMULL`/`UMULL`.
#[test]
fn a_multiply_of_two_extensions_is_one_long_multiply() {
    let text = assembly(
        "define void @f(ptr %a, ptr %b, ptr %c) {\n\
         \x20 %l1 = load <2 x i32>, ptr %a\n\
         \x20 %l2 = load <2 x i32>, ptr %b\n\
         \x20 %e1 = sext <2 x i32> %l1 to <2 x i64>\n\
         \x20 %e2 = sext <2 x i32> %l2 to <2 x i64>\n\
         \x20 %m = mul <2 x i64> %e1, %e2\n\
         \x20 store <2 x i64> %m, ptr %c, align 8\n\
         \x20 ret void\n}\n\
         define void @g(ptr %a, ptr %b, ptr %c) {\n\
         \x20 %l1 = load <2 x i32>, ptr %a\n\
         \x20 %l2 = load <2 x i32>, ptr %b\n\
         \x20 %e1 = zext <2 x i32> %l1 to <2 x i64>\n\
         \x20 %e2 = zext <2 x i32> %l2 to <2 x i64>\n\
         \x20 %m = mul <2 x i64> %e1, %e2\n\
         \x20 %d = load <2 x i64>, ptr %c, align 8\n\
         \x20 %r = add <2 x i64> %d, %m\n\
         \x20 store <2 x i64> %r, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "smull", ".2d"), 1, "{text}");
    assert_eq!(count(&text, "umlal", ".2d"), 1, "{text}");
    assert_eq!(count(&text, "umull", ""), 0, "the add folds into the accumulate: {text}");
}

/// The matmul inner loop of the pin: `sext <8 x i16>` times the splat of a
/// scalar `sext`, summed. The splat was a `v2i64` `dup` and an `xtn` per
/// use, the high halves a `mov d` each; the pin has one `dup v.4s` and
/// `smull2`/`smlal2` of the high halves.
#[test]
fn a_multiply_of_an_extension_by_a_splat_uses_the_high_half_forms() {
    let text = assembly(
        "define void @f(ptr %a, ptr %c, i16 %x, i16 %y) {\n\
         \x20 %xs = sext i16 %x to i64\n\
         \x20 %ys = sext i16 %y to i64\n\
         \x20 %i1 = insertelement <8 x i64> poison, i64 %xs, i64 0\n\
         \x20 %s1 = shufflevector <8 x i64> %i1, <8 x i64> poison, <8 x i32> zeroinitializer\n\
         \x20 %i2 = insertelement <8 x i64> poison, i64 %ys, i64 0\n\
         \x20 %s2 = shufflevector <8 x i64> %i2, <8 x i64> poison, <8 x i32> zeroinitializer\n\
         \x20 %l1 = load <8 x i16>, ptr %a, align 2\n\
         \x20 %e1 = sext <8 x i16> %l1 to <8 x i64>\n\
         \x20 %m1 = mul nsw <8 x i64> %e1, %s1\n\
         \x20 %p2 = getelementptr inbounds nuw i8, ptr %a, i64 16\n\
         \x20 %l2 = load <8 x i16>, ptr %p2, align 2\n\
         \x20 %e2 = sext <8 x i16> %l2 to <8 x i64>\n\
         \x20 %m2 = mul nsw <8 x i64> %e2, %s2\n\
         \x20 %r = add nsw <8 x i64> %m2, %m1\n\
         \x20 store <8 x i64> %r, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "smull2", ".2d"), 2, "{text}");
    assert_eq!(count(&text, "smlal2", ".2d"), 2, "{text}");
    assert_eq!(count(&text, "smull", ".2d"), 2, "{text}");
    assert_eq!(count(&text, "smlal", ".2d"), 2, "{text}");
    assert_eq!(text.matches("dup.4s").count(), 2, "{text}");
    assert_eq!(count(&text, "xtn", ""), 0, "{text}");
    assert_eq!(count(&text, "mov", " d"), 0, "{text}");
}

/// `mul <2 x i64>` of two loaded vectors was refused ("custom lowering of
/// Mul"; matmul did not compile): `LowerMUL` declines a product that is no
/// long multiply and the pin expands it, one scalar `mul` per lane.
#[test]
fn a_plain_two_lane_64_bit_multiply_is_one_scalar_multiply_per_lane() {
    let text = assembly(
        "define void @f(ptr %a, ptr %b, ptr %c) {\n\
         \x20 %x = load <2 x i64>, ptr %a, align 8\n\
         \x20 %y = load <2 x i64>, ptr %b, align 8\n\
         \x20 %m = mul <2 x i64> %x, %y\n\
         \x20 store <2 x i64> %m, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(
        text.lines().filter(|l| l.trim_start().starts_with("mul ") || l.trim_start().starts_with("madd ")).count(),
        2,
        "{text}"
    );
}

/// The extension and the splat of a multiply were made in the block before it
/// (matmul's loop-invariant operands) and crossed as `v2i64` registers: the
/// product was eight scalar `madd`s (matmul O2 3575 instructions). The pin's
/// `isProfitableToSinkOperands` makes both again beside the multiply, which is
/// then `smull`/`smlal` of the halves.
#[test]
fn an_extension_and_a_splat_made_in_another_block_are_sunk_to_the_multiply() {
    let text = assembly(
        "define void @f(ptr %a, ptr %c, i16 %x) {\n\
         entry:\n\
         \x20 %xs = sext i16 %x to i64\n\
         \x20 %i = insertelement <8 x i64> poison, i64 %xs, i64 0\n\
         \x20 %s = shufflevector <8 x i64> %i, <8 x i64> poison, <8 x i32> zeroinitializer\n\
         \x20 br label %body\n\
         body:\n\
         \x20 %n = phi i64 [ 0, %entry ], [ %n1, %body ]\n\
         \x20 %p = getelementptr inbounds <8 x i64>, ptr %c, i64 %n\n\
         \x20 %q = getelementptr inbounds <8 x i16>, ptr %a, i64 %n\n\
         \x20 %l2 = load <8 x i16>, ptr %q, align 2\n\
         \x20 %e2 = sext <8 x i16> %l2 to <8 x i64>\n\
         \x20 %m = mul nsw <8 x i64> %e2, %s\n\
         \x20 store <8 x i64> %m, ptr %p, align 8\n\
         \x20 %n1 = add nuw nsw i64 %n, 1\n\
         \x20 %done = icmp eq i64 %n1, 8\n\
         \x20 br i1 %done, label %exit, label %body\n\
         exit:\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "smull2", ".2d"), 2, "{text}");
    assert_eq!(count(&text, "smull", ".2d"), 2, "{text}");
    assert_eq!(text.lines().filter(|l| l.trim_start().starts_with("madd ")).count(), 0, "{text}");
}

/// A `v8i64` product of two `sext <8 x i16>` was split into `v2i64` pieces and
/// scalarized (eight `madd`s). The pin's `performVectorExtCombine` multiplies
/// at `v8i32` and extends: two `smull .4s`, four `sshll .2d`.
#[test]
fn a_wide_multiply_of_two_extensions_is_done_at_half_the_width_and_extended() {
    let text = assembly(
        "define void @f(ptr %a, ptr %b, ptr %c) {\n\
         \x20 %la = load <8 x i16>, ptr %a, align 2\n\
         \x20 %lb = load <8 x i16>, ptr %b, align 2\n\
         \x20 %ea = sext <8 x i16> %la to <8 x i64>\n\
         \x20 %eb = sext <8 x i16> %lb to <8 x i64>\n\
         \x20 %m = mul nsw <8 x i64> %ea, %eb\n\
         \x20 store <8 x i64> %m, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "smull", ".4s") + count(&text, "smull2", ".4s"), 2, "{text}");
    assert_eq!(text.lines().filter(|l| l.trim_start().starts_with("madd ")).count(), 0, "{text}");
}

/// LoopVectorize refused any loop with a `sext` ("a loaded value used other
/// than by a store or a binary operation"): `c[i] = (long)a[i] * x` over 64
/// elements stayed scalar. The pin widens the cast (VF 8, 13 against 7 a
/// lane at VF 1) and the product is `smull`/`smull2` of the halves.
#[test]
fn a_loop_of_widening_multiplies_by_a_scalar_is_vectorized() {
    let text = assembly(
        "define void @f(ptr noalias %a, ptr noalias %c, i16 %x16) {\n\
         entry:\n\
         \x20 %x = sext i16 %x16 to i64\n\
         \x20 br label %loop\n\
         loop:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %n, %loop ]\n\
         \x20 %p = getelementptr inbounds i16, ptr %a, i64 %i\n\
         \x20 %v = load i16, ptr %p, align 2\n\
         \x20 %e = sext i16 %v to i64\n\
         \x20 %m = mul nsw i64 %e, %x\n\
         \x20 %q = getelementptr inbounds i64, ptr %c, i64 %i\n\
         \x20 store i64 %m, ptr %q, align 8\n\
         \x20 %n = add nuw nsw i64 %i, 1\n\
         \x20 %d = icmp eq i64 %n, 64\n\
         \x20 br i1 %d, label %exit, label %loop\n\
         exit:\n\
         \x20 ret void\n}\n",
    );
    assert!(count(&text, "smull", ".2d") + count(&text, "smull2", ".2d") >= 2, "{text}");
    assert_eq!(
        text.lines().filter(|l| l.trim_start().starts_with("madd ") || l.trim_start().starts_with("mul ")).count(),
        0,
        "{text}"
    );
}
