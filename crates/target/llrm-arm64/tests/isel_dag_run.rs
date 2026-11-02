//! Programs with loops, diamonds, phis and every integer compare, compiled
//! with `--isel=dag` and run: the answers are C's.

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn the_dag_path_computes_what_c_computes() {
    for level in ["--no-opt"] {
        let scratch = tempfile::tempdir().unwrap();
        let dir = scratch.path();
        let llc = std::path::Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
        let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/dag_run.ll");
        let object = dir.join("f.o");
        let said =
            std::process::Command::new(llc).args([level, "--isel=dag", "-o"]).arg(&object).arg(ir).output().unwrap();
        assert!(said.status.success(), "{level}: {}", String::from_utf8_lossy(&said.stderr));
        let driver = dir.join("main.c");
        std::fs::write(&driver, include_str!("data/dag_run.c")).unwrap();
        let program = dir.join("prog");
        let built =
            std::process::Command::new("cc").arg(&driver).arg(&object).arg("-o").arg(&program).output().unwrap();
        assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
        let ran = std::process::Command::new(&program).output().unwrap();
        assert!(ran.status.success(), "{level}: {}", String::from_utf8_lossy(&ran.stdout));
    }
}
