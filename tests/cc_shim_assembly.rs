mod common;

/// The shim appended `-c` to every command, so `-S -o a.s` wrote an OMF object
/// into a.s: gr8_high_bytes died on "stream did not contain valid UTF-8".
#[test]
fn s_writes_assembly_through_the_llrm_c_shim() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("one.c");
    std::fs::write(&source, "int one(void) { return 1; }\n").unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-march=i486", "-O1", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).expect("assembly text, not an object");
    assert!(asm.contains("one_ proc"), "{asm}");
}
