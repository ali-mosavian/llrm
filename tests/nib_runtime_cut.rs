//! `runtime.nib --used-by PROGRAM.obj` for a program that names no runtime routine: the cut runtime has no export and
//! no entry, and tools/dosbatch could not link such a Nib program (#747).

use std::process::Command;

#[test]
fn test_the_runtime_cut_for_a_program_that_names_none_of_it_is_made() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("p.nib");
    std::fs::write(
        &source,
        "@extern(\"c\")\nfn report(x: i32) -> i32\n\nfn main() -> i32:\n    unsafe:\n        return report(40 + 2)\n",
    )
    .unwrap();
    let compiler = env!("CARGO_BIN_EXE_llrm-nib");
    let program = directory.path().join("p.obj");
    let made = Command::new(compiler).args(["-m32", "-o"]).arg(&program).arg(&source).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let runtime =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/frontends/llrm-nib/src/runtime/runtime.nib");
    let cut = Command::new(compiler)
        .args(["-m32", "-o"])
        .arg(directory.path().join("rt.obj"))
        .arg(&runtime)
        .arg("--used-by")
        .arg(&program)
        .output()
        .unwrap();
    assert!(cut.status.success(), "{}", String::from_utf8_lossy(&cut.stderr));
}
