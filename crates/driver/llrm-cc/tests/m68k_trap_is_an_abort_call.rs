//! `__builtin_trap` stopped the m68k backend with "Unsupported Trap: convert
//! node to libcall" (QB unit/num_test.c, every level, 68000 and 68020): the
//! target has no trap node, so the legalizer calls `abort` on the node's chain
//! (`LegalizeDAG::ExpandNode`, `ISD::TRAP`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "void f(int x) { if (x) __builtin_trap(); }\n\
    int g(int x) { if (x > 3) __builtin_trap(); return x + 1; }\n";

#[test]
fn a_builtin_trap_on_m68k_calls_abort() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68ktrap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t.c"), SOURCE).unwrap();
    for target in ["m68k-68000", "m68k-68020"] {
        for level in ["-O1", "-O2", "-Os"] {
            let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
                .env("LLRM_CC_TARGET", target)
                .args([level, "-w", "-S", "-o"])
                .arg(dir.join("t.s"))
                .arg(dir.join("t.c"))
                .output()
                .unwrap();
            assert!(run.status.success(), "{target} {level}: {}", String::from_utf8_lossy(&run.stderr));
            let text = std::fs::read_to_string(dir.join("t.s")).unwrap();
            assert!(text.contains("abort"), "{target} {level}: no call of abort:\n{text}");
        }
    }
}
