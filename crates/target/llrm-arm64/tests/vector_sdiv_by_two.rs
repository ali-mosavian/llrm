//! `sdiv <2 x i64> %x, 2` took `cmlt; usra #63; sshr #1`: the fold of
//! `srl (sra x, 63), 63` to `srl x, 63` compared the amount with the width of
//! the whole vector (128) and a scalar constant, so a splat never matched. The
//! pin emits `usra #63; sshr #1`; stft_fixed -Os pays the extra `cmlt` per
//! butterfly lane pair.

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

#[test]
fn a_vector_halving_is_one_usra_and_one_sshr() {
    let text = assembly(
        "define void @f(ptr %p, ptr %q) {\n\
         \x20 %x = load <2 x i64>, ptr %p, align 16\n\
         \x20 %d = sdiv <2 x i64> %x, <i64 2, i64 2>\n\
         \x20 store <2 x i64> %d, ptr %q, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(!text.contains("cmlt"), "{text}");
    assert_eq!(text.matches("usra").count(), 1, "{text}");
    assert_eq!(text.matches("sshr").count(), 1, "{text}");
}
