//! x86-64 -O2: a 128-bit integer vector shifted by an amount that is no
//! constant splat is `LowerShift`'s sequence of vector instructions, not one
//! scalar shift a lane. Before, the legalizer refused the node (`Unsupported {
//! op: Shl, ty: V8I16, "convert node to libcall" }`) and the whole file failed
//! to compile. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_shifts.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

/// Each function is the instructions of its pin sequence and no scalar shift,
/// spill or lane move.
#[test]
fn a_vector_shift_is_vector_instructions_and_no_scalar_ones() {
    let text = assembly();
    let expected: [(&str, &[&str]); 15] = [
        ("words_by_vector", &["psllw", "psraw", "pand", "pandn", "por"]),
        ("dwords_by_vector", &["pshuflw", "psrld", "punpckldq", "punpckhdq"]),
        ("bytes_by_vector", &["pcmpgtb", "psrlw", "pandn"]),
        ("signed_bytes_by_vector", &["pcmpgtw", "psraw", "packuswb"]),
        ("quads_by_vector", &["psrlq", "pshufd", "punpcklqdq"]),
        ("signed_quads_by_vector", &["psrlq", "pxor", "psubq"]),
        ("bytes_by_constant", &["psllw", "pand"]),
        ("signed_bytes_by_constant", &["psrlw", "pxor", "psubb"]),
        ("words_by_constants", &["pmullw"]),
        ("unsigned_words_by_constants", &["pmulhuw"]),
        ("dwords_by_two_constants", &["psrld", "punpcklqdq"]),
        ("words_by_splat", &["movd", "psllw"]),
        ("bytes_by_splat", &["movd", "psllw", "pmullw"]),
        ("bytes_by_splat", &["pand"]),
        ("dwords_by_undef_splat", &["psrld"]),
    ];
    for (name, instructions) in expected {
        let text = body(&text, name);
        for instruction in instructions {
            assert!(text.contains(&format!("{instruction} ")), "{name}: no {instruction}\n{text}");
        }
        assert!(!text.contains("[rsp") && !text.contains("sar ") && !text.contains("shl "), "{name}:\n{text}");
    }
    // The splat of a variable is one `psllw` by a register, not the ladder.
    assert!(!body(&text, "words_by_splat").contains("pcmpgt"), "{text}");
    // The loop vectorizer priced a constant shift as a variable one (the loop
    // stayed scalar) when the amount carried no operand information.
    assert!(body(&text, "shift_loop").contains("psrld "), "{text}");
    // A constant splat with an undef lane is still one immediate shift: it was
    // four shifts and four shuffles.
    let undef = body(&text, "dwords_by_undef_splat");
    assert!(!undef.contains("pshufd") && !undef.contains("punpck"), "{undef}");
}
