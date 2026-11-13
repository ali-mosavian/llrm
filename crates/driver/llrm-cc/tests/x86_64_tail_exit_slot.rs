//! x86-64: a one-slot frame is `push rax` / `pop reg` also where the function leaves by a sibling call
//! (`X86RegisterInfo::findDeadCallerSavedReg` handles `TCRETURN*`: the first register the jump does not read).
//! It was `sub rsp, 8` / `add rsp, 8` whenever any exit was a `jmp` (+6 B per frame: shl_long_by_one `four`
//! -O1 +6 B, loop_fusion, sfm16_dbg, 8 sfm16_float programs). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tail-exit-slot-{}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/tail_exit_slot.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(text: &'a str, name: &str) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn a_one_slot_frame_before_a_sibling_call_is_a_push_and_a_pop() {
    for level in ["-O1", "-Os"] {
        let text = assembly(level);
        let tail = body(&text, "tail");
        assert!(tail.contains("push rax"), "{level}: no push rax:\n{tail}");
        assert!(!tail.contains("sub rsp") && !tail.contains("add rsp"), "{level}: a sub/add frame:\n{tail}");
    }
}

/// `g4` reads `edi esi edx ecx`: the popped register may be none of them.
#[test]
fn the_pop_before_a_sibling_call_is_a_register_the_jump_does_not_read() {
    let text = assembly("-Os");
    for name in ["tail4", "mixed"] {
        let function = body(&text, name);
        let lines: Vec<&str> = function.lines().map(str::trim).collect();
        for (at, line) in lines.iter().enumerate() {
            if line.starts_with("jmp g4") && let Some(pop) = lines[..at].iter().rev().find(|line| line.starts_with("pop ")) {
                let register = pop.trim_start_matches("pop ");
                assert!(!["rdi", "rsi", "rdx", "rcx"].contains(&register), "{name}: `{pop}` before the jump:\n{function}");
            }
        }
    }
}
