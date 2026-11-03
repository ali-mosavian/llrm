//! Leading and trailing zero counts that may assume a non-zero argument, and
//! the counts of `i8` and `i16`, through the DAG path alone (`--isel=dag`): the
//! legalizer had no expansion for `CTLZ_ZERO_POISON` / `CTTZ_ZERO_POISON` and
//! no promotion for a narrow count, so `stft_fixed` and `stft_single` were
//! refused ("convert node to libcall", "bitreverse of i8" from the hand
//! selector the refusal fell back to).

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// The assembly of `function` (with `declare`s as needed) at -O2.
fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn count(
    name: &str,
    bits: u32,
    poison: bool,
) -> String {
    assembly(&format!(
        "declare i{bits} @llvm.{name}.i{bits}(i{bits}, i1)\n\
         define i{bits} @f(i{bits} %a) {{\n %r = call i{bits} @llvm.{name}.i{bits}(i{bits} %a, i1 {poison})\n ret i{bits} %r\n}}\n"
    ))
}

#[test]
fn a_leading_zero_count_that_assumes_non_zero_is_clz() {
    assert!(count("ctlz", 32, true).contains("clz w0, w0"));
    assert!(count("ctlz", 64, true).contains("clz x0, x0"));
}

#[test]
fn a_trailing_zero_count_that_assumes_non_zero_is_rbit_and_clz() {
    for bits in [32, 64] {
        let text = count("cttz", bits, true);
        assert!(text.contains("rbit") && text.contains("clz"), "{text}");
    }
}

#[test]
fn counts_of_narrow_integers_are_made_in_the_wider_type() {
    for name in ["ctlz", "cttz"] {
        for bits in [8, 16] {
            for poison in [false, true] {
                let text = count(name, bits, poison);
                assert!(text.contains("clz"), "{name} i{bits} {poison}: {text}");
            }
        }
    }
}

#[test]
fn the_bit_reverse_of_an_i8_is_rbit_and_a_shift() {
    let text = assembly(
        "declare i8 @llvm.bitreverse.i8(i8)\ndefine i8 @f(i8 %a) {\n %r = call i8 @llvm.bitreverse.i8(i8 %a)\n ret i8 %r\n}\n",
    );
    assert!(text.contains("rbit"), "{text}");
}
