//! A generated table with no open pattern predicate must not bind `ctx`: the checked-in AArch64 table failed
//! `-D warnings` (CI's setting) on the unused binding. Generated here from the M68k fixture record, which has none.
//! `DAGISEL_SKIP_PIN=1` skips the test where the pin is not installed.

use std::path::Path;
use std::process::Command;

#[test]
fn a_pattern_predicate_with_no_arm_does_not_bind_ctx() {
    if std::env::var_os("DAGISEL_SKIP_PIN").is_some() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let into = tempfile::tempdir().unwrap();
    let status = Command::new(root.join("tools/dagisel/regen-target.sh"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/m68k/dagisel.toml"))
        .arg(into.path())
        .env("DAGISEL_GEN", env!("CARGO_BIN_EXE_llrm-dagisel-gen"))
        .status()
        .expect("run regen-target.sh");
    assert!(status.success(), "regen-target.sh failed on the fixture record");
    let text = std::fs::read_to_string(into.path().join("m68k_m68000.rs")).unwrap();
    let at = text.find("pub fn pattern_predicate<").expect("the table has a pattern_predicate");
    let body = &text[at..];
    let body = &body[..body.find("\n}\n").expect("the function ends")];
    let arms = body.lines().filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit())).count();
    assert_eq!(arms, 0, "the premise: the fixture table has no open pattern predicate");
    assert!(body.lines().next().unwrap().contains("(_ctx:"), "ctx is bound but unused:\n{body}");
}
