//! A far pointer made from a segment in one block is made again where it is
//! read.

use std::process::Command;

const KERNEL: &str = "DEFINT A-Z
DECLARE SUB f (c AS INTEGER)
INPUT c
CALL f(c)
SUB f (c AS INTEGER)
DIM l(3) AS INTEGER
DIM k AS INTEGER
FOR k = 1 TO 3
  l(1) = SIN(k / 25) * 120
  l(2) = SIN(k / 35) * 110
  IF c > k THEN l(3) = SIN(k / 45) * 100
  PRINT l(1) + l(2) + l(3)
NEXT
END SUB
";

/// The array's far pointer is the segment and the offset 0, made in the entry
/// block and read in three: the zero crossed each edge in a register, so the
/// second and third stores were `es:[bx+4]` and `es:[bx+6]` behind an
/// `xor ebx, ebx` (deedlines COPPER +398 B under the DAG selector).
#[test]
fn test_an_array_element_in_a_later_block_is_addressed_without_a_zero_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.bas"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--isel=dag", "a.bas", "--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(asm.contains("es:[4]") && asm.contains("es:[6]"), "{asm}");
    assert!(!asm.contains("es:[bx+4]") && !asm.contains("es:[bx+6]"), "{asm}");
}
