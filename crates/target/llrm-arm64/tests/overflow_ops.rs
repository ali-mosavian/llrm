//! `{s,u}{add,sub}.with.overflow` is `adds`/`subs` and one `cset` or `b.cc`
//! (`LowerXALUO`); before, the legalizer refused it (`text_write`).

use std::process::Command;

fn body(name: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/overflow_ops.ll");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(ir)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// The legalizer refused `sadd.with.overflow` ("custom lowering of SAddO"):
/// `text_write`.
#[test]
fn a_signed_add_overflow_bit_is_adds_and_a_csinc_on_vc() {
    let text = body("ao");
    assert!(text.contains("adds w9, w0, w1"), "{text}");
    assert!(text.contains("vc"), "{text}");
}

/// `xor (uadd.with.overflow).1, true` is one `csinc` on `hs`; it was an `adds`,
/// a `csinc` on `lo` and an `eor`.
#[test]
fn an_inverted_unsigned_add_overflow_bit_is_one_csinc_on_the_inverted_condition() {
    let text = body("no");
    assert!(text.contains("adds xzr, x0, x1"), "{text}");
    assert!(text.contains("csinc w0, wzr, wzr, hs"), "{text}");
    assert!(!text.contains("eor"), "{text}");
}
