//! A 128-bit vector whose low half is a value and whose high half is zero is
//! one `fmov d, x` (or the `dup`/`ldr` of 64 bits, which clear the upper half),
//! not an `fmov` and an `ins` of zero per lane: tile -O2 +2 instructions a row;
//! a shuffle of a vector with zeros was refused by llc.

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

fn instructions(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with(['.', '/', '$'])).collect()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let all = instructions(text);
    let start = all.iter().position(|line| line.trim_end_matches(':') == name).expect("the function");
    all[start + 1..].iter().copied().take_while(|line| !line.ends_with(':')).collect()
}

/// `<2 x i64> {x, 0}` was `fmov d, x; ins v.d[1], xzr`.
#[test]
fn an_i64_in_the_low_lane_and_zero_above_is_one_fmov() {
    let text = assembly(
        "define void @a(i64 %x, ptr %p) {\n  %v = insertelement <2 x i64> zeroinitializer, i64 %x, i64 0\n  \
store <2 x i64> %v, ptr %p, align 16\n  ret void\n}\n",
    );
    let code = body(&text, "a");
    assert_eq!(code.iter().filter(|line| line.starts_with("fmov d")).count(), 1, "{text}");
    assert!(code.iter().all(|line| !line.starts_with("ins ")), "{text}");
}

/// `<4 x i32> {x, x, 0, 0}` was an `fmov s`, an `ins` and two `ins` of `wzr`.
#[test]
fn a_repeated_low_half_and_zero_above_is_one_dup() {
    let text = assembly(
        "define void @b(i32 %x, ptr %p) {\n  %v0 = insertelement <4 x i32> zeroinitializer, i32 %x, i64 0\n  \
%v = insertelement <4 x i32> %v0, i32 %x, i64 1\n  store <4 x i32> %v, ptr %p, align 16\n  ret void\n}\n",
    );
    let code = body(&text, "b");
    assert_eq!(code.len(), 3, "dup, str, ret: {text}");
    assert!(code[0].starts_with("dup"), "{text}");
}

/// A shuffle of a loaded `<2 x i32>` with zeros was refused by llc (`STRQui: a
/// store of something that is no register`); the load clears the upper half.
#[test]
fn a_half_vector_concatenated_with_zeros_is_its_load() {
    let text = assembly(
        "define void @d(ptr %a, ptr %p) {\n  %x = load <2 x i32>, ptr %a, align 8\n  \
%v = shufflevector <2 x i32> %x, <2 x i32> zeroinitializer, <4 x i32> <i32 0, i32 1, i32 2, i32 3>\n  \
store <4 x i32> %v, ptr %p, align 16\n  ret void\n}\n",
    );
    let code = body(&text, "d");
    assert_eq!(code.len(), 3, "ldr, str, ret: {text}");
    assert!(code[0].starts_with("ldr d"), "{text}");
}

/// tile -O2: two inserts into one shared zero vector were a `mov` of the zero
/// register and an `ins` each (`aggressivelyPreferBuildVectorSources`: the
/// combine rebuilds from a `BUILD_VECTOR` with other users too).
#[test]
fn two_inserts_into_one_zero_vector_are_one_fmov_each() {
    let text = assembly(
        "define void @e(i64 %x, i64 %y, ptr %p, ptr %q) {\n  %a = insertelement <2 x i64> zeroinitializer, i64 %x, i64 0\n  \
%b = insertelement <2 x i64> zeroinitializer, i64 %y, i64 0\n  store <2 x i64> %a, ptr %p, align 16\n  \
store <2 x i64> %b, ptr %q, align 16\n  ret void\n}\n",
    );
    let code = body(&text, "e");
    assert_eq!(code.iter().filter(|line| line.starts_with("fmov d")).count(), 2, "{text}");
    assert!(code.iter().all(|line| !line.starts_with("ins ") && !line.starts_with("mov v")), "{text}");
}
