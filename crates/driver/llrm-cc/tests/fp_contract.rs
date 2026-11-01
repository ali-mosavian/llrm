//! `a * b + c` compiles under clang's default contraction (`-ffp-contract=on`,
//! an `llvm.fmuladd`) to the x87 program of `-ffp-contract=off`, and both give
//! the extended-precision intermediate: 1 + 2^-29 + 2^-60 does not survive a
//! rounding to double, so a result of zero would mean the product was rounded.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn run(contract: Option<&str>) -> (String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let work =
        std::env::temp_dir().join(format!("llrm-cc-contract-{}-{}", contract.unwrap_or("default"), std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let source = root.join("tests/cc/fp_contract/contract.c");
    let compile = |extra: &[&str], output: &Path, listing: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
        command
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
            .args(["-O2"])
            .args(extra);
        if listing {
            command.arg("-S");
        }
        let out = command.arg(&source).arg("-o").arg(output).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    let flag = contract.map(|one| format!("-ffp-contract={one}"));
    let extra: Vec<&str> = flag.iter().map(String::as_str).collect();
    compile(&extra, &work.join("contract.s"), true);
    compile(&extra, &work.join("contract"), false);
    let listing = std::fs::read_to_string(work.join("contract.s")).unwrap();
    let printed = Command::new(work.join("contract")).output().unwrap();
    let _ = std::fs::remove_dir_all(&work);
    (listing, String::from_utf8_lossy(&printed.stdout).trim().to_owned())
}

/// The instructions of the function `name`, constant labels (numbered by how
/// many the module made) left out.
fn body(
    listing: &str,
    name: &str,
) -> Vec<String> {
    listing
        .lines()
        .skip_while(|line| !line.starts_with(&format!("{name} proc")))
        .take_while(|line| !line.starts_with(&format!("{name} endp")))
        .map(|line| line.trim().to_owned())
        .collect()
}

#[test]
fn a_contracted_multiply_add_is_the_x87_multiply_then_add() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG names no pinned clang");
        return;
    }
    let (default_listing, default_output) = run(None);
    let (off_listing, off_output) = run(Some("off"));
    for name in ["f", "g"] {
        assert_eq!(body(&default_listing, name), body(&off_listing, name), "{name}:\n{default_listing}");
    }
    assert_eq!(default_output, "3c30000000000000 33800000", "double rounding gives zero here");
    assert_eq!(default_output, off_output);
}
