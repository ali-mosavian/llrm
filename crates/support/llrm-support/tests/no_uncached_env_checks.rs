//! `std::env::var_os` takes the environment lock and scans the environment on
//! every call: the `LLRM_CHECK_*` switches asked once for each instruction's
//! liveness effect were 2% of a compile of rectwo. A switch is asked through
//! `llrm_support::env_set`, a lookup in a snapshot. (`llrm-mir` depends on
//! nothing, so its switches are asked where they are cold, and are not
//! scanned.)

fn sources(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).expect("a directory reads").flatten() {
        let path = entry.path();
        let name = path.file_name().expect("a name").to_string_lossy().into_owned();
        if path.is_dir() {
            // A build's `target` sits beside a `Cargo.toml`; `crates/target`
            // holds the targets' sources.
            let build = name == "target" && dir.join("Cargo.toml").exists();
            if !build && !matches!(name.as_str(), "tests" | "llrm-mir" | "llrm-support") {
                sources(&path, out);
            }
        } else if name.ends_with(".rs")
            && !name.ends_with("_tests.rs")
            && !name.starts_with("test_")
            && name != "build.rs"
        {
            out.push(path);
        }
    }
}

fn crate_sources() -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), &mut files);
    files
}

/// The walk skipped every directory named `target`, so the whole of
/// `crates/target` went unscanned and 44 uncached switches passed.
#[test]
fn the_scan_reaches_the_target_crates() {
    let driver = std::path::Path::new("target/llrm-6502/src/driver.rs");
    assert!(crate_sources().iter().any(|path| path.ends_with(driver)));
}

#[test]
fn no_switch_is_asked_of_the_environment_by_name_in_a_loop() {
    let files = crate_sources();
    let found: Vec<String> = files
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path)
                .expect("a source reads")
                .lines()
                .any(
                    |line| !line.trim_start().starts_with("//")
                        && line.contains("env::var_os(\"LLRM_")
                        && (line.contains(".is_some()") || line.contains(".is_none()")),
                )
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(found.is_empty(), "ask llrm_support::env_set: {found:?}");
}
