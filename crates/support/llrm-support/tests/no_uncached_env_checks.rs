//! `std::env::var_os` takes the environment lock and scans the environment on every call: the `LLRM_CHECK_*` switches
//! asked once for each instruction's liveness effect were 2% of a compile of rectwo. A switch is asked through
//! `llrm_support::env_set`, a lookup in a snapshot. (`llrm-mir` depends on nothing, so its switches are asked where
//! they are cold, and are not scanned.)

fn sources(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).expect("a directory reads").flatten() {
        let path = entry.path();
        let name = path.file_name().expect("a name").to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "target" | "tests" | "llrm-mir" | "llrm-support"
            ) {
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

#[test]
fn no_switch_is_asked_of_the_environment_by_name_in_a_loop() {
    let mut files = Vec::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), &mut files);
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
