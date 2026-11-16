//! A local function takes gcc's `regparm(3)` (EAX, EDX, ECX), so a call of it
//! in tail position is a `jmp`. Under the pin's two-register `fastcc` the third
//! word went on the stack, `isEligibleForSiblingCallOpt` refused a computed
//! stack argument and every call stayed a call: x_sf_i2d -Os ran 1078 calls to
//! gcc's none.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_a_local_functions_tail_call_is_a_jmp_through_three_registers() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("local-regparm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "static __attribute__((noinline)) long long helper(long long a, unsigned b) { return a * b + 7; }\n\
         long long f(int x) { return helper((long long)x << 3, (unsigned)x ^ 5u); }\n",
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
    assert!(listing.contains("jmp helper") && !listing.contains("call helper"), "expected a tail jmp:\n{listing}");
}
