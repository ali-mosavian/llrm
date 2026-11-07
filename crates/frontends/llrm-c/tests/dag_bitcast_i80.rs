//! A long double read through an integer union compiles under the DAG emitter.

/// The listing of `source` at -O2 for 32-bit x86 under `--isel=dag`.
fn listing(source: &str) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let (input, output) = (dir.path().join("in.c"), dir.path().join("out.s"));
    std::fs::write(&input, source).expect("the source");
    let argv: Vec<String> = ["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o"]
        .iter()
        .map(|one| one.to_string())
        .chain([output.display().to_string(), input.display().to_string()])
        .collect();
    assert_eq!(llrm_c::compile::main(&argv), 0);
    std::fs::read_to_string(output).expect("the listing")
}

/// `bitcast x86_fp80 to i80` had no promote-result case ("Unsupported
/// BitCast Int(80) promote result"): every long double union pun or memcpy
/// at -O2 failed to compile on x86_32 and x86_64.
#[test]
fn a_long_double_read_through_a_union_of_shorts_compiles() {
    let text = listing(
        "unsigned f(long double x) { union { long double d; unsigned short w[5]; } u; u.d = x; return u.w[0]; }\n",
    );
    assert!(text.contains("fstp"), "{text}");
}
