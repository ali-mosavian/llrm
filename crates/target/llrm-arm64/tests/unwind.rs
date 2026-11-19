//! The `.eh_frame` an arm64 ELF object carries: a function with a frame states
//! it (`AArch64FrameLowering`'s `.cfi_*`), read back by binutils' `readelf`
//! (an unwinder's reader, not ours) and, where the pinned AArch64 LLVM is
//! built, against what it states at the same calls. A function has the rows of
//! its `uwtable` kind (`AArch64FunctionInfo::needsAsyncDwarfUnwindInfo`); the
//! tests that read epilogue rows say `uwtable`.

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
/// The saves are stated after the frame register is set up, highest address
/// first, as the pin does (they were stated at the `stp`, 4 bytes early, in the
/// other order: `cfi_diff.py` showed pc 4 `{x29, x30}` against the pin's none).
#[test]
fn a_function_with_a_frame_record_states_where_its_caller_is() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(
        directory.path(),
        "declare i32 @g(ptr)\ndefine i32 @caller(i32 %a) uwtable \"frame-pointer\"=\"non-leaf\" {\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n",
    );
    assert_eq!(
        rules(&object),
        [[
            "DW_CFA_def_cfa_offset: 16",
            "DW_CFA_def_cfa: r29 (x29) ofs 16",
            "DW_CFA_offset: r30 (x30) at cfa-8",
            "DW_CFA_offset: r29 (x29) at cfa-16",
            "DW_CFA_def_cfa: r31 (sp) ofs 16",
            "DW_CFA_def_cfa_offset: 0",
            "DW_CFA_restore: r30 (x30)",
            "DW_CFA_restore: r29 (x29)",
        ]
        .map(String::from)
        .to_vec()]
    );
}

const CALLER: &str = "declare i32 @g(ptr)\ndefine i32 @caller(i32 %a) ATTRS {\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n";

fn caller(attrs: &str) -> Vec<Vec<String>> {
    let directory = tempfile::tempdir().unwrap();
    rules(&object(directory.path(), &CALLER.replace("ATTRS", attrs)))
}

/// `nounwind` without `uwtable` was given a full asynchronous FDE (4 rows and
/// 4 restores) the pin does not write: `MachineFunction::needsFrameMoves` is
/// false, so the object carried an `.eh_frame` entry for every C function.
#[test]
fn a_nounwind_function_without_uwtable_has_no_entry() {
    assert_eq!(caller("nounwind \"frame-pointer\"=\"non-leaf\""), Vec::<Vec<String>>::new());
}

/// A function that may unwind has an entry without `uwtable`, with the rows of
/// the prologue alone (`needsUnwindTableEntry`; the kind is none).
#[test]
fn a_function_that_may_unwind_has_prologue_rows_only() {
    let expected =
        ["DW_CFA_def_cfa: r29 (x29) ofs 16", "DW_CFA_offset: r30 (x30) at cfa-8", "DW_CFA_offset: r29 (x29) at cfa-16"];
    assert_eq!(caller("\"frame-pointer\"=\"non-leaf\""), [expected.map(String::from).to_vec()]);
}

/// `uwtable(sync)` states the finished frame once, after the prologue's last
/// instruction, and nothing for the epilogue: it was the step-by-step rows of
/// `uwtable`, with `def_cfa_offset 16` at the `stp` and the restores.
#[test]
fn a_synchronous_table_states_the_prologue_frame_once() {
    let expected =
        ["DW_CFA_def_cfa: r29 (x29) ofs 16", "DW_CFA_offset: r30 (x30) at cfa-8", "DW_CFA_offset: r29 (x29) at cfa-16"];
    assert_eq!(caller("uwtable(sync) \"frame-pointer\"=\"non-leaf\""), [expected.map(String::from).to_vec()]);
}

/// An asynchronous table at `minsize` is the synchronous one
/// (`getUWTableKind() == Async && !hasMinSize()`).
#[test]
fn an_asynchronous_table_at_minsize_has_the_synchronous_rows() {
    let sync =
        ["DW_CFA_def_cfa: r29 (x29) ofs 16", "DW_CFA_offset: r30 (x30) at cfa-8", "DW_CFA_offset: r29 (x29) at cfa-16"];
    assert_eq!(caller("uwtable minsize \"frame-pointer\"=\"non-leaf\""), [sync.map(String::from).to_vec()]);
}

/// Without a frame record the synchronous row is the stack pointer's offset,
/// `.cfi_def_cfa_offset`, not a `DW_CFA_def_cfa` of `sp`.
#[test]
fn a_synchronous_frame_without_a_record_states_the_offset() {
    let rules = caller("uwtable(sync)");
    assert_eq!(rules.len(), 1);
    assert!(rules[0][0].starts_with("DW_CFA_def_cfa_offset: "), "{rules:?}");
    assert!(rules[0].iter().skip(1).all(|rule| rule.starts_with("DW_CFA_offset")), "{rules:?}");
}

