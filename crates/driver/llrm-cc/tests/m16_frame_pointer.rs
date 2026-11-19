//! A C function with no locals that only calls got `push bp; mov bp, sp` ..
//! `pop bp` at m16: the code16 triple has clang tag every function
//! `"frame-pointer"="all"`, which the backend keeps (`MachineFunction::
//! disableFramePointerElim`). Bar left it out; sieve, huge, mandel, parity,
//! qmove and ring each grew 4 B at -Os.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn a_function_with_no_locals_has_no_frame_at_m16() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("frame-{}.c", std::process::id()));
    std::fs::write(&source, "int f(int);\nint main(void) { return f(1024); }\n").unwrap();
    for target in ["x86_16-dos", "x86_16-wat"] {
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", target)
            .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
            .args(["-Os", "-w", "-c", "-S", "-march=i386", "-o", "-"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout);
        assert!(!asm.contains("push bp"), "{target}: a frame with nothing in it:\n{asm}");
    }
}

/// -Os opened a frame with locals as `push bp; mov bp, sp; sub sp, N` (7 B for N over 127): sieve was 5 B above Open Watcom, which
/// uses the 4-byte `enter N, 0`.
#[test]
fn a_size_frame_with_locals_opens_with_enter_at_m16() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("enter-{}.c", std::process::id()));
    std::fs::write(
        &source,
        "extern void use(char *);\nint f(int n) { char buf[1024]; buf[n] = 1; use(buf); return buf[0]; }\n",
    )
    .unwrap();
    for (level, entered) in [("-Os", true), ("-O2", false)] {
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_16-wat")
            .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
            .args([level, "-w", "-c", "-S", "-march=i386", "-o", "-"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let asm = String::from_utf8_lossy(&out.stdout);
        assert_eq!(asm.contains("enter 1024, 0"), entered, "{level}:\n{asm}");
    }
}
