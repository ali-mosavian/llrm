//! `fcos` and `fln` of two statements were both emitted as `fln`'s bytes
//! (cos(ci) gave ln(ci)): the DAG route gives them one instruction address
//! and the inline sites were keyed by it.

use std::process::Command;

const SOURCE: &str = "dim ci as double\ndim li as double\ndim cv as double\ndim lv as double\nread ci, li\ncv = cos(ci)\nlv = log(li)\nif mkd$(cv) <> chr$(140) then\n    print \"FAIL c\"\nend if\nif mkd$(lv) <> chr$(140) then\n    print \"FAIL l\"\nend if\ndata 0.5, 2.0\n";

#[test]
fn cos_and_log_in_one_program_keep_their_own_bytes() {
    let scratch = tempfile::tempdir().unwrap();
    let (input, out) = (scratch.path().join("r.bas"), scratch.path().join("r.asm"));
    std::fs::write(&input, SOURCE).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-qb")).arg(&input).args(["-O2", "-S", "-o"]).arg(&out).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(out).unwrap();
    assert!(asm.contains("db 0d9h,0ffh"), "no fcos (d9 ff) emitted for cos(ci):\n{asm}");
    assert!(asm.contains("db 0d9h,0edh,0d9h,0c9h,0d9h,0f1h"), "no fln emitted for log(li):\n{asm}");
}
