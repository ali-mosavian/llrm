//! Builds wccq, the Open Watcom front end llrm-c records C through, into
//! OUT_DIR. The first build also clones and bootstraps Open Watcom (see
//! owshim/build.sh). owshim/bin/wccq links to it for the Python reference.
//! Also builds jwasm and jwlink (tools/jwbuild.sh). All of it only with the
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
    for input in ["owshim/build.sh", "owshim/cgshim.c", "owshim/cc-objects.txt", "owshim/patches"] {
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rerun-if-env-changed=OWROOT");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("owshim");
    let status = Command::new("sh").arg("owshim/build.sh").arg(&out).status().expect("could not start sh");
    assert!(status.success(), "owshim/build.sh failed: {status}");
    let wccq = out.join("wccq");
    println!("cargo:rustc-env=LLRM_WCCQ={}", wccq.display());

    let link = PathBuf::from("owshim/bin/wccq");
    std::fs::create_dir_all("owshim/bin").unwrap();
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&wccq, &link).unwrap();

    // jwasm and jwlink beside llrm's own binaries, in target/<profile>.
    println!("cargo:rerun-if-changed=tools/jwbuild.sh");
    let profile = PathBuf::from(std::env::var("OUT_DIR").unwrap()).ancestors().nth(3).unwrap().to_path_buf();
    let status = Command::new("sh").arg("tools/jwbuild.sh").arg(&profile).status().expect("could not start sh");
    assert!(status.success(), "tools/jwbuild.sh failed: {status}");

    // The headless DOSBox-X the e2e tests run on, there too.
    println!("cargo:rerun-if-changed=tools/dosrunbuild.sh");
    let status = Command::new("sh").arg("tools/dosrunbuild.sh").arg(&profile).status().expect("could not start sh");
    assert!(status.success(), "tools/dosrunbuild.sh failed: {status}");
}
