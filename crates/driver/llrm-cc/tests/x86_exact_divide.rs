//! `BuildExactSDIV` and `BuildExactUDIV`: a pointer difference is an `exact`
//! divide by the element size, a shift and a multiply by the inverse of the odd
//! rest (`sar 4; imul` by 3's inverse for 48), not a hardware divide.
//! QuickrBasic's `flow_pret` took `idiv rcx` for a 48-byte frame (`mov ecx, 48;
//! cqo; idiv rcx`): twice the instructions of clang's, and a 64-bit divide each
//! call. Needs `LLRM_LLC`.

use std::process::Command;

fn compiled(target: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o", "-"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/exact_divide.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    String::from_utf8_lossy(&run.stdout).into_owned()
}

fn function(
    asm: &str,
    name: &str,
) -> String {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).collect::<Vec<_>>().join("\n")
}

#[test]
fn test_an_exact_divide_by_48_is_a_shift_and_a_multiply_at_x86_64() {
    let asm = compiled("x86_64-linux");
    let diff = function(&asm, "diff");
    assert!(!diff.contains("div"), "a hardware divide:\n{diff}");
    // 3's inverse modulo 2^64, after `sar rsi, 4`.
    assert!(diff.contains("sar rsi, 4") && diff.contains("-6148914691236517205"), "not sar and imul:\n{diff}");
    let count = function(&asm, "count");
    // 3's inverse modulo 2^32, after `shr rsi, 4`.
    assert!(count.contains("shr rsi, 4") && count.contains("imul eax, esi, -1431655765"), "not shr and imul:\n{count}");
    let negated = function(&asm, "negated");
    assert!(!negated.contains("idiv"), "a hardware divide:\n{negated}");
}

#[test]
fn test_an_exact_divide_by_48_is_no_hardware_divide_at_x86_32() {
    let asm = compiled("x86_32-linux");
    for name in ["diff", "count"] {
        let body = function(&asm, name);
        assert!(!body.contains("div"), "{name}: a hardware divide:\n{body}");
    }
}
