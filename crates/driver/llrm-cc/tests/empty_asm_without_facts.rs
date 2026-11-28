//! A compiler barrier, `asm volatile("" ::: "memory")`, on a target without
//! inline-assembly facts reached its backend, which stopped on it ("expected a
//! constant, found `asm`"): it holds no instruction and is dropped, as on x86.

use std::path::Path;
use std::process::Command;

#[test]
fn an_empty_asm_on_a_target_without_inline_asm_facts_is_dropped() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("empty-asm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("b.c"),
        "int g;\nint f(int x) { g = x; __asm__ volatile(\"\" ::: \"memory\"); return g; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "m68k-68000")
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("b.s"))
        .arg(dir.join("b.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
