//! A QB SUB's locals are zero in the runtime-built frame: the DAG path does not
//! write them again.

use std::process::Command;

const KERNEL: &str = "DEFINT A-Z
DECLARE SUB Locals (n, m)
ON ERROR GOTO handler
Locals 6, 9
END
handler:
RESUME NEXT
SUB Locals (n, m)
    DIM a(10 TO 99)
    REDIM b(n, 5 TO m)
    t& = 0
    FOR i = LBOUND(a) TO UBOUND(a)
        t& = t& + i
    NEXT
    PRINT \"local\"; t&; UBOUND(b, 2) - LBOUND(b, 1); UBOUND(b, 1); LBOUND(b, 2)
    FOR k = 1 TO 3
        REDIM c(k TO 2 * k, 1 TO k)
        PRINT \"loop\"; LBOUND(c); UBOUND(c)
    NEXT
END SUB
";

fn listing(isel: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args([isel, "a.bas", "--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout).into_owned();
    asm.split("LOCALS proc").nth(1).and_then(|rest| rest.split("LOCALS endp").next()).expect("LOCALS").to_owned()
}

/// B$ENRA zero-fills the frame, so the entry block's `store 0` / `memset 0` to
/// a fresh local is already done. The DAG path kept them: bounds LOCALS began
/// with `xor edx, edx`, a run of `mov [bp-N], dx` and two `rep stosb` (+192 B;
/// the `mov` +88 / `xor` +21 of every QB SUB with locals). The old selector has
/// none.
#[test]
fn test_a_local_the_runtime_zeroed_is_not_zeroed_again() {
    for isel in ["--isel=old", "--isel=dag"] {
        let body = listing(isel);
        let prologue = body.split("B$DDIM").next().unwrap();
        assert!(!prologue.contains("rep stos") && !prologue.contains("xor edx, edx"), "{isel}: {prologue}");
    }
}
