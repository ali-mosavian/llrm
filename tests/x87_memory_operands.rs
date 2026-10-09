//! A float load that only one arithmetic instruction reads is that instruction's memory operand.

use std::process::Command;

const KERNEL: &str = "
double f(double p, double *q) { return p * q[0] + q[1]; }
float g(float p, float *q) { return q[0] - p * q[1]; }
";

/// `fld qword ptr [eax+8]; faddp st(1), st(0)` was two instructions and a register where `fadd qword ptr [eax+8]` is one (gcc's output has
/// it everywhere; x_horner, nbody).
#[test]
fn test_a_float_load_one_instruction_reads_is_that_instruction_s_memory_operand() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("fadd qword ptr [eax+8]"), "{text}");
    assert!(!text.contains("faddp") && !text.contains("fsubp") && !text.contains("fsubrp"), "{text}");
}
