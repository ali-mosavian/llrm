//! The `.eh_frame` an arm64 ELF object carries: a function with a frame states
//! it (`AArch64FrameLowering`'s `.cfi_*`), read back by binutils' `readelf`
//! (an unwinder's reader, not ours) and, where the pinned AArch64 LLVM is
//! built, against what it states at the same calls.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_arm64::machine::Platform;

fn llc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
}

/// `source`'s functions as an ELF object by this compiler, in `directory`.
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

/// Each FDE's rules as `readelf` words them, less the advances and padding:
/// the frame address and the saves in the order they are stated.
fn rules(object: &Path) -> Vec<Vec<String>> {
    let said = Command::new("readelf").arg("--debug-dump=frames").arg(object).output().expect("readelf starts");
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let mut out: Vec<Vec<String>> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.contains(" FDE ") {
            out.push(Vec::new());
        } else if let (Some(current), true) = (out.last_mut(), line.starts_with("DW_CFA_")) {
            if !line.starts_with("DW_CFA_advance_loc") && !line.starts_with("DW_CFA_nop") {
                current.push(line.to_owned());
            }
        }
    }
    out
}

/// A function that calls and uses the result keeps a frame record; before this
/// the object had no `.eh_frame`, and nothing could unwind through the call.
#[test]
fn a_function_with_a_frame_record_states_where_its_caller_is() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(
        directory.path(),
        "declare i32 @g(ptr)\ndefine i32 @caller(i32 %a) {\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n",
    );
    assert_eq!(
        rules(&object),
        [[
            "DW_CFA_def_cfa_offset: 16",
            "DW_CFA_offset: r29 (x29) at cfa-16",
            "DW_CFA_offset: r30 (x30) at cfa-8",
            "DW_CFA_def_cfa_register: r29 (x29)",
            "DW_CFA_def_cfa_register: r31 (sp)",
            "DW_CFA_def_cfa_offset: 0",
            "DW_CFA_restore: r29 (x29)",
            "DW_CFA_restore: r30 (x30)",
        ]
        .map(String::from)
        .to_vec()]
    );
}

/// A function with no frame has an entry that says only the CIE's rule.
#[test]
fn a_frameless_function_has_an_entry_with_no_rules() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(directory.path(), "define i32 @leaf(i32 %a) {\n  %r = add i32 %a, 1\n  ret i32 %r\n}\n");
    assert_eq!(rules(&object), [Vec::<String>::new()]);
}

/// A double kept across a call is saved by `d8`, which has no short form of
/// `DW_CFA_offset`: it was refused ("register 72 is past the short form") and
/// the object was not written.
#[test]
fn a_saved_float_register_is_stated_with_the_extended_form() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(
        directory.path(),
        "declare i32 @g(ptr)\ndefine double @f(double %a) {\n  %x = fadd double %a, 1.0\n  %r = call i32 @g(ptr null)\n  %y = fadd double %x, 2.0\n  ret double %y\n}\n",
    );
    let rules = rules(&object);
    assert!(
        rules[0].iter().any(|rule| rule.starts_with("DW_CFA_offset_extended") && rule.contains("r72")),
        "{rules:?}"
    );
    assert!(
        rules[0].iter().any(|rule| rule.starts_with("DW_CFA_restore_extended") && rule.contains("r72")),
        "{rules:?}"
    );
}

const DARWIN_DATALAYOUT: &str = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32";

/// The pinned AArch64 LLVM's `bin`, or None where it is not built (the test
/// then says it skipped).
fn pinned() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin =
        PathBuf::from(std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64")))
            .join("bin");
    bin.join("llvm-objdump").exists().then_some(bin)
}

/// `source` as a Mach-O object by this compiler, and what `llvm-objdump`
/// reads of its `__compact_unwind`: the encodings, in function order.
fn compact_encodings(source: &str) -> Option<Vec<String>> {
    let bin = pinned().or_else(|| {
        eprintln!("skipped: needs the pinned LLVM with AArch64");
        None
    })?;
    let directory = tempfile::tempdir().unwrap();
    let ir = directory.path().join("t.ll");
    std::fs::write(
        &ir,
        format!("target datalayout = \"{DARWIN_DATALAYOUT}\"\ntarget triple = \"arm64-apple-macosx11.0.0\"\n{source}"),
    )
    .unwrap();
    let out = directory.path().join("t.o");
    let said = Command::new(llc()).arg("-o").arg(&out).arg(&ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let said = Command::new(bin.join("llvm-objdump")).args(["--macho", "--unwind-info"]).arg(&out).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    Some(
        String::from_utf8_lossy(&said.stdout)
            .lines()
            .filter_map(|line| line.trim().strip_prefix("compact encoding:").map(|word| word.trim().to_owned()))
            .collect(),
    )
}

/// Before this a Mach-O object had no `__compact_unwind` or `__eh_frame`: the
/// writer refused an unwind section, and a frame's caller was lost to `ld64`.
#[test]
fn a_macho_object_has_a_compact_encoding_for_each_function() {
    let source = "declare i32 @g(ptr)\ndefine i32 @caller(i32 %a) {\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\ndefine i32 @leaf(i32 %a) {\n  %r = add i32 %a, 1\n  ret i32 %r\n}\n";
    let Some(found) = compact_encodings(source) else { return };
    // The pinned `llc` writes 0x04000000 (a frame, nothing else saved) and
    // 0x02000000 (frameless, no stack) for these.
    assert_eq!(found, ["0x04000000", "0x02000000"]);
}

/// A register saved below the record is not what the compact frame says (its
/// pairs are stored high register first), so the function is left to DWARF.
#[test]
fn a_function_saving_a_register_is_left_to_its_dwarf_frame() {
    let source = "declare i32 @g(ptr)\ndefine i32 @f(i32 %a) {\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, %a\n  ret i32 %s\n}\n";
    let Some(found) = compact_encodings(source) else { return };
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("0x03"), "{found:?}");
}
