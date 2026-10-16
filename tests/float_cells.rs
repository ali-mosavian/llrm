//! A spilled x87 value takes a cell of its float type's width.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// A float live across a call was spilled as 8 bytes (d_faces -O2: 62 qword
/// stack operands where gcc and clang had none): it takes a cell of its own
/// type's 4, a double its 8.
#[test]
fn a_float_spills_as_a_float_and_a_double_as_a_double() {
    let text = listing(
        "extern void g(void);\nfloat f(float *p) { float a = p[0] * p[1]; g(); return a + p[2]; }\n\
         double h(double *p) { double a = p[0] * p[1]; g(); return a + p[2]; }\n",
    );
    let (f, h) = text.split_at(text.find("_h proc").unwrap());
    assert!(f.contains("fstp dword ptr [esp") && !f.contains("qword ptr [esp"), "{text}");
    assert!(h.contains("fstp qword ptr [esp"), "{text}");
}
