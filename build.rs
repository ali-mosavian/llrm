//! Builds jwasm and jwlink (toolchain/jwbuild.sh, feature `jw`) and the
//! headless DOSBox-X (toolchain/dosrunbuild.sh, feature `dosrun`) beside llrm's
//! binaries. The scripts need a Unix host.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    stamp();
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

/// The commit and whether the tree had tracked changes, for `--version`
/// (`LLRM_BUILD_SHA`, `LLRM_BUILD_DIRTY`). A build outside a git tree says
/// `unknown`.
fn stamp() {
    let git = |args: &[&str]| {
        Command::new("git").args(args).output().ok().filter(|out| out.status.success()).map(|out| out.stdout)
    };
    let sha = git(&["rev-parse", "HEAD"])
        .map(|out| String::from_utf8_lossy(&out).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|out| !out.is_empty());
    println!("cargo:rustc-env=LLRM_BUILD_SHA={sha}");
    println!("cargo:rustc-env=LLRM_BUILD_DIRTY={}", u8::from(dirty));
    // Cargo gives a build script no flag for incremental compilation:
    // CARGO_INCREMENTAL=0 in the environment (the cost gate's build) or the
    // `dist` profile are the cold builds, anything else is the profile's own
    // `incremental = true`. An incremental build runs ~13% more
    // instructions in the object writer (#5744 round 1), so a count measured on
    // one is not the gate's.
    println!("cargo:rerun-if-env-changed=CARGO_INCREMENTAL");
    let cold = std::env::var("CARGO_INCREMENTAL").is_ok_and(|value| value == "0")
        || std::env::var("OUT_DIR").is_ok_and(|dir| dir.contains("/dist/"));
    println!("cargo:rustc-env=LLRM_BUILD_INCREMENTAL={}", u8::from(!cold));
    // HEAD moves with a checkout; the sources change the dirty flag.
    for path in ["HEAD", "index"] {
        if let Some(out) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={}", String::from_utf8_lossy(&out).trim());
        }
    }
    for dir in ["src", "crates", "Cargo.toml"] {
        println!("cargo:rerun-if-changed={dir}");
    }
}
