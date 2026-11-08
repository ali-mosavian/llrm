//! `x * 31` at m64 -Os under `--isel=dag`: `mov; shl 5; sub` (8 B) where the
//! old selector and gcc keep `imul eax, edi, 31` (3 B). The m64 tuning lacked
//! `imul_at_size`, which m32 has; select_of_constants -Os was 14506 B against
//! 12987.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m64_dag_os_a_multiply_by_two_to_the_n_minus_one_is_one_imul() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mul31-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "unsigned f(unsigned x) { return x * 31; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .args(["-m64", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(listing.contains("imul eax, edi, 31"), "expected one imul:\n{listing}");
}
