//! A loaded `i16`/`i32` converted with `sitofp` on the x87 is `fild`ed from
//! its own address (SD332), so `fimul`/`fiadd` read it in place. It was stored
//! to a stack slot and reloaded (`mov [bp-20], eax; fimul [bp-20]`): two more
//! instructions and a frame temporary per use (jpeg_single's dot loop, 589824
//! trips).

mod common;

use std::process::Command;

fn asm(body: &str) -> String {
    let layout = common::datalayout("x86-m16");
    let layout = if layout.ends_with("-ni:2:3") { layout } else { format!("{layout}-ni:2:3") };
    let ir = format!("target datalayout = \"{layout}\"\n{body}");
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("f.ll");
    std::fs::write(&input, ir).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stdout).into_owned()
}

fn scale(
    int: &str,
    bits: &str,
    qualifiers: &str,
    extra: &str,
) -> String {
    format!(
        "define float @f(ptr %p, ptr %q) {{\n  %x = load float, ptr %p\n  %i = load {qualifiers}{int}, ptr %q\n{extra}  %c = sitofp {int} %i to float\n  %m = fmul float %x, %c\n  ret float %m\n}}\n; {bits}\n"
    )
}

#[test]
fn a_loaded_i32_is_multiplied_in_place() {
    let text = asm(&scale("i32", "32", "", ""));
    assert!(text.contains("fimul dword ptr [") && !text.contains("fimul dword ptr [bp-"), "{text}");
}

#[test]
fn a_loaded_i16_is_multiplied_in_place() {
    let text = asm(&scale("i16", "16", "", ""));
    assert!(text.contains("fimul word ptr [") && !text.contains("fimul word ptr [bp-"), "{text}");
}

#[test]
fn a_load_with_a_second_user_keeps_its_slot() {
    let text = asm(
        "define float @f(ptr %p, ptr %q, ptr %r) {\n  %x = load float, ptr %p\n  %i = load i32, ptr %q\n  store i32 %i, ptr %r\n  %c = sitofp i32 %i to float\n  %m = fmul float %x, %c\n  ret float %m\n}\n",
    );
    assert!(text.contains("fimul dword ptr [bp-"), "{text}");
}

#[test]
fn a_volatile_load_keeps_its_slot() {
    let text = asm(&scale("i32", "32", "volatile ", ""));
    assert!(text.contains("fimul dword ptr [bp-"), "{text}");
}

#[test]
fn a_store_between_the_load_and_the_use_does_not_move_the_read() {
    let text = asm(
        "define float @f(ptr %p, ptr %q) {\n  %x = load float, ptr %p\n  %i = load i32, ptr %q\n  store i32 0, ptr %q\n  %c = sitofp i32 %i to float\n  %m = fmul float %x, %c\n  ret float %m\n}\n",
    );
    let read = text.find("fimul dword ptr [").or_else(|| text.find("fild dword ptr [")).expect("the read");
    let store = text.find("], 0\n").expect("the store");
    assert!(read < store, "{text}");
}
