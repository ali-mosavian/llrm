//! The `llrm-cc` shim writes assembly text for `-S`, not an object.

mod common;

/// The shim appended `-c` to every run, so `-S -o a.s` wrote an OMF object into
/// `a.s` and every test reading its listing failed with "stream did not contain
/// valid UTF-8".
#[test]
fn a_shim_run_with_dash_s_writes_text() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), "int f(void) { return 1; }\n").unwrap();
    let output = common::cc::llrm_cc().current_dir(directory).args(["-m32", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).expect("a.s is text");
    assert!(text.contains("proc"), "{text}");
}
