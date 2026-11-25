//! arm64 inline asm (`udiv`/`msub` with an early-clobber output, `rev ${0:w}`,
//! `fsqrt ${0:d}`, a memory output, a fixed `w0`) was refused by the backend
//! ("expected a constant, found asm"). A block is now one `llrm.a64.asm.*` call
//! whose words are laid down at the site. The gate needs the pin's AArch64
//! clang (found by `llrm-arm64`'s tests/support/clang.rs), the
//! AArch64 binutils objcopy and `llrm-arm64-llc` built beside the driver: a
//! missing tool fails the test.

#[path = "../../../target/llrm-arm64/tests/support/clang.rs"]
mod clang;

use std::path::Path;
use std::process::Command;

fn tool_present(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}

#[test]
fn test_arm64_asm_blocks_compute_what_the_source_says_at_every_level() {
    let clang = clang::clang()
        .expect("this test needs the pin clang built with AArch64, aarch64-linux-gnu-gcc and qemu-aarch64");
    assert!(tool_present("aarch64-linux-gnu-objcopy"), "this test needs aarch64-linux-gnu-objcopy");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/a64_inline_asm.c");
    for level in ["-O1", "-O2", "-Os"] {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("a64asm-{level}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let object = dir.join("a.o");
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "arm64-linux")
            .env("LLRM_A64_CLANG", &clang)
            .args([level, "-w", "-c", "-o"])
            .arg(&object)
            .arg(&source)
            .output()
            .unwrap();
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        let program = dir.join("a.out");
        let linked =
            Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{level}: {}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout), "14 2 44332211 1.50 77 42\n", "{level}");
    }
}

/// A block that clobbers `x19` or `d8` was refused by name ("clobber of
/// callee-saved register"). The registers it declares are defs of the block, so
/// the frame saves and restores them: the caller's values in them survive, and
/// the program prints what the source says.
#[test]
fn test_a_block_may_clobber_callee_saved_registers_and_the_frame_saves_them() {
    let clang = clang::clang().expect("this test needs the pin clang built with AArch64 and qemu-aarch64");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/a64_inline_asm_csr.c");
    for level in ["-O1", "-O2", "-Os"] {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("a64asm-csr{level}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (object, program) = (dir.join("a.o"), dir.join("a.out"));
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "arm64-linux")
            .env("LLRM_A64_CLANG", &clang)
            .args([level, "-w", "-c", "-o"])
            .arg(&object)
            .arg(&source)
            .output()
            .unwrap();
        assert!(done.status.success(), "{level}: {}", String::from_utf8_lossy(&done.stderr));
        let linked =
            Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{level}: {}", String::from_utf8_lossy(&linked.stderr));
        let ran = Command::new("qemu-aarch64").arg(&program).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout), "12 1001 3.0 1.5\n", "{level}");
    }
}

/// A block was a call to the allocator, so a pointer kept across `udiv`/`msub`
/// moved from `x2` to the callee-saved `x19` and the function saved and
/// restored it (`divmod`: `str x19`, `ldr x19`). The block now destroys what
/// it declares (here nothing but its outputs), as the pin's `INLINEASM` does.
#[test]
fn test_a_value_kept_across_an_asm_block_stays_in_its_register() {
    let clang = clang::clang().expect("this test needs the pin clang built with AArch64");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/a64_inline_asm.c");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("a64asm-keep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listing = dir.join("a.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "arm64-linux")
        .env("LLRM_A64_CLANG", &clang)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&listing)
        .arg(&source)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&listing).unwrap();
    let body: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("divmod:"))
        .take_while(|line| !line.starts_with("swap32:"))
        .collect();
    assert!(!body.iter().any(|line| line.contains("x19")), "{}", body.join("\n"));
}
