//! A vector `sdiv`/`udiv`/`srem`/`urem` was refused (`CannotSelect("SDiv")`):
//! the vector legalizer unrolled add, sub, mul and the shifts only. The pin's
//! `VectorLegalizer::Expand` does a lane at a time any operation the target
//! has no vector form of; SLP's reduction tree makes `sdiv <8 x i64>`.

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

/// `sdiv <4 x i64>` by a divisor in memory: four scalar `sdiv`s.
#[test]
fn a_vector_divide_is_four_scalar_divides() {
    for (op, mnemonic) in [("sdiv", "sdiv"), ("udiv", "udiv")] {
        let text = assembly(&format!(
            "define void @f(ptr %a, ptr %b, ptr %c) {{\n\
             \x20 %x = load <4 x i64>, ptr %a, align 8\n\
             \x20 %y = load <4 x i64>, ptr %b, align 8\n\
             \x20 %z = {op} <4 x i64> %x, %y\n\
             \x20 store <4 x i64> %z, ptr %c, align 8\n\
             \x20 ret void\n}}\n"
        ));
        let divides = text.lines().filter(|line| line.trim().starts_with(&format!("{mnemonic} "))).count();
        assert_eq!(divides, 4, "{op}: {text}");
    }
}

/// The remainder of a vector is four scalar remainders (a divide, a multiply
/// and a subtract each).
#[test]
fn a_vector_remainder_is_four_scalar_remainders() {
    let text = assembly(
        "define void @f(ptr %a, ptr %b, ptr %c) {\n\
         \x20 %x = load <4 x i64>, ptr %a, align 8\n\
         \x20 %y = load <4 x i64>, ptr %b, align 8\n\
         \x20 %z = srem <4 x i64> %x, %y\n\
         \x20 store <4 x i64> %z, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    let divides = text.lines().filter(|line| line.trim().starts_with("sdiv ")).count();
    assert_eq!(divides, 4, "{text}");
}

/// jpeg_fixed at O2: `sdiv <8 x i64>` by a power of two per lane (512, 1024)
/// was `CannotSelect("Select Simple(I64)")`. Each lane met the compare-and-
/// select form of `sdiv x, 2^k` after legalization, where `Select` is Custom.
#[test]
fn a_vector_divide_by_different_powers_of_two_is_compiled() {
    let text = assembly(
        "define void @f(ptr %a, ptr %c) {\n\
         \x20 %x = load <2 x i64>, ptr %a, align 8\n\
         \x20 %z = sdiv <2 x i64> %x, <i64 512, i64 1024>\n\
         \x20 store <2 x i64> %z, ptr %c, align 8\n\
         \x20 ret void\n}\n",
    );
    assert!(!text.contains("sdiv "), "{text}");
}
