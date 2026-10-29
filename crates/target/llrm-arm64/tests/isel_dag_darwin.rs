//! Every `tests/darwin` fixture through `--isel=shadow`, linked and run natively: it must
//! print and exit as recorded wherever the hand selector can compile it. Shadow mode falls
//! back per function, so this is the check that what the DAG path does take is right: it
//! once passed 9 of 27 (variadic arguments in registers, `sret` as a pointer in `x0`).

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn the_darwin_fixtures_run_as_recorded_under_shadow() {
    use std::process::Command;

    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/darwin");
    let llc = env!("CARGO_BIN_EXE_llrm-arm64-llc");
    let scratch = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut fixtures: Vec<_> = std::fs::read_dir(&here).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    fixtures.sort();
    for ir in fixtures.iter().filter(|p| p.extension().is_some_and(|e| e == "ll")) {
        let name = ir.file_stem().unwrap().to_str().unwrap();
        for level in ["--no-opt", "-O2"] {
            let object = |mode: &str| scratch.path().join(format!("{name}{level}{mode}.o"));
            let compile = |mode: &str| {
                Command::new(llc).args([level, &format!("--isel={mode}"), "-o"]).arg(object(mode)).arg(ir).output().unwrap()
            };
            // What the hand selector cannot compile is no test of the DAG path.
            if !compile("hand").status.success() {
                continue;
            }
            let said = compile("shadow");
            if !said.status.success() {
                failures.push(format!("{name} {level}: llc failed: {}", String::from_utf8_lossy(&said.stderr)));
                continue;
            }
            let exe = scratch.path().join(format!("{name}{level}"));
            let linked = Command::new("cc").arg(object("shadow")).arg("-o").arg(&exe).output().unwrap();
            assert!(linked.status.success(), "{name}: {}", String::from_utf8_lossy(&linked.stderr));
            let ran = Command::new(&exe).output().unwrap();
            let want_out = std::fs::read(here.join(format!("{name}.out"))).unwrap_or_default();
            let want_status: i32 =
                std::fs::read_to_string(here.join(format!("{name}.status"))).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            checked += 1;
            if ran.stdout != want_out || ran.status.code() != Some(want_status) {
                failures.push(format!("{name} {level}: output or status differs (status {:?})", ran.status.code()));
            }
        }
    }
    assert!(checked >= 40, "only {checked} runs: the fixtures were not found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
