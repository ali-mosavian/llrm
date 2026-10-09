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
    for input in [
        "../../../toolchain/cache.sh",
        "../../../toolchain/owshim/build.sh",
        "../../../toolchain/owshim/hash.sh",
        "../../../toolchain/owshim/ow-commit",
        "../../../toolchain/owshim/cgshim.c",
        "../../../toolchain/owshim/cc-objects.txt",
        "../../../toolchain/owshim/patches",
        "../../../runtime/c",
        "../../../crates/target",
    ] {
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rerun-if-env-changed=OWROOT");
    // One front end for each Open Watcom tree the targets' C runtime
    // descriptions name.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("owshim");
    // What each tree is asked to be: the sizes its target's description states,
    // by the CPU the target names.
    let mut trees: std::collections::BTreeMap<String, [String; 4]> = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir("../../../runtime/c").expect("runtime/c").flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("c.toml")) else { continue };
        let table: toml::Table =
            text.parse().unwrap_or_else(|error| panic!("{}: {error}", entry.path().join("c.toml").display()));
        let frontend = table.get("frontend").unwrap_or_else(|| panic!("{} has no [frontend]", entry.path().display()));
        let cpu = frontend
            .get("watcom_cpu")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{} has no frontend.watcom_cpu", entry.path().display()));
        let int = frontend
            .get("int_bytes")
            .and_then(toml::Value::as_integer)
            .unwrap_or_else(|| panic!("{} has no frontend.int_bytes", entry.path().display()));
        let target = entry.file_name().to_string_lossy().into_owned();
        let layout_file = format!("../../target/llrm-{target}/src/machines/datalayout.toml");
        println!("cargo:rerun-if-changed={layout_file}");
        let layout = llrm_target::layout::Layout::parse(
            &std::fs::read_to_string(&layout_file).unwrap_or_else(|error| panic!("{layout_file}: {error}")),
        )
        .unwrap_or_else(|error| panic!("{layout_file}: {error}"));
        let data = llrm_mir::datalayout::DataLayout::parse(&layout.datalayout).expect("a target's datalayout parses");
        let bytes = |space: u32| (data.pointer(space).bits / 8).to_string();
        let sizes = [
            u32::from(layout.spaces.far_is_near()).to_string(),
            bytes(layout.spaces.near),
            bytes(layout.spaces.far),
            int.to_string(),
        ];
        if let Some(had) = trees.insert(cpu.to_owned(), sizes.clone()) {
            assert_eq!(had, sizes, "two targets share the {cpu} front end and state different sizes");
        }
    }
    for (cpu, [flat, near, far, int]) in &trees {
        let status = Command::new("sh")
            .arg("../../../toolchain/owshim/build.sh")
            .arg(out.join(cpu))
            .env("OWCPU", cpu)
            .env("LLRM_FLAT", flat)
            .env("LLRM_NEAR_BYTES", near)
            .env("LLRM_FAR_BYTES", far)
            .env("LLRM_INT_BYTES", int)
            .status()
            .expect("could not start sh");
        assert!(status.success(), "OWCPU={cpu} ../../../toolchain/owshim/build.sh failed: {status}");
    }
    println!("cargo:rustc-env=LLRM_WCCQ_DIR={}", out.display());
}
