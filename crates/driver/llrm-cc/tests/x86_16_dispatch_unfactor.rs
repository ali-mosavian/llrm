//! A computed-goto interpreter with 20 handlers on m16. At -O2 its dispatch was
//! one block with an indirect jump (2 in the listing); the pin's
//! post-allocation tail duplication copies it into every handler. Needs
//! `LLRM_LLC` and `LLRM_CLANG_M16` (the m16 fork clang).

use std::path::Path;
use std::process::Command;

/// Indirect jumps through a word in the listing of `m32_dispatch_unfactor.c` at
/// `level`.
fn indirect_jumps(level: &str) -> usize {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    std::env::var_os("LLRM_CLANG_M16").expect("LLRM_CLANG_M16 is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "x86-16-dispatch-unfactor-{}-{}",
        std::process::id(),
        level
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listing = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-ffreestanding", "-S", "-o"])
        .arg(&listing)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_dispatch_unfactor.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&listing).unwrap();
    text.lines().filter(|line| line.trim_start().starts_with("jmp word ptr ")).count()
}

#[test]
fn a_computed_goto_dispatch_is_copied_into_each_handler() {
    let jumps = indirect_jumps("-O2");
    assert!(jumps >= 20, "{jumps} indirect jumps: the dispatch is still one shared block (the pin: 21)");
}

#[test]
fn an_os_dispatch_stays_one_block() {
    let jumps = indirect_jumps("-Os");
    assert!(jumps <= 4, "{jumps} indirect jumps at -Os: the dispatch was copied into the handlers (bytes first)");
}

/// `double_across_a_call_result` at -Os: its `turn` is framed only by reading
/// its arguments through `bp`, and the copy of `pop bp; retf` into both arms
/// cost 2 B. The pin's return block holds the frame's `pop`: two instructions,
/// one allowed at -Os.
#[test]
fn an_os_return_block_with_a_frame_pop_is_not_copied() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    std::env::var_os("LLRM_CLANG_M16").expect("LLRM_CLANG_M16 is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-16-frame-pop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listing = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&listing)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c/double_across_a_call_result.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&listing).unwrap();
    let turn = text.split("_turn@3 proc far").nth(1).and_then(|rest| rest.split("endp").next()).unwrap();
    let returns = turn.lines().filter(|line| line.trim() == "retf").count();
    assert_eq!(returns, 1, "{returns} returns in turn: the frame's pop and ret were copied into each arm\n{turn}");
}
