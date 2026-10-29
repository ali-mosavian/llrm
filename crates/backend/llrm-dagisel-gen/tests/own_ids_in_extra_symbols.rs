//! An id in `own_ids` that `extra_symbols` also lists reached the C++ ids
//! program, which cannot name it (a target with no LLVM header, 6502, broke
//! on it). `DAGISEL_SKIP_PIN=1` skips the test where the pin is not installed.

use std::path::Path;
use std::process::Command;

#[test]
fn an_own_id_listed_in_extra_symbols_is_not_looked_up_in_the_headers() {
    if std::env::var_os("DAGISEL_SKIP_PIN").is_some() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let record = std::fs::read_to_string(root.join("crates/target/llrm-m68k/dagisel.toml")).unwrap();
    let record = record.replacen("extra_symbols = [", "own_ids = \"own.txt\"\nextra_symbols = [\"M68kISD::Fake\", ", 1);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dagisel.toml"), record).unwrap();
    std::fs::write(dir.path().join("own.txt"), "M68kISD::Fake 777\n").unwrap();

    let out = Command::new(root.join("tools/dagisel/regen-target.sh"))
        .arg(dir.path().join("dagisel.toml"))
        .arg(dir.path().join("out"))
        .env("DAGISEL_GEN", env!("CARGO_BIN_EXE_llrm-dagisel-gen"))
        .output()
        .expect("run regen-target.sh");

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr).lines().take(4).collect::<Vec<_>>().join("\n")
    );
}
