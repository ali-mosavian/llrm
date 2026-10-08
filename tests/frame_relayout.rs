//! A function whose frame reaches past `[bp-128]` is laid out a second time, with a hole above its allocas for the spill slots, and the
//! layout with fewer far operands is kept. That second layout used to be a second run of the whole backend (a quarter to a third of
//! the backend's time on QCport); it is made from the first run's frame now (docs/optimizations/second-frame.md).

use std::process::Command;

/// (program, flags) whose frames reach past one-byte displacements and spill.
// shellsort -m32 left the list: with EBP a value register (no frame pointer) it no longer spills.
const FAR: [(&str, &[&str]); 4] = [
    ("matmul", &["-m32", "-march=i486"]),
    ("matmul", &[]),
    ("nbody", &[]),
    ("sieve", &["-m32", "-march=i486"]),
];

fn compiled(program: &str, flags: &[&str], level: &str, envs: &[(&str, &str)]) -> (Vec<u8>, String) {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/bench/{program}/{program}.c", env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-c"));
    command.current_dir(scratch.path()).args(flags).args([level, "-o", "a.obj", &source]).env_remove("LLRM_CHECK_FRAME").env("LLRM_TIME_TOP", "1000");
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{program} {flags:?} {level}: {}", String::from_utf8_lossy(&output.stderr));
    (std::fs::read(scratch.path().join("a.obj")).unwrap(), String::from_utf8_lossy(&output.stderr).into_owned())
}

/// The backend ran twice for these functions, once more for the frame: 4 runs of isel in matmul at -O2, 2 candidates each.
#[test]
fn test_a_far_frame_is_laid_out_again_not_run_again() {
    for (program, flags) in FAR {
        let (_, report) = compiled(program, flags, "-O2", &[("LLRM_DEBUG", "time")]);
        assert!(report.contains("frame laid again"), "{program} {flags:?}: no frame was laid again:\n{report}");
        assert!(!report.contains("candidate second frame"), "{program} {flags:?}: the backend ran again for the frame:\n{report}");
    }
}

/// `LLRM_CHECK_FRAME=1` runs the backend again beside and fails on any difference, the displacements, the debug homes, the reserve
/// and the inline code included: the same objects either way.
#[test]
fn test_the_frame_laid_again_is_what_running_the_backend_again_made() {
    for (program, flags) in FAR {
        for level in ["-O0", "-O2", "-Os"] {
            let (made, _) = compiled(program, flags, level, &[]);
            let (checked, _) = compiled(program, flags, level, &[("LLRM_CHECK_FRAME", "1")]);
            assert!(made == checked, "{program} {flags:?} {level}: the objects differ");
        }
    }
}
