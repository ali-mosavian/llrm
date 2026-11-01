//! Machine opcodes the DAG path emitted as "has no arm64 instruction yet":
//! 16 of the corpus's bench kernels (queens `place`, ...) were refused by
//! `--isel=dag` and fell to the hand selector.

use std::process::Command;

const HEADER: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn dag(body: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
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

/// `add x, y, sext(z)` is the pin's `ADDXrx` (`add x0, x0, w1, sxtw`).
#[test]
fn an_add_of_a_sign_extended_word_is_the_extended_register_add() {
    let asm =
        dag("define i64 @f(i64 %a, i32 %b) {\n  %e = sext i32 %b to i64\n  %r = add i64 %a, %e\n  ret i64 %r\n}\n")
            .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("add x0, x0, w1, sxtw"), "{asm}");
}

/// The high half of a 64x64 multiply is `SMULHrr`.
#[test]
fn the_high_half_of_a_signed_multiply_is_smulh() {
    let asm = dag("define i64 @f(i64 %a, i64 %b) {\n  %x = sext i64 %a to i128\n  %y = sext i64 %b to i128\n  %m = mul i128 %x, %y\n  %h = lshr i128 %m, 64\n  %r = trunc i128 %h to i64\n  ret i64 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("smulh x0, x0, x1"), "{asm}");
}

/// A pointer stepped past each element is a post-index store: the pin selects
/// `STRXpost` (`str x1, [x0], #8`), a node with two results.
#[test]
fn a_store_that_steps_its_pointer_is_post_indexed() {
    let asm = dag("define ptr @f(ptr %p, i64 %v) {\n  store i64 %v, ptr %p, align 8\n  %n = getelementptr i64, ptr %p, i64 1\n  ret ptr %n\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("str x1, [x0], #8"), "{asm}");
}

/// A load whose pointer is also stepped is `LDRXpost` (`ldr x8, [x0], #8`):
/// its second result is the value, which `operand` took for the flags, so
/// the copy of the value to its register was "a copy of a constant" and the
/// function was refused.
#[test]
fn a_load_that_steps_its_pointer_is_post_indexed() {
    let asm = dag("define i64 @h(ptr %p, ptr %out) {\n  %v = load i64, ptr %p, align 8\n  %n = getelementptr i64, ptr %p, i64 1\n  store ptr %n, ptr %out, align 8\n  ret i64 %v\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("[x0], #8"), "{asm}");
}

/// A `memset` the legalizer turned into a libcall has an `ExternalSymbol`
/// callee; the DAG path refused it as `CannotSelect("ExternalSymbol")`, which
/// stopped 8 of the corpus's bench kernels (bintree, ...) at `--isel=dag`.
#[test]
fn a_libcall_the_legalizer_made_is_a_direct_call() {
    let asm = dag("declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\ndefine void @f(ptr %p, i64 %n) {\n  call void @llvm.memset.p0.i64(ptr %p, i8 0, i64 %n, i1 false)\n  ret void\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("bl _memset") || asm.contains("bl _bzero"), "{asm}");
}
