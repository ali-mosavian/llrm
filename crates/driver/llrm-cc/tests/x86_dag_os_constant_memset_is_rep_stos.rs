//! `memset(p, 1, 64)` on an unaligned pointer at -Os under `--isel=dag`: the
//! pin's `call memset` (push x3, call, add esp: 12-16 B) where the old
//! selector and gcc fill with `rep stos` (9-11 B). os_more -Os was 228 B
//! against 214.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_dag_os_a_constant_memset_past_the_store_limit_is_rep_stos() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-memset-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "extern unsigned char *more(int);\nvoid f(void) { unsigned char *p = more(64); __builtin_memset(p, 1, 64); }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(listing.contains("rep stos") && !listing.contains("call memset"), "expected rep stos:\n{listing}");
}
