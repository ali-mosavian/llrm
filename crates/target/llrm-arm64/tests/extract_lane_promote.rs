//! A lane read out of a byte vector and used as an integer stopped the compile
//! (`jpeg_single` at -O3: `Unsupported ExtractElement i8 "promote result"`): the
//! legalizer had no promotion for an `extract_element` whose result type
//! (`i8`) is illegal.

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

/// A byte lane extended to an `i32`: one `umov`.
#[test]
fn a_byte_lane_extended_to_an_int_is_a_umov() {
    let text = compile(
        "define i32 @f(ptr %p) {\n  %v = load <16 x i8>, ptr %p, align 16\n  %e = extractelement <16 x i8> %v, i64 3\n  \
         %z = zext i8 %e to i32\n  ret i32 %z\n}\n",
    )
    .unwrap();
    assert!(text.contains("umov"), "{text}");
}

/// Two byte lanes added and returned as a byte.
#[test]
fn two_byte_lanes_added_compile() {
    compile(
        "define void @f(ptr %p, ptr %q, ptr %d) {\n  %v = load <16 x i8>, ptr %p, align 16\n  \
         %w = load <16 x i8>, ptr %q, align 16\n  %e = extractelement <16 x i8> %v, i64 3\n  \
         %g = extractelement <16 x i8> %w, i64 5\n  %s = add i8 %e, %g\n  store i8 %s, ptr %d, align 1\n  ret void\n}\n",
    )
    .unwrap();
}

/// A halfword lane sign-extended to an `i64` stopped `pattern_fill.c` at -O2
/// (`machine opcode SMOVvi16to64 has no arm64 instruction yet`): one `smov`.
#[test]
fn a_halfword_lane_sign_extended_to_i64_is_an_smov() {
    let text = compile(
        "define i64 @f(ptr %p) {\n  %v = load <8 x i16>, ptr %p, align 16\n  %e = extractelement <8 x i16> %v, i64 3\n  \
         %z = sext i16 %e to i64\n  ret i64 %z\n}\n",
    )
    .unwrap();
    assert!(text.contains("smov"), "{text}");
}
