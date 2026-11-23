//! The IR a stage dump ends with is the module the object was made from. A -Os
//! trial compiles the module again with a decision reversed and puts it back
//! where the object is not smaller; every compile wrote the stages
//! (`LLRM_MIR_STAGES`), so after a put-back trial the last stage was the
//! reversed one (QB operands.c: `hot_int` a call in the dump, inlined in the
//! object, and every comparison made on the dump wrong).

use std::path::Path;
use std::process::Command;

fn last_stage(dir: &Path) -> String {
    let mut stages: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|one| one.ok().map(|one| one.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "ll"))
        .filter_map(|path| {
            let number = path.file_name()?.to_str()?.split('-').next()?.parse().ok()?;
            Some((number, path))
        })
        .collect();
    stages.sort();
    std::fs::read_to_string(&stages.last().expect("a stage was dumped").1).unwrap()
}

/// The assembly and the last dumped stage of `data/bench_shellsort.ll` at -Os,
/// with the trials or, `trials` false, without.
fn compiled(trials: bool) -> (String, String) {
    let dir = tempfile::tempdir().unwrap();
    let (output, stages) = (dir.path().join("t.s"), dir.path().join("d"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    command.env("LLRM_MIR_STAGES", &stages);
    if !trials {
        command.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = command
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/bench_shellsort.ll"))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    (std::fs::read_to_string(&output).unwrap(), last_stage(&stages))
}

#[test]
fn the_last_dumped_stage_is_the_module_the_object_was_made_from() {
    let (object, dumped) = compiled(true);
    let (plain_object, plain_dumped) = compiled(false);
    assert_eq!(object, plain_object, "the premise: the trials are put back, the object is the first compile's");
    assert_eq!(dumped, plain_dumped, "the last stage is the put-back trial's module, not the object's");
}
