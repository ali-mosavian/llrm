//! A check or a trace switch is read once, where the call site names it
//! (`llrm_support::env_flag!`): `env_set` hashes the name and `var_os` scans
//! the environment at every call, and a call on a pass step's path is made
//! thousands of times (`env_set` was 0.4% of a d_faces compile, 112 sites).

use std::fs;
use std::path::Path;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn sources(
    dir: &Path,
    out: &mut Vec<String>,
) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path.strip_prefix(ROOT).unwrap().to_string_lossy().into_owned());
        }
    }
}

#[test]
fn test_no_source_reads_the_environment_at_every_call() {
    let mut files = Vec::new();
    sources(&Path::new(ROOT).join("crates"), &mut files);
    let mut found = Vec::new();
    for file in files.iter().filter(|file| {
        !file.ends_with("_tests.rs")
            && !file.ends_with("build.rs")
            && !file.ends_with("dumpdir.rs")
            && !file.ends_with("/tests.rs")
            && !file.contains("/tests/")
    }) {
        // Where the reading is: the switch is made here, or the mir crate (no
        // dependency on llrm-support) caches its own.
        let mir = file.starts_with("crates/ir/llrm-mir/");
        for (number, line) in fs::read_to_string(Path::new(ROOT).join(file)).unwrap().lines().enumerate() {
            let code = line.split("//").next().unwrap();
            let literal_env_set = code.contains("env_set(\"");
            let uncached_var = mir && code.contains("env::var_os(\"") && !code.contains("get_or_init");
            if literal_env_set || uncached_var {
                found.push(format!("{file}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "use llrm_support::env_flag!(\"NAME\") (llrm-mir: a OnceLock):\n{}", found.join("\n"));
}
