//! m32 -Os (VASTART-PROMOTE): a one-pointer `va_list` is kept in a register.
//! With `<stdarg.h>` the list was a stack slot `llvm.va_start` wrote through,
//! so `va_arg` loaded and stored it on every trip: `sum` below was `sub esp,8;
//! lea; mov [esp],eax; ... mov edx,[esp]; mov [esp],edx` (46 B, gcc 32 B).
//! Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16 clang
//! (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "#include <stdarg.h>
int sum(int n, ...)
{
    va_list ap;
    int total = 0;
    va_start(ap, n);
    while (n-- > 0)
        total += va_arg(ap, int);
    va_end(ap);
    return total;
}
";

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vastart-{name}-{}", std::process::id()));
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
    (std::fs::read_to_string(dir.join("d.s")).unwrap(), std::fs::metadata(dir.join("d.o")).unwrap().len())
}

#[test]
fn test_m32_os_va_list_is_not_a_stack_slot() {
    let (listing, _) = compile("sum", SOURCE, "-Os");
    assert!(!listing.contains("sub esp"), "the list lives in memory:\n{listing}");
}
