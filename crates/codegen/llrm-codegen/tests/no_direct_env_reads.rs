//! `std::env::var` takes the environment lock and scans it on every call. Seven
//! debug probes asked once per function cost 2.3 M instructions in a compile of
//! d_faces (0.26%, 2.5% of instruction selection). Every `LLRM_*` variable is
//! read through `llrm_support::env_var` / `env_set`, a snapshot.

fn sources(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).expect("a directory reads").flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") && !path.to_string_lossy().ends_with("_tests.rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_llrm_variable_is_read_from_the_environment_by_name() {
    let mut files = Vec::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let found: Vec<String> = files
        .iter()
        .flat_map(|path| {
            std::fs::read_to_string(path)
                .expect("a source reads")
                .lines()
                .enumerate()
                .filter(|(_, line)| {
                    !line.trim_start().starts_with("//")
                        && (line.contains("env::var(\"LLRM_") || line.contains("env::var_os(\"LLRM_"))
                })
                .map(|(at, _)| format!("{}:{}", path.display(), at + 1))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(found.is_empty(), "ask llrm_support::env_var or env_set: {found:#?}");
}
