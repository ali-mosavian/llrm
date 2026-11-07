//! SD443's second IPSCCP folds `@check_locals.y` and `@src` into `lr1`, whose
//! address `check_locals` took until inlining made its call direct; with one
//! IPSCCP (the pin's) the arguments stay and `memory_expansion` is 1332 B
//! bigger at m32 -O2. The positive control of the SD443 A/B: the bench corpus
//! never reached the pass. Needs the Open Watcom tree (tools/ow-fetch.sh) and
//! `LLRM_PIN_CLANG`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_function_called_with_constants_after_inlining_loses_its_parameters_at_m32() {
    assert!(
        llrm_cc::ow_tree().is_dir(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_PIN_CLANG").is_some(),
        "LLRM_PIN_CLANG is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-ipsccp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/second_ipsccp.c");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-O2", "-w", "-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(source)
        .arg("--dump")
        .arg(dir.join("stages"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let stage = std::fs::read_dir(dir.join("stages"))
        .unwrap()
        .map(|one| one.unwrap().path())
        .find(|path| path.file_name().unwrap().to_string_lossy().ends_with("-interprocedural.ll"))
        .expect("no interprocedural stage dumped");
    let ir = std::fs::read_to_string(stage).unwrap();
    assert!(
        ir.contains("@lr1()"),
        "lr1 kept its parameters:\n{}",
        ir.lines().filter(|line| line.contains("@lr1(")).collect::<Vec<_>>().join("\n")
    );
}
