//! A `ret` from the DAG emitter reads no flags: the pin has no exit rule.

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

/// The DAG emitter's `ret` was not marked as reading only its results, so
/// flags stayed live to the function's exit, `zeroes` kept the early-out's
/// `mov eax, 0` (5 bytes) where the old selector and the pin have `xor eax,
/// eax` (2): 116 of them over the 71 programs at m32 -O2.
#[test]
fn a_zero_returned_early_is_an_xor_under_dag() {
    let text = listing(
        "int f(int n) { int s = 0, i; if (n <= 0) return 0; \
         for (i = 0; i < n; i++) s += (i & 7) + 3; return s; }\n",
    );
    assert!(!text.contains("mov eax, 0"), "{text}");
}
