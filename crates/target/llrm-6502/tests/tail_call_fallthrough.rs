//! A call whose block falls into a lone `rts` is a jump: sibling_call_callees
//! -O1 ran `jsr h` then the shared `rts`, 9 clocks over `jmp h`
//! (258 clocks against llvm-mos's 249). Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("tail-call-fallthrough");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sibling_call_callees.c");
    let out = dir.join(format!("sibling.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-S", "-w", "-fsigned-char", "-fwhole-program", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn a_call_before_a_shared_return_is_a_jump() {
    let text = assembly("-O1");
    let g: String = text.lines().skip_while(|l| *l != "g:").take_while(|l| *l != "h:").collect::<Vec<_>>().join("\n");
    assert!(!g.contains("jsr "), "g calls then falls into rts:\n{g}");
}
