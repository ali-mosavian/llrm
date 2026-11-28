//! x86-64 -O2/-O3: a full unroll is the pin's threshold, not a price in clocks
//! against bytes. Bytes are not counted at O1-O3 (ruling 2026-11-14); the byte
//! price (312 milliclocks a byte) vetoed the 3-trip outer loop of
//! `rotate_small` because it saw "saves 18 clk, adds 241 B" and not the nest
//! that folds once the table's elements are constants: 1925 executed
//! instructions in `main` where clang's is 690. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(
    source: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("fu-{}-{call}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// `rotate_small.c`: the outer loop over `bytes[i]`/`words[i]` is unrolled, the
/// inner loops fold on the constants, and `main` no longer reads the two tables
/// (it did: 86 lines against 19).
#[test]
fn a_nest_over_constant_tables_is_unrolled_and_no_longer_reads_them() {
    for level in ["-O2", "-O3"] {
        let text = assembly("rotate_small.c", level);
        let main = text.split("main proc near").nth(1).and_then(|rest| rest.split("main endp").next()).expect("a body");
        assert!(!main.contains("bytes") && !main.contains("words"), "{level}: {main}");
    }
}

/// A loop holding a variable-length array has `llvm.stacksave` and
/// `llvm.stackrestore` in its body; the unroller could not price the two calls
/// ("no cost for call") and kept `nested(6)` a loop: `call nested`, 3,942
/// executed instructions to clang's 2,752. The pin prices an intrinsic it has
/// no case for as a library call (1 for size): the nest unrolls and
/// folds to `183`.
#[test]
fn a_loop_with_a_variable_length_array_is_unrolled_and_folds_to_a_constant() {
    for level in ["-O2", "-O3"] {
        let text = assembly("vla_callee_constant.c", level);
        let main = text.split("main proc near").nth(1).and_then(|rest| rest.split("main endp").next()).expect("a body");
        assert!(!main.contains("call nested") && main.contains("mov rdi, 183"), "{level}: {main}");
    }
}
