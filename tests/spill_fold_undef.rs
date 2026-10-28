//! Folding a spilled register out of an instruction leaves nothing naming it.

mod common;

/// gfxdev.c at m16 -Os panicked in the spiller ("v48 is erased while an
/// instruction names it"): a folded `sbb [cell], v` still listed the spilled
/// register as undef. The source is gfxdev.c cut down.
#[test]
fn test_a_fold_drops_the_spilled_register_from_the_undefs() {
    let root = env!("CARGO_MANIFEST_DIR");
    let scratch = tempfile::tempdir().unwrap();
    let mut command = common::cc::llrm_c();
    command.current_dir(scratch.path()).args(["-m16", "-Os", "--isel=dag", "-mbss-begin"]);
    for directory in ["runtime/qb", "runtime/qb/dos/m16", "runtime/qb/dev", "runtime/qb/dev/dos/m16"] {
        command.arg(format!("-I{root}/{directory}"));
    }
    for directory in ["runtime/qb/dev/dos", "runtime/shared/dos/m16", "runtime/shared"] {
        command.arg(format!("-I{root}/{directory}"));
    }
    let output =
        command.args(["-S", "-o", "a.s"]).arg(format!("{root}/tests/data/spill_fold_undef.c")).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
