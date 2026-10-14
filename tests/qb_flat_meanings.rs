//! The runtime's own bodies for ASC, LEN, MID$ and CHR$ are inlined at every
//! target (#456). At m32 `ASC` declared a 16-bit result where the body is of
//! the target's word, so its body was refused and each `ASC(MID$(s$, i, 1))`
//! called `B$FASC`: crc ran 1,533 instructions to m16's 1,217.

use std::process::Command;

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.bas"),
        "DEFINT A-Z\nd$ = \"123456789\"\nFOR i = 0 TO 8\ns = s + ASC(MID$(d$, i + 1, 1))\nNEXT\nPRINT s\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["a.bas", "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-S", "-o", "a.s"])
        .args(flags)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

fn runtime_calls(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("call") && (line.contains("B$FASC") || line.contains("B$FMID")))
        .collect()
}

#[test]
fn test_asc_of_mid_is_a_byte_load_at_m16_and_at_m32() {
    for flags in [&[][..], &["-m32"][..]] {
        let text = listing(flags);
        assert!(runtime_calls(&text).is_empty(), "{flags:?}: {:?}", runtime_calls(&text));
    }
}
