//! `sbb x, x` after the undef-input pass: both reads name the destination.

use std::process::Command;

/// CRC-32 bit loop at m32 -O2 (dag): the mask is `sbb x, x`, whose inputs are
/// undef. The rewriter gave an undef read with no register the first of the
/// class, so the source was `eax` beside a destination `edx`
/// (`sbb edx, eax`) and the checksum came out 1309022537, not -873187034.
#[test]
fn an_undef_sbb_source_is_the_destination_register() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/crc_sbb_mask.ll");
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("x.s");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-m32", "-O2", "-march=i486", "--isel=dag", "-S", "-o"])
        .arg(&out)
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    let sbbs: Vec<&str> = text.lines().map(str::trim).filter(|line| line.starts_with("sbb ")).collect();
    assert!(!sbbs.is_empty(), "{text}");
    for line in sbbs {
        let (dest, source) = line["sbb ".len()..].split_once(", ").unwrap();
        assert_eq!(dest, source, "{line}");
    }
}
