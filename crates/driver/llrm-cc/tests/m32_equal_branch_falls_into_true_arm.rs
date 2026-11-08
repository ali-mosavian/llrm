//! A 50/50 branch `e >= 23 ? m << ... : m >> ...` at m32 -O2: the pin lists a
//! block's successors true edge first, and block placement breaks ties by
//! it, so the true arm is the fall-through (`cmp; jl false_arm`). Ours listed
//! the jump target first and fell into the false arm (`jge`), which sank the
//! other arms of u64_shift_select_arms behind long jumps (+33 B against the
//! pin).

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_o2_an_even_branch_falls_into_its_true_arm() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("true-arm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "typedef unsigned long long u64;\ntypedef unsigned long u32;\nextern void report(long);\n\
         void f(volatile u32 *pa, volatile int *pe)\n{\n    u32 a = *pa;\n    int e = *pe;\n\
         u64 m = (a & 0x7FFFFF) | 0x800000;\n    report((long)(e >= 23 ? m << (e - 23) : m >> (23 - e)));\n}\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-O2", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    let branch = listing
        .lines()
        .skip_while(|line| !line.trim().starts_with("cmp ecx, 23"))
        .nth(1)
        .map(str::trim)
        .unwrap_or_default();
    assert!(
        branch.starts_with("jl "),
        "the branch after the compare is `{branch}`, not `jl` to the false arm:\n{listing}"
    );
}
