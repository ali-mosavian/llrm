//! A flat (`-m32`) BASIC module body whose registers the frame saves.

use std::process::Command;

/// The listing of a module body that saves registers carries a `.cfi_*`
/// directive after each push, and the check that the native shell is the one
/// the runtime's entry replaces compared them with the shell's instructions:
/// `$QB$MAIN: native frame prefix changed shape` stopped ten bq32 programs
/// (particle, scroll, stft_fixed, stft_single, tile) from building.
#[test]
fn a_flat_module_body_that_saves_registers_builds() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("t.bas");
    std::fs::write(
        &source,
        "DEFINT A-Z\nDIM SHARED t(63, 63)\nFOR y = 0 TO 63: FOR x = 0 TO 63: t(y, x) = (x * 3 + y * 5) AND 255: NEXT: NEXT\nPRINT t(1, 1)\n",
    )
    .unwrap();
    // The bench builds them with `-g`, which puts a line mark ahead of the
    // directives: both forms.
    for debug in [false, true] {
        let object = scratch.path().join(format!("t{debug}.obj"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-qb"));
        command
            .current_dir(scratch.path())
            .arg(&source)
            .args(["--dialect", "qb45", "-fqb-runtime=llrm", "-march=i486", "-m32", "-o"])
            .arg(&object);
        if debug {
            command.arg("-g");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "-g {debug}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(object.exists());
    }
}
