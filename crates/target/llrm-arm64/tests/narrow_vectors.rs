//! A vector under a D register (`<4 x i8>`, `<2 x i16>`) has no register
//! class: the legalizer promotes its lanes (`getPreferredVectorAction`,
//! `<4 x i8>` to `<4 x i16>`). Without it `load <4 x i8>` stopped the compile
//! ("vector with no register class"), and the loop vectorizer could not take
//! an epilogue of 4 byte lanes as the pin does.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn compile(function: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{function}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// Four byte lanes compared with zero and counted in 64-bit lanes (the epilogue
/// of a count loop).
#[test]
fn four_byte_lanes_compared_and_widened_compile() {
    let text = compile(
        "define void @f(ptr %p, ptr %q) {\n  %x = load <4 x i8>, ptr %p, align 1\n  \
         %m = icmp eq <4 x i8> %x, zeroinitializer\n  %z = zext <4 x i1> %m to <4 x i64>\n  \
         store <4 x i64> %z, ptr %q, align 8\n  ret void\n}\n",
    )
    .unwrap();
    assert!(text.contains("ldr s"), "{text}");
    assert!(text.contains("cmeq"), "{text}");
}

/// `v4i8` is a scalar load and a widening (`ldr s`, `ushll`), as the pin.
#[test]
fn four_byte_lanes_extended_are_one_load_and_a_widening() {
    let text = compile(
        "define void @f(ptr %p, ptr %q) {\n  %x = load <4 x i8>, ptr %p, align 1\n  \
         %z = zext <4 x i8> %x to <4 x i32>\n  store <4 x i32> %z, ptr %q, align 16\n  ret void\n}\n",
    )
    .unwrap();
    assert!(text.contains("ldr s"), "{text}");
    assert!(text.contains("ushll"), "{text}");
}

/// `v4i8` is stored by narrowing `v4i16` (`uzp1`) and a word store, as the pin.
#[test]
fn four_byte_lanes_added_are_stored_as_one_word() {
    let text = compile(
        "define void @f(ptr %p, ptr %q) {\n  %x = load <4 x i8>, ptr %p, align 1\n  \
         %y = add <4 x i8> %x, <i8 1, i8 2, i8 3, i8 4>\n  store <4 x i8> %y, ptr %q, align 1\n  ret void\n}\n",
    )
    .unwrap();
    assert!(text.contains("uzp1"), "{text}");
    assert!(text.contains("str s"), "{text}");
}

/// A mask of four byte lanes stored as bytes.
#[test]
fn four_byte_lanes_compared_and_sign_extended_compile() {
    let text = compile(
        "define void @f(ptr %p, ptr %q) {\n  %x = load <4 x i8>, ptr %p, align 1\n  \
         %m = icmp eq <4 x i8> %x, zeroinitializer\n  %s = sext <4 x i1> %m to <4 x i8>\n  \
         store <4 x i8> %s, ptr %q, align 1\n  ret void\n}\n",
    )
    .unwrap();
    assert!(text.contains("cmeq"), "{text}");
}
