//! A shuffle whose mask is a zip, unzip or transpose of its operands is
//! `zip1/2`, `uzp1/2` or `trn1/2` (`LowerVECTOR_SHUFFLE`, `isZIPMask`,
//! `isUZPMask`, `isTRNMask`). The mask was declined, so a `<16 x i8>`
//! interleave made `textfill -O2` run 32009 instructions to the pin's 4120.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn assembly(
    ty: &str,
    mask: &[i32],
    second: &str,
) -> String {
    let mask: Vec<String> =
        mask.iter().map(|lane| if *lane < 0 { "i32 poison".into() } else { format!("i32 {lane}") }).collect();
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p, ptr %q, ptr %o) {{
  %x = load {ty}, ptr %p, align 16
  %y = load {ty}, ptr %q, align 16
  %t = shufflevector {ty} %x, {ty} {second_arg}, <{n} x i32> <{mask}>
  store <{n} x i8> %t, ptr %o, align 16
  ret void
}}
"#,
        n = mask.len(),
        mask = mask.join(", "),
        second_arg = if second == "undef" { "poison" } else { "%y" },
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

fn zip(
    second: bool,
    swapped: bool,
) -> Vec<i32> {
    (0..8)
        .flat_map(|at| {
            let (even, odd) = (at + if second { 8 } else { 0 }, at + 16 + if second { 8 } else { 0 });
            if swapped { [odd, even] } else { [even, odd] }
        })
        .take(16)
        .collect()
}

/// The interleave of two byte vectors: `zip1`, `zip2`, and with the
/// operands swapped.
#[test]
fn a_zip_mask_is_zip1_or_zip2() {
    let asm = assembly("<16 x i8>", &zip(false, false), "%y");
    assert!(asm.contains("zip1\tv") || asm.contains("zip1 v"), "{asm}");
    let asm = assembly("<16 x i8>", &zip(true, false), "%y");
    assert!(asm.contains("zip2"), "{asm}");
}

/// The even or odd bytes of two vectors.
#[test]
fn an_unzip_mask_is_uzp1_or_uzp2() {
    let even: Vec<i32> = (0..16).map(|at| at * 2).collect();
    let odd: Vec<i32> = (0..16).map(|at| at * 2 + 1).collect();
    assert!(assembly("<16 x i8>", &even, "%y").contains("uzp1"));
    assert!(assembly("<16 x i8>", &odd, "%y").contains("uzp2"));
}

#[test]
fn a_transpose_mask_is_trn1_or_trn2() {
    let one: Vec<i32> = (0..8).flat_map(|at| [at * 2, 16 + at * 2]).collect();
    let two: Vec<i32> = (0..8).flat_map(|at| [at * 2 + 1, 16 + at * 2 + 1]).collect();
    assert!(assembly("<16 x i8>", &one, "%y").contains("trn1"));
    assert!(assembly("<16 x i8>", &two, "%y").contains("trn2"));
}

/// `<0, 0, 1, 1, ...>` of one vector: the `_v_undef_` form zips it with itself.
#[test]
fn a_one_input_zip_mask_zips_the_vector_with_itself() {
    let mask: Vec<i32> = (0..8).flat_map(|at| [at, at]).collect();
    let asm = assembly("<16 x i8>", &mask, "undef");
    assert!(asm.contains("zip1"), "{asm}");
}
