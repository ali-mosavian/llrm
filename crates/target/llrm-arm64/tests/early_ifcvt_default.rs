//! `EarlyIfConverter` runs by default (`aarch64-enable-early-ifcvt` is on).

fn selects(ifcvt: Option<&str>) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/early_ifcvt_mandel.ll")).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    command.args(["-S", "-O1", "-o"]).arg(&output).arg(&input).env_remove("LLRM_ARM64_IFCVT");
    if let Some(mode) = ifcvt {
        command.env("LLRM_ARM64_IFCVT", mode);
    }
    let said = command.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    asm.lines().filter(|line| line.trim_start().starts_with("csel")).count()
}

/// mandel's escape test left a five-instruction diamond as a branch (arm64
/// -O1: 222,591 clocks against clang's 188,827); it is two `csel`s.
#[test]
fn mandels_escape_diamond_is_selects() {
    assert_eq!(selects(Some("off")), 0, "the control: the pass off keeps the branch");
    assert!(selects(None) >= 2);
}
