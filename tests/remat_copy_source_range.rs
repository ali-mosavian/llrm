//! A copy erased by rematerialization cuts back the range of its source.

mod common;

/// gfxdev.c at m16 -Os panicked in the splitter ("Empty interval"): the spiller
/// made a split copy again from its root definition and erased the copy, but
/// left the copy's source ending at the copy, past its last remaining use.
#[test]
fn test_an_erased_remat_copy_shrinks_the_range_it_read() {
    let root = env!("CARGO_MANIFEST_DIR");
    let scratch = tempfile::tempdir().unwrap();
    let mut command = common::cc::llrm_cc();
    command.current_dir(scratch.path()).args(["-m16", "-Os"]);
    for directory in [
        "runtime/qb/dev",
        "runtime/qb/dev/dos/m16",
        "runtime/qb/dos/m16",
        "runtime/qb/dos",
        "runtime/qb/dev/dos",
        "runtime/qb",
    ] {
        command.arg(format!("-I{root}/{directory}"));
    }
    let output = command.args(["-S", "-o", "a.s"]).arg(format!("{root}/runtime/qb/dev/dos/gfxdev.c")).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
