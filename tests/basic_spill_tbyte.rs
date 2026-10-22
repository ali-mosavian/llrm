//! A BASIC double temporary spills at the register's 80 bits.

use std::process::Command;

const PROGRAM: &str = "DIM d AS DOUBLE
FOR i% = -7 TO 7
    d = 99999999999999.95# * 10# ^ i%
    PRINT d
NEXT
";

/// `10# ^ i%` over an unrolled loop spilled log2(10) as a qword: the first
/// copy read the 80-bit register, the other fourteen the rounded reload, and
/// ` 9999999.999999994` printed as ` 10000000` (real_print_wide, D-RFP5).
#[test]
fn test_a_basic_double_temporary_spills_with_fstp_tbyte_and_reloads_with_fld_tbyte() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), PROGRAM).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["-O3", "-S", "-o", "a.s", "a.bas"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("fstp tbyte ptr"), "{text}");
    assert!(text.contains("fld tbyte ptr"), "{text}");
}
