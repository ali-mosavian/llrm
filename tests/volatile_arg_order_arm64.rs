//! A call whose arguments are volatile loads reads them in source order, at
//! every -O level (the m68k miscompile class). Run on arm64 Linux under
//! qemu-aarch64 with a PROT_NONE page per global; the fault order is the access
//! order. The reversed source is the control: it must print the reverse, or
//! the instrument sees nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sysroot() -> PathBuf {
    let made = Command::new(root().join("tools/targets/build-arm64-sysroot.sh"))
        .output()
        .expect("tools/targets/build-arm64-sysroot.sh runs");
    assert!(made.status.success(), "no arm64 sysroot: {}", String::from_utf8_lossy(&made.stderr));
    PathBuf::from(String::from_utf8_lossy(&made.stdout).trim())
}

fn observed(
    level: &str,
    reversed: bool,
) -> String {
    let out = std::env::temp_dir().join(format!("volatile_order_{level}_{reversed}_{}", std::process::id()));
    let mut build = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
    build.args(["-target", "arm64-glibc", "-w", &format!("-O{level}")]);
    if reversed {
        build.arg("-DREVERSED");
    }
    build.env("LLRM_ARM64_SYSROOT", sysroot());
    build.arg(root().join("tests/volatile_args/order.c")).arg("-o").arg(&out);
    let built = build.output().expect("llrm-cc runs");
    assert!(built.status.success(), "-O{level}: {}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new("qemu-aarch64").arg(&out).output().expect("qemu-aarch64 runs");
    std::fs::remove_file(&out).ok();
    String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

#[test]
fn volatile_arguments_are_read_in_source_order_at_every_level() {
    for level in ["0", "1", "2", "3", "s"] {
        assert_eq!(observed(level, false), "args=123 order=123", "-O{level}");
    }
}

/// The control: the same call written the other way round must be seen the
/// other way round, so that the fault log can tell the orders apart.
#[test]
fn the_reversed_call_is_observed_reversed() {
    assert_eq!(observed("2", true), "args=321 order=321");
}
