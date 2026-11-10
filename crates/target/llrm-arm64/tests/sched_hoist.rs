//! The pin schedules `-mcpu=generic` (Cortex-A510, in order) so that a load's
//! dependent divides leave the stores in front of them: `sdiv` is issued
//! before the first store. Our scheduler was the i486 shape (bottom-up, no
//! model), which kept source order: the divides came after every store.

use std::process::Command;

const DIAMOND: &str = r#"target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

declare void @use(ptr)

define i32 @f(i32 %x, i32 %y) {
  %a = alloca [6 x i32], align 4
  %b = alloca i32, align 4
  store i32 %x, ptr %b, align 4
  %a0 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 0
  %a1 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 1
  %a2 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 2
  %a3 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 3
  %a4 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 4
  %a5 = getelementptr inbounds [6 x i32], ptr %a, i32 0, i32 5
  store i32 1, ptr %a0, align 4
  store i32 2, ptr %a1, align 4
  store i32 3, ptr %a2, align 4
  store i32 4, ptr %a3, align 4
  store i32 5, ptr %a4, align 4
  store i32 6, ptr %a5, align 4
  %v = load i32, ptr %b, align 4
  %d1 = sdiv i32 %v, %y
  %d2 = sdiv i32 %d1, %y
  call void @use(ptr %a)
  ret i32 %d2
}
"#;

fn assembly() -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, DIAMOND).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

/// The index of the first line whose mnemonic is one of `mnemonics` and that
/// does not mention `except`.
fn first(
    text: &str,
    mnemonics: &[&str],
    except: &str,
) -> usize {
    text.lines()
        .position(|line| {
            line.split_whitespace().next().is_some_and(|word| mnemonics.contains(&word))
                && (except.is_empty() || !line.contains(except))
        })
        .unwrap_or_else(|| panic!("no {mnemonics:?} in:\n{text}"))
}

/// The six-store, one-load, two-divide diamond under `generic`: the pin's
/// `llc -mcpu=generic -O2` issues the first `sdiv` before the first store to
/// the array (under `apple-m1` and `neoverse-n1` it stays behind them).
#[test]
fn the_generic_cpu_issues_a_dependent_divide_before_the_stores() {
    let text = assembly();
    // The prologue's `stp x30, ...` is no store of the program.
    assert!(
        first(&text, &["sdiv"], "") < first(&text, &["str", "stp"], "x30"),
        "the divide stayed behind the stores:\n{text}"
    );
}
