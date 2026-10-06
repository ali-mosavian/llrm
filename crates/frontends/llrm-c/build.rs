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
    for input in ["../../../toolchain/cache.sh", "../../../toolchain/owshim/build.sh", "../../../toolchain/owshim/hash.sh", "../../../toolchain/owshim/ow-commit", "../../../toolchain/owshim/cgshim.c", "../../../toolchain/owshim/cc-objects.txt", "../../../toolchain/owshim/patches", "../../../runtime/c"] {
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rerun-if-env-changed=OWROOT");
    // One front end for each Open Watcom tree the targets' C runtime descriptions name.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("owshim");
    let mut cpus = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir("../../../runtime/c").expect("runtime/c").flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("c.toml")) else { continue };
        let table: toml::Table = text.parse().unwrap_or_else(|error| panic!("{}: {error}", entry.path().join("c.toml").display()));
        let cpu = table.get("frontend").and_then(|one| one.get("watcom_cpu")).and_then(toml::Value::as_str).unwrap_or_else(|| panic!("{} has no frontend.watcom_cpu", entry.path().display()));
        cpus.insert(cpu.to_owned());
    }
    for cpu in &cpus {
        let status = Command::new("sh").arg("../../../toolchain/owshim/build.sh").arg(out.join(cpu)).env("OWCPU", cpu).status().expect("could not start sh");
        assert!(status.success(), "OWCPU={cpu} ../../../toolchain/owshim/build.sh failed: {status}");
    }
    println!("cargo:rustc-env=LLRM_WCCQ_DIR={}", out.display());
}
