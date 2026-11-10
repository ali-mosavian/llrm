//! A monotonic atomic access of a global folds the page offset into the
//! access, as a plain one does (`isWorthFoldingADDlow` counts `ATOMIC_LOAD`
//! and `ATOMIC_STORE` users that are not stronger than monotonic).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const HEADER: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@g = dso_local global i32 0, align 4
"#;

fn instructions(body: &str) -> Vec<String> {
    let source = format!("{HEADER}{body}");
    let asm = driver::assembly(&source, Platform::Linux, Level::O2).expect("lowers");
    asm.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('.') && !l.ends_with(':'))
        .map(String::from)
        .collect()
}

/// `load atomic monotonic` of a global was `adrp; add x9, x9, :lo12:g;
/// ldr w0, [x9]`, one instruction more than the pin's `adrp; ldr w0, [x8,
/// :lo12:g]`.
#[test]
fn a_monotonic_atomic_load_of_a_global_folds_the_page_offset() {
    let code = instructions("define i32 @f() {\n %v = load atomic i32, ptr @g monotonic, align 4\n ret i32 %v\n}\n");
    assert!(code.iter().all(|line| !line.starts_with("add")), "{code:?}");
}

/// The same for the store.
#[test]
fn a_monotonic_atomic_store_to_a_global_folds_the_page_offset() {
    let code =
        instructions("define void @f(i32 %x) {\n store atomic i32 %x, ptr @g monotonic, align 4\n ret void\n}\n");
    assert!(code.iter().all(|line| !line.starts_with("add")), "{code:?}");
}

/// A release store keeps `stlr`, which has no offset form: the page offset is
/// added in a register (`isStrongerThanMonotonic`), as before.
#[test]
fn a_release_store_to_a_global_keeps_the_address_in_a_register() {
    let code = instructions("define void @f(i32 %x) {\n store atomic i32 %x, ptr @g release, align 4\n ret void\n}\n");
    assert!(code.iter().any(|line| line.starts_with("stlr")), "{code:?}");
    assert!(code.iter().any(|line| line.starts_with("add")), "{code:?}");
}
