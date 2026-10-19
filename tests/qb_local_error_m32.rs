//! ON LOCAL ERROR at -m32.

use std::process::Command;

const SOURCE: &str = "' dialect: pds71
DECLARE SUB top ()
top
END
SUB top
ON LOCAL ERROR GOTO th
PRINT \"x\"
EXIT SUB
th:
RESUME NEXT
END SUB
";

/// The handler's offset is `ptrtoint` of the landing to the runtime's 16-bit
/// argument; at -m32 a pointer is 32 bits and no pattern selected the narrowing,
/// so every program with ON LOCAL ERROR failed with "@TOP: ptrtoint" (#2672).
#[test]
fn test_on_local_error_compiles_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.bas"), SOURCE).unwrap();
    for level in ["-O0", "-O2"] {
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .current_dir(directory)
            .args(["-m32", level, "-S", "-o", "a.s", "a.bas"])
            .output()
            .unwrap();
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{level}: {errors}");
    }
}
