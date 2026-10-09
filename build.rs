//! Builds jwasm and jwlink (toolchain/jwbuild.sh, feature `jw`) and the headless
//! DOSBox-X (toolchain/dosrunbuild.sh, feature `dosrun`) beside llrm's
//! binaries. The scripts need a Unix host.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    for (feature, script) in [("JW", "jwbuild.sh"), ("DOSRUN", "dosrunbuild.sh")] {
        if std::env::var_os(format!("CARGO_FEATURE_{feature}")).is_some() {
            toolchain(script);
        }
    }
}

#[cfg(not(unix))]
fn toolchain(_: &str) {
    panic!("the jw and dosrun features need a Unix host; build with --no-default-features");
}

#[cfg(unix)]
fn toolchain(script: &str) {
    // Beside llrm's own binaries, in target/<profile>.
    println!("cargo:rerun-if-changed=toolchain/{script}");
    println!("cargo:rerun-if-changed=toolchain/cache.sh");
    let profile = PathBuf::from(std::env::var("OUT_DIR").unwrap()).ancestors().nth(3).unwrap().to_path_buf();
    let status =
        Command::new("sh").arg(format!("toolchain/{script}")).arg(&profile).status().expect("could not start sh");
    assert!(status.success(), "toolchain/{script} failed: {status}");
}
