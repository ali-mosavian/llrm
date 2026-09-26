//! Builds wccq, the Open Watcom front end llrm-c records C through, into
//! OUT_DIR. The first build also clones and bootstraps Open Watcom (see
//! toolchain/owshim/build.sh). Only with the `toolchain` feature, whose
//! script needs a Unix host.

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
    for input in ["../../toolchain/owshim/build.sh", "../../toolchain/owshim/cgshim.c", "../../toolchain/owshim/cc-objects.txt", "../../toolchain/owshim/patches"] {
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rerun-if-env-changed=OWROOT");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("owshim");
    let status = Command::new("sh").arg("../../toolchain/owshim/build.sh").arg(&out).status().expect("could not start sh");
    assert!(status.success(), "../../toolchain/owshim/build.sh failed: {status}");
    let wccq = out.join("wccq");
    println!("cargo:rustc-env=LLRM_WCCQ={}", wccq.display());
}
