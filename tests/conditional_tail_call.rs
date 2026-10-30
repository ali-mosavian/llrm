//! A conditional branch to a block of only a sibling call is that call, `jcc
//! callee` (BranchFolding.cpp L1642, DEVIATE TC1). Before, `jb L; ...; L: jmp
//! other` followed a `ret`.

use std::process::Command;

fn compiled(fixture: &str) -> String {
    compiled_at(fixture, "-O2")
}

fn compiled_at(
    fixture: &str,
    level: &str,
) -> String {
    let path = format!("{}/tests/check/mir/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args([level, "-S", "-o", "-", &path]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn test_m64_branches_to_a_tail_call_jump_to_the_callee() {
    let text = compiled("cond_tail_call_m64.ll");
    assert!(text.contains("jb other") && text.contains("je other"), "{text}");
    assert!(!text.contains("jmp other"), "{text}");
}

/// The block keeps its epilogue (`pop ecx`): the pin folds only a block that
/// starts with the tail call.
#[test]
fn test_m32_block_with_an_epilogue_before_the_tail_call_is_not_folded() {
    let text = compiled("cond_tail_call_m32.ll");
    assert!(text.contains("pop ecx\n    jmp other_"), "{text}");
}

/// 16-bit code is outside the fold (a `jcc` to an external near symbol has no
/// rel8/rel16 fixup here).
#[test]
fn test_m16_keeps_the_jump_to_the_tail_call() {
    let text = compiled("cond_tail_call_m16.ll");
    assert!(text.contains("jmp _other@3"), "{text}");
}

#[test]
fn test_m64_object_has_a_conditional_jump_with_a_relocation_to_the_callee() {
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("t.o");
    let fixture = format!("{}/tests/check/mir/cond_tail_call_m64.ll", env!("CARGO_MANIFEST_DIR"));
    let output =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-o"]).arg(&object).arg(&fixture).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let bytes = std::fs::read(&object).unwrap();
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|window| window == needle);
    assert!(has(&[0x0F, 0x82, 0, 0, 0, 0]) && has(&[0x0F, 0x84, 0, 0, 0, 0]), "no jb/je rel32 in the object");
}

/// `f` is reached by a branch and by the fall-through of the last test: the
/// branch is folded (`je f`), the fall-through stays a `jmp f` (the pin folds
/// only a branch whose taken block it is).
#[test]
fn test_m64_branch_to_a_callee_also_reached_by_fall_through_is_folded_at_o2() {
    let text = compiled_at("cond_tail_call_shared_m64.ll", "-O2");
    assert!(text.contains("je f") && text.contains("je h") && text.contains("jmp f"), "{text}");
}

/// DEVIATE TC1: a `jcc rel32` is 4 bytes more than a `jcc rel8` and the `jmp`
/// stays for the fall-through; at -Os the fold waits for a callee that one
/// block reaches (`h` folds, `f` does not).
#[test]
fn test_m64_callee_reached_from_two_blocks_is_not_folded_at_os() {
    let text = compiled_at("cond_tail_call_shared_m64.ll", "-Os");
    assert!(text.contains("je h") && !text.contains("je f") && text.contains("jmp f"), "{text}");
}

/// The pin's form behind the switch: the callee reached from a branch and a
/// fall-through folds at -Os too.
#[test]
fn test_m64_pin_form_of_tc1_folds_at_os() {
    let path = format!("{}/tests/check/mir/cond_tail_call_shared_m64.ll", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .env("LLRM_TC1_PIN", "1")
        .args(["-Os", "-S", "-o", "-", &path])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("je f") && text.contains("je h"), "{text}");
}
