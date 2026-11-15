//! A 28-byte aligned constant fill at -Os (lru's `lhead`/`ltail` init): seven
//! dword stores, within the pin's `MaxStoresPerMemsetOptSize` of 8. It was
//! `rep stosd` (DEVIATE REPFILL took it for the bytes): lru -Os 201 clk
//! against clang's 143.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_dag_os_a_fill_within_the_store_limit_is_stores_not_rep_stos() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-fill7-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "struct S { int a; int b[7]; } __attribute__((aligned(4)));\nstruct S s;\n\
         void f(void) { __builtin_memset(s.b, 0xff, 28); }\n",
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
    assert!(!listing.contains("rep stos"), "expected seven stores:\n{listing}");
}
