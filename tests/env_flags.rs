//! `LLRM_CHECK_X=0` turned the check on: a switch was tested for presence, so
//! `=0`, `=false` and `=` ran the oracle as `=1` did (#2279). Seen by
//! `LLRM_DEBUG=checks`, which prints a `[checks] NAME n` line for each oracle
//! the compile reached.

mod common;

/// The oracles llrm-c reached compiling a small program with
/// `LLRM_CHECK_<name>` set to `value`.
///
/// llrm-cc: the command is `common::cc::llrm_cc()`, which names the target and
/// keeps `LLRM_OW` and the pinned clang that the old `env_remove` of every
/// `LLRM_*` variable dropped.
fn reached(
    name: &str,
    value: Option<&str>,
) -> Vec<String> {
    let directory = tempfile::tempdir().expect("a directory");
    let source = directory.path().join("t.c");
    std::fs::write(&source, "int f(int a) { int s = 0; while (a--) s += a * 3; return s; }\n")
        .expect("writes the source");
    let mut command = common::cc::llrm_cc();
    command.args(["-m32", "-O2", "-o", directory.path().join("t.obj").to_str().unwrap()]).arg(&source);
    // The target and the pinned toolchain stay; only the switches under test
    // go.
    for (key, _) in std::env::vars_os().filter(|(key, _)| {
        let key = key.to_string_lossy();
        key.starts_with("LLRM_CHECK_") || key == "LLRM_DEBUG"
    }) {
        command.env_remove(key);
    }
    command.env("LLRM_DEBUG", "checks");
    if let Some(value) = value {
        command.env(format!("LLRM_CHECK_{name}"), value);
    }
    let done = command.output().expect("runs llrm-cc");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stderr)
        .lines()
        .filter_map(|line| line.strip_prefix("[checks] "))
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

/// One switch of each reader: `STALE` is llrm-mir's own (`llrm_env`),
/// `LIVE` is asked through `llrm_support::env_flag!`.
#[test]
fn test_a_check_set_to_an_off_word_does_not_run_and_set_to_one_does() {
    for name in ["STALE", "LIVE"] {
        assert!(reached(name, Some("1")).iter().any(|one| one == name), "LLRM_CHECK_{name}=1 did not run it");
        for off in [None, Some("0"), Some("false"), Some("off"), Some("")] {
            assert!(!reached(name, off).iter().any(|one| one == name), "LLRM_CHECK_{name}={off:?} ran the oracle");
        }
    }
}
