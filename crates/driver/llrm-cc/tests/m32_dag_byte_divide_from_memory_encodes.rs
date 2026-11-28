//! `unsigned char a / b` with `b` on the stack under `--isel=dag` at m32: the
//! listing printed `div byte ptr [esp+8]`, but `-c` failed ("ud:
//! Semantics(op=div ...)") because `divide_mem` refused width 1. The corpus
//! sweep lost div8.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_dag_a_byte_divide_by_a_stack_argument_assembles() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("div8-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "unsigned char ud(unsigned char a, unsigned char b) { return a / b; }\n\
         signed char sd(signed char a, signed char b) { return a / b; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-Os", "--isel=dag", "-w", "-c", "-o"])
        .arg(dir.join("c.o"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
