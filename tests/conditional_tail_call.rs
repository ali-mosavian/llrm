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

/// The block keeps its epilogue (`add esp, 4`): the pin folds only a block
/// that starts with the tail call. The fixture has a frame slot; without one
/// (as #8976 left the first fixture) there is no epilogue to keep.
#[test]
fn test_m32_block_with_an_epilogue_before_the_tail_call_is_not_folded() {
    let text = compiled("cond_tail_call_m32.ll");
    assert!(text.contains("sub esp, 4"), "premise: the function has a frame\n{text}");
    assert!(text.contains("add esp, 4\n    jmp other_"), "{text}");
    assert!(!text.contains("je other_") && !text.contains("jb other_"), "{text}");
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

/// `f` is reached by a branch and by the fall-through of the last test: the pin
/// (BranchFolder::OptimizeBlock, the move of a no-successor block, then the
/// fold) ends with three `je` and no `jmp`. Ours kept `jmp f` until the tail
/// fold learned the move (#9833).
fn assert_three_conditional_tail_jumps(level: &str) {
    let text = compiled_at("cond_tail_call_shared_m64.ll", level);
    assert!(text.matches("je f").count() == 2 && text.contains("je h") && !text.contains("jmp"), "{text}");
}

#[test]
fn test_m64_branch_to_a_callee_also_reached_by_fall_through_is_folded_at_o2() {
    assert_three_conditional_tail_jumps("-O2");
}

/// The pin folds at every size (no TC1 guard).
#[test]
fn test_m64_branch_to_a_callee_also_reached_by_fall_through_is_folded_at_os() {
    assert_three_conditional_tail_jumps("-Os");
}
