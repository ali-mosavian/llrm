//! m32 -Os (SETCC-TREE-BRANCH): `m + (r > 4 || (r == 4 && odd))` was two
//! `setcc`, an `or`, a `movzx` and an `add` (sf32_pack: 258 B, gcc 233 B); it is
//! two compares that jump over an `inc`. Needs `LLRM_OW` and the m16 clang
//! (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "typedef unsigned u32;
u32 round_up(u32 m)
{
    u32 r = m & 7;
    m >>= 3;
    if (r > 4 || (r == 4 && (m & 1))) m++;
    return m;
}
";

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    for var in ["LLRM_OW", "LLRM_CLANG_M16"] {
        assert!(std::env::var_os(var).is_some(), "{var} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("setcctree-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = |mode: &str, out: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([flag, "-w", mode, "-march=i486", "-mabi=sysv", "-o"])
            .arg(dir.join(out))
            .arg(dir.join("d.c"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run("-S", "d.s");
    run("-c", "d.o");
    (
        std::fs::read_to_string(dir.join("d.s")).unwrap(),
        std::fs::metadata(dir.join("d.o")).unwrap().len(),
    )
}

#[test]
fn test_m32_os_a_sum_of_two_compares_is_jumps_not_setcc() {
    let (listing, _) = compile("round", SOURCE, "-Os");
    assert!(!listing.lines().any(|line| line.trim_start().starts_with("set")), "the compares are materialised:\n{listing}");
}
