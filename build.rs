//! Builds jwasm and jwlink (toolchain/jwbuild.sh) and the headless DOSBox-X
//! (toolchain/dosrunbuild.sh) beside llrm's binaries. Only with the
//! `toolchain` feature, whose scripts need a Unix host.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    if std::env::var_os("CARGO_FEATURE_TOOLCHAIN").is_some() {
        toolchain();
    }
}

#[cfg(not(unix))]
fn toolchain() {
    panic!("the toolchain feature needs a Unix host; build with --no-default-features");
}

#[cfg(unix)]
fn toolchain() {
    // jwasm and jwlink beside llrm's own binaries, in target/<profile>.
    println!("cargo:rerun-if-changed=toolchain/jwbuild.sh");
    let profile = PathBuf::from(std::env::var("OUT_DIR").unwrap()).ancestors().nth(3).unwrap().to_path_buf();
    let status = Command::new("sh").arg("toolchain/jwbuild.sh").arg(&profile).status().expect("could not start sh");
    assert!(status.success(), "toolchain/jwbuild.sh failed: {status}");

    // The headless DOSBox-X the e2e tests run on, there too.
    println!("cargo:rerun-if-changed=toolchain/dosrunbuild.sh");
    let status = Command::new("sh").arg("toolchain/dosrunbuild.sh").arg(&profile).status().expect("could not start sh");
    assert!(status.success(), "toolchain/dosrunbuild.sh failed: {status}");
}
