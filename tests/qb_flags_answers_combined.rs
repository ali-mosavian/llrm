//! Compares of two runtime routines' flags answers, joined by `and` and branched on,
//! were `CannotSelect(Target(24585))` under `--isel=dag`: the merged-branch split
//! exported each compare's flags to a register (qb45/strings, udt, data, intrinsics).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "dim fixedText as string * 8
dim text as string
dim passed as integer
passed = left$(text, 4) = \"dog \" and right$(text, 1) = \"!\" and len(fixedText) = 8
if passed then
    print \"PASS strings\"
end if
";

#[test]
fn test_a_branch_on_the_and_of_two_string_compares_is_selected() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb"));
    let scratch = tempfile::tempdir().unwrap();
    let (input, out) = (scratch.path().join("a.bas"), scratch.path().join("a.asm"));
    std::fs::write(&input, SOURCE).unwrap();
    let done = Command::new(bin)
        .arg(&input)
        .args(["--dialect", "qb45", "--runtime", "qb45", "-O2", "-S", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(out).unwrap();
    assert_eq!(asm.matches("call far ptr B$SCMP").count(), 2, "{asm}");
}
