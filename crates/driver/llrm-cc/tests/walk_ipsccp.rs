//! A callee's argument range is stated from the bodies the walk inlined into
//! its callers: `sort(0, clamp(n))` bounds `hi` to 0..1023 only after `clamp`
//! is inlined, and the pre-walk IPSCCP left `range(i32 0, 2048)`. Needs
//! `LLRM_OW` and `LLRM_PIN_CLANG`.

use std::path::Path;
use std::process::Command;

#[test]
fn an_argument_bounded_by_an_inlined_helper_gets_the_narrow_range() {
    for name in ["LLRM_OW", "LLRM_PIN_CLANG"] {
        assert!(std::env::var_os(name).is_some(), "{name} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("walk-ipsccp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/walk_ipsccp.c");
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
    let sort = ir.lines().find(|line| line.starts_with("define") && line.contains("@sort(")).expect("no sort");
    assert!(sort.contains("range(i32 0, 1024)"), "{sort}");
}
