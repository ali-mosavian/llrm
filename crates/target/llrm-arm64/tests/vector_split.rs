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
