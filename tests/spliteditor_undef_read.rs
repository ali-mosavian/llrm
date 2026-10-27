//! A split register read as undef by an instruction that also defines it.

use std::process::Command;

/// `crc = ... XOR (&HEDB88320 AND (0& - (crc AND 1&)))` at -O2 (dag): the mask
/// is `sbb x, x`, whose tied read of `x` is undef. `rewriteAssigned` looked the
/// read up at the instruction and the def at its register slot; the split put
/// the boundary between them and the editor panicked "a tied pair is split over
/// registers 0 and 1 (D-SE5)". An undef read goes where the def does.
#[test]
fn a_split_register_read_as_undef_by_its_own_def_compiles() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/qb/crc.bas");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["--dialect", "qb45", "--runtime", "qb45", "-O2", "-march=i486", "--isel=dag", "-o", "x.obj"])
        .arg(&source)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
}
