//! x86-64 -Os: a constant fill is the `rep stos` unit with the fewest bytes
//! (DEVIATE REP-STOS-UNIT). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rep-stos-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// A 700-byte frame object of 0xEE was `movabs rax, 0xEEEE..; mov ecx, 87; rep
/// stosq; mov dword [m], 0xEEEEEEEE` (28 B) where `mov al, -18; mov ecx, 700;
/// rep stosb` is 9 B (memory_expansion -Os, 31 functions of +15..23 B).
#[test]
fn a_frame_object_fill_at_size_is_a_rep_stosb_when_that_is_fewer_bytes() {
    let text = assembly("rep_stos_frame.c");
    assert!(text.contains("rep stosb"), "no rep stosb:\n{text}");
    assert!(!text.contains("movabs") && !text.contains("stosq"), "the quadword form:\n{text}");
}

/// A loop storing a copied byte and a constant became a `rep movs` and kept the constant stores as a 21 B loop
/// (`mov byte [rsp+rax], 0xee; inc; cmp; jne`), which the fill left over after the copy is judged again only at speed
/// (memory_expansion `lr<N>` -Os +4 B each, 31 functions).
#[test]
fn a_fill_left_beside_a_copied_store_is_a_rep_stos_at_size() {
    let text = assembly("rep_stos_fused.c");
    assert!(text.contains("rep movs"), "no rep movs:\n{text}");
    assert!(text.contains("rep stosb"), "the fill stayed a loop:\n{text}");
}

/// A 512-byte zero fill of an aligned array was `rep stosb` with ecx = 512
/// (the unit was chosen between the pin's quadword and a byte only); `rep
/// stosd`, ecx = 128, is the same 9 B and a quarter of the iterations (histo
/// -Os: +381 executed instructions over gcc).
#[test]
fn a_zero_fill_at_size_takes_the_widest_unit_of_the_fewest_bytes() {
    let text = assembly("rep_stos_zero.c");
    assert!(text.contains("rep stos"), "no rep stos:\n{text}");
    assert!(!text.contains("rep stosb"), "the byte form:\n{text}");
}
