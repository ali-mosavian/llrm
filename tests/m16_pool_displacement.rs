//! A constant pool entry read at a displacement is a pool cell plus an offset.

mod common;

/// `(double)(u64)` of a constant at -O0 panicked in the DAG selector ("a
/// constant pool entry plus a displacement is not in our DAG"): zz_f.c did
/// not compile.
#[test]
fn test_u64_constant_to_double_reads_the_fudge_at_plus_four() {
    let source = "typedef unsigned long long u64;\n\
                  double f(void) { u64 big = 0xFFFFFFFFFFFFFFFFULL; double e = (double)big; return e; }\n";
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-O0", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(asm.contains("fadd dword ptr $K") && asm.contains("+4"), "{asm}");
}