/// The CIE counted code in 4-byte units and saves in 8-byte slots; the pin's
/// ELF CIE counts code in bytes and saves in 4 (`getMinInstAlignment`,
/// `getCalleeSaveStackSlotSize`), so every `.eh_frame` differed in its CIE.
#[test]
fn the_cie_counts_code_in_bytes_and_saves_in_four_byte_slots() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(directory.path(), &CALLER.replace("ATTRS", "uwtable"));
    let said = Command::new("readelf").arg("--debug-dump=frames").arg(&object).output().expect("readelf starts");
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    assert!(text.contains("Code alignment factor: 1"), "{text}");
    assert!(text.contains("Data alignment factor: -4"), "{text}");
}

/// A function with no frame has an entry that says only the CIE's rule.
#[test]
fn a_frameless_function_has_an_entry_with_no_rules() {
    let directory = tempfile::tempdir().unwrap();
    let object =
        object(directory.path(), "define i32 @leaf(i32 %a) uwtable {\n  %r = add i32 %a, 1\n  ret i32 %r\n}\n");
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
        "declare i32 @g(ptr)\ndefine double @f(double %a) uwtable \"frame-pointer\"=\"non-leaf\" {\n  %x = fadd double %a, 1.0\n  %r = call i32 @g(ptr null)\n  %y = fadd double %x, 2.0\n  ret double %y\n}\n",
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

/// Code laid out after the epilogue (an early exit's block that the branch
/// skips to) runs in the frame again: its rows state the frame address and
/// the saves anew. Without them the unwinder takes that code for a frameless
/// one, the state the epilogue left.
#[test]
fn code_laid_out_after_the_epilogue_states_the_frame_again() {
    let directory = tempfile::tempdir().unwrap();
    let object = object(
        directory.path(),
        "declare i32 @g(ptr)\ndeclare i32 @h(ptr)\ndefine i32 @f(i32 %a) uwtable \"frame-pointer\"=\"non-leaf\" {\n  %c = icmp eq i32 %a, 0\n  br i1 %c, label %early, label %late\nearly:\n  %r = call i32 @g(ptr null)\n  %q = mul i32 %r, 3\n  ret i32 %q\nlate:\n  %s = call i32 @h(ptr null)\n  %t = add i32 %s, %a\n  ret i32 %t\n}\n",
    );
    let rules = &rules(&object)[0];
    let torn_down = rules.iter().rposition(|rule| rule == "DW_CFA_def_cfa_offset: 0").expect("the epilogue");
    assert!(
        rules[torn_down..].iter().any(|rule| rule.starts_with("DW_CFA_def_cfa: r29")),
        "the block after the epilogue does not state the frame: {rules:?}"
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
    macho_unwind(source).map(|(encodings, _)| encodings)
}

/// [`compact_encodings`], and the names of the object's sections.
fn macho_unwind(source: &str) -> Option<(Vec<String>, Vec<String>)> {
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
    let encodings = String::from_utf8_lossy(&said.stdout)
        .lines()
        .filter_map(|line| line.trim().strip_prefix("compact encoding:").map(|word| word.trim().to_owned()))
        .collect();
    let said = Command::new(bin.join("llvm-objdump")).arg("-h").arg(&out).output().unwrap();
    let sections = String::from_utf8_lossy(&said.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).filter(|name| name.starts_with("__")).map(str::to_owned))
        .collect();
    Some((encodings, sections))
}

/// What the platform's clang gives a function that frames: no epilogue rows
/// (`uwtable(sync)`) and the frame register set (`non-leaf`).
const ATTRS: &str = "nounwind uwtable(sync) \"frame-pointer\"=\"non-leaf-no-reserve\"";

/// Before this a Mach-O object had no `__compact_unwind` or `__eh_frame`: the
/// writer refused an unwind section, and a frame's caller was lost to `ld64`.
#[test]
fn a_macho_object_has_a_compact_encoding_for_each_function() {
    let source = format!(
        "declare i32 @g(ptr)\ndefine i32 @caller(i32 %a) {ATTRS} {{\n  %r = call i32 @g(ptr null)\n  %s = add i32 %r, 1\n  ret i32 %s\n}}\ndefine i32 @leaf(i32 %a) {ATTRS} {{\n  %r = add i32 %a, 1\n  ret i32 %r\n}}\n"
    );
    let Some(found) = compact_encodings(&source) else { return };
    // The pinned `llc` writes 0x04000000 (a frame, nothing else saved) and
    // 0x02000000 (frameless, no stack) for these.
    assert_eq!(found, ["0x04000000", "0x02000000"]);
}

/// The Mach-O sections and compact words of `caller` with `attrs`.
fn macho_caller(attrs: &str) -> Option<(Vec<String>, Vec<String>)> {
    macho_unwind(&CALLER.replace("ATTRS", attrs))
}

/// A synchronous table is the compact frame alone: `__eh_frame` holds the
/// frames the compact word leaves to DWARF, and the pin writes none for it.
#[test]
fn a_synchronous_macho_frame_has_no_eh_frame() {
    let Some((words, sections)) = macho_caller(ATTRS) else { return };
    assert_eq!(words, ["0x04000000"]);
    assert!(!sections.contains(&"__eh_frame".to_owned()), "{sections:?}");
}

/// An asynchronous table states the epilogue (`def_cfa sp`), which the compact
/// word cannot: DWARF's word, no offset in it, and the frame in `__eh_frame`.
#[test]
fn an_asynchronous_macho_frame_is_left_to_its_dwarf_entry() {
    let Some((words, sections)) = macho_caller("nounwind uwtable \"frame-pointer\"=\"non-leaf-no-reserve\"") else {
        return;
    };
    assert_eq!(words, ["0x03000000"]);
    assert!(sections.contains(&"__eh_frame".to_owned()), "{sections:?}");
}

/// `nounwind` without `uwtable` had a compact entry and an FDE, as on ELF.
#[test]
fn a_macho_function_without_an_unwind_table_has_no_entry() {
    let Some((words, sections)) = macho_caller("nounwind \"frame-pointer\"=\"non-leaf-no-reserve\"") else { return };
    assert!(words.is_empty(), "{words:?}");
    assert!(
        !sections.contains(&"__compact_unwind".to_owned()) && !sections.contains(&"__eh_frame".to_owned()),
        "{sections:?}"
    );
}

/// The registers a frame saves are stored in pairs, the higher register at the
/// lower address, below the record, and the compact word names them. They
/// were stored the other way round, which the word cannot say, so each such
/// function was left to its DWARF frame (`0x03...`): 18 of 69 at -O2 against
/// the pin's one. A register whose partner is free is saved with it, and
/// `d8`, `x19` and `x21`/`x22` come in the order the word reads them.
#[test]
fn the_saved_pairs_of_a_frame_are_named_in_its_compact_word() {
    let source = format!(
        "declare i32 @g(ptr)
define i32 @one(i32 %a) {ATTRS} {{
  %r = call i32 @g(ptr null)
  %s = add i32 %r, %a
  ret i32 %s
}}
define i32 @three(i32 %a, i32 %b, i32 %c) {ATTRS} {{
  %r = call i32 @g(ptr null)
  %s = add i32 %r, %a
  %t = add i32 %s, %b
  %u = add i32 %t, %c
  ret i32 %u
}}
define double @fp(double %a) {ATTRS} {{
  %x = fadd double %a, 1.0
  %r = call i32 @g(ptr null)
  %y = fadd double %x, 2.0
  ret double %y
}}
define i64 @both(i64 %a, double %b) {ATTRS} {{
  %r = call i32 @g(ptr null)
  %f = fptosi double %b to i64
  %s = add i64 %a, %f
  ret i64 %s
}}
"
    );
    let Some(found) = compact_encodings(&source) else { return };
    // As the pinned `llc` writes them.
    assert_eq!(found, ["0x04000001", "0x04000003", "0x04000100", "0x04000101"]);
}

/// The object the entry block touches first sits at `[sp]`, so that access
/// folds into the frame's allocation (`str x0, [sp, #-32]!`); the pin's order
/// put the first object at the top, `sub sp` and `str [sp, #8]`, and the dense
/// one at the bottom cost the same two instructions here. The word is the
/// pin's frameless 32 bytes.
#[test]
fn the_first_touched_frame_object_is_at_sp_and_its_access_folds() {
    let source = format!(
        "define i64 @local(i64 %a, i32 %b) {ATTRS} {{
  %p = alloca i64
  %q = alloca i32
  store volatile i64 %a, ptr %p
  store volatile i32 %b, ptr %q
  %v = load volatile i32, ptr %q
  %w = load volatile i32, ptr %q
  %x = add i32 %v, %w
  %y = zext i32 %x to i64
  ret i64 %y
}}
"
    );
    let text = llrm_arm64::driver::assembly(
        &format!("target datalayout = \"{DARWIN_DATALAYOUT}\"\n{source}"),
        Platform::Darwin,
        true,
    )
    .expect("lowers");
    assert!(text.contains("str x0, [sp, #-16]!"), "{text}");
    let Some(found) = compact_encodings(&source) else { return };
    assert_eq!(found, ["0x02001000"]);
}
