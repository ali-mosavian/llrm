//! `ad_copy` of the QB runtime copies an array into a fresh one (REDIM
//! PRESERVE): a block copy, not the byte loop the C source was. At -m32 it was
//! `movzx eax, byte ptr [..]; mov byte ptr [..], al` per byte; at -m16, the
//! same over two far pointers.
//!
//! llrm-cc: the sources are compiled with the `dev/` headers on the include
//! path, and the m32 one declares `str_free_bytes` (clang rejects the implicit
//! declaration Watcom took).

mod common;

fn listing(
    directory: &str,
    flags: &[&str],
) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-O2", "-S", "-o", "ad.s"])
        .args(["-I", &format!("{root}/runtime/qb/dos/{directory}")])
        .args([
            "-I",
            &format!("{root}/runtime/qb/dev/dos/{directory}"),
            "-I",
            &format!("{root}/runtime/qb/dev"),
            "-I",
            &format!("{root}/runtime/qb/dos"),
            "-I",
            &format!("{root}/runtime/qb"),
        ])
        .arg(format!("{root}/runtime/qb/dos/{directory}/ad.c"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("ad.s")).unwrap()
}

/// The text of `ad_copy`, from its `proc` to its `endp`.
fn ad_copy(text: &str) -> String {
    let start = text.lines().position(|line| line.contains("ad_copy") && line.contains(" proc")).expect("ad_copy");
    text.lines()
        .skip(start)
        .take_while(|line| !(line.contains("ad_copy") && line.contains(" endp")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn m32_ad_copy_calls_the_block_copy() {
    let body = ad_copy(&listing("m32", &["-m32", "-mabi=sysv", "-march=i486"]));
    assert!(body.contains("B$COPY"), "{body}");
    assert!(!body.contains("byte ptr"), "{body}");
}

#[test]
fn m16_ad_copy_calls_the_block_move() {
    let body = ad_copy(&listing("m16", &["-m16", "-march=i386"]));
    assert!(body.contains("B$MOVE"), "{body}");
    assert!(!body.contains("byte ptr es:") && !body.contains("byte ptr fs:"), "{body}");
}
