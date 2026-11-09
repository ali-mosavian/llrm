//! A m16 function with a dynamic `alloca` and no frame cell keeps its frame register, which restores the stack pointer at the return. Needs the
//! pinned clang (`LLRM_PIN_CLANG`) and the Open Watcom headers (tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn has(
    code: &[u8],
    pattern: &[u8],
) -> bool {
    code.windows(pattern.len()).any(|window| window == pattern)
}

/// At -O1/-O2/-Os `f` was compiled with no `push bp; mov bp, sp` and no `lea sp, [bp]`: it returned with the stack pointer where the alloca
/// left it, so the `retf` popped the allocated bytes as the return address.
#[test]
fn a_function_with_an_alloca_and_no_frame_cell_restores_the_stack_pointer() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_PIN_CLANG is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16alloca-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), "extern void use(int *);\nvoid f(int n) { int *p = __builtin_alloca(n * 4); use(p); }\n").unwrap();
    for level in ["-O1", "-O2", "-Os"] {
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_16-wat")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([level, "-w", "-c", "-march=i386", "-o"])
            .arg(dir.join("f.o"))
            .arg(dir.join("f.c"))
            .output()
            .unwrap();
        assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
        let code = std::fs::read(dir.join("f.o")).unwrap();
        assert!(has(&code, &[0x55, 0x89, 0xe5]), "{level}: no `push bp; mov bp, sp`");
        assert!(has(&code, &[0x8d, 0x66]) || has(&code, &[0x8d, 0xa6]), "{level}: no `lea sp, [bp + d]` at the return");
    }
}
