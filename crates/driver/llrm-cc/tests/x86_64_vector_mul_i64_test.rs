//! x86-64: a loop whose induction is multiplied as `i64` is vectorized (the
//! pin's SSE2 `v2i64` multiply row is 7 a register). The row was missing, every
//! width was "not priced", and tile's 64-wide fill stayed scalar: bench tile
//! -O2 ran 14,000 instructions over gcc. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn an_i64_induction_multiply_loop_is_vectorized_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-mul-i64-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_mul_i64.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body =
        text.split("fill_i64 proc near").nth(1).and_then(|rest| rest.split("fill_i64 endp").next()).expect("a body");
    let vector_stores = body.matches("xmmword ptr [rdi").count();
    let scalar_stores = body.matches("mov word ptr [rdi").count();
    assert!(
        vector_stores >= 4 && scalar_stores == 0,
        "{vector_stores} vector and {scalar_stores} scalar stores:\n{body}"
    );
}
