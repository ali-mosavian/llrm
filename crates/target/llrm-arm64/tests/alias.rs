//! An IR alias is a symbol of the object: `@other = alias ..., ptr @impl` puts
//! `other` at `impl`'s address (the GlobalAlias plan, step 3).

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_arm64::machine::Platform;

fn llc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
}

fn object(
    directory: &Path,
    source: &str,
) -> PathBuf {
    let ir = directory.join("t.ll");
    std::fs::write(
        &ir,
        format!(
            "target datalayout = \"{}\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n{source}",
            Platform::Linux.datalayout()
        ),
    )
    .unwrap();
    let out = directory.join("t.o");
    let said = Command::new(llc()).arg("-o").arg(&out).arg(&ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    out
}

/// `(name, value)` of each symbol in `readelf -sW`.
fn symbols(object: &Path) -> Vec<(String, u64)> {
    let said = Command::new("readelf").arg("-sW").arg(object).output().expect("readelf starts");
    String::from_utf8_lossy(&said.stdout)
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let value = u64::from_str_radix(words.get(1)?, 16).ok()?;
            Some((words.last()?.to_string(), value))
        })
        .collect()
}

/// The module parsed and its call to `@other` resolved, but the object had
/// `impl` and `user` and no `other`: a caller in another unit failed to link
/// the name the source defines.
#[test]
#[ignore = "arm64 emits no alias symbols yet; remove with the emitter (globalalias-plan.md step 3)"]
fn an_ir_alias_is_a_symbol_at_its_aliasees_address() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(
        directory.path(),
        "define i32 @impl(i32 %x) {\n  %r = add i32 %x, 1\n  ret i32 %r\n}\n@other = alias i32 (i32), ptr @impl\n\
         define i32 @user(i32 %x) {\n  %r = call i32 @other(i32 %x)\n  ret i32 %r\n}\n",
    );
    let found = symbols(&object);
    let at = |name: &str| found.iter().find(|one| one.0 == name).map(|one| one.1);
    assert!(at("other").is_some(), "no symbol `other`: {found:?}");
    assert_eq!(at("other"), at("impl"));
}
