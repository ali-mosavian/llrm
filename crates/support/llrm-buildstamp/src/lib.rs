//! The build script half of `--version` (`llrm_support::version`): every compiler binary's package calls `emit` from its
//! build script, so llrm-llc and each backend's llc print one stamp from one reading of the tree.

use std::process::Command;

/// The commit and whether the tree had tracked changes, for `--version`
/// (`LLRM_BUILD_SHA`, `LLRM_BUILD_DIRTY`). A build outside a git tree says
/// `unknown`.
pub fn emit() {
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
        if let Some(out) = git(&["rev-parse", "--path-format=absolute", "--git-path", path]) {
            println!("cargo:rerun-if-changed={}", String::from_utf8_lossy(&out).trim());
        }
    }
    // Any tracked file of the workspace changes the dirty flag, whichever package this is.
    if let Some(top) = git(&["rev-parse", "--show-toplevel"]) {
        let top = String::from_utf8_lossy(&top).trim().to_owned();
        for dir in ["src", "crates", "Cargo.toml"] {
            println!("cargo:rerun-if-changed={top}/{dir}");
        }
    }
}
