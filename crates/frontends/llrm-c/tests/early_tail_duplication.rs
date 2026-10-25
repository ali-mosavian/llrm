//! Early Tail Duplication in a compile: `llrm-c -O2` of a loop whose join has a
//! phi.

/// The listing of `source` at -O2 for 32-bit x86.
fn listing(source: &str) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let (input, output) = (dir.path().join("in.c"), dir.path().join("out.s"));
    std::fs::write(&input, source).expect("the source");
    let argv: Vec<String> = ["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o"]
        .iter()
        .map(|one| one.to_string())
        .chain([output.display().to_string(), input.display().to_string()])
        .collect();
    assert_eq!(llrm_c::compile::main(&argv), 0);
    std::fs::read_to_string(output).expect("the listing")
}

/// The join's store was one block both arms jumped to; the pass left it
/// there, so `-O2` printed one `mov dword ptr _k, eax`. Copied into each arm
/// (the join's phi becomes each arm's call result, the loop header's phi
/// reads both copies), the listing has two.
#[test]
fn a_join_of_two_calls_is_copied_into_both_arms() {
    let text = listing(
        "int p(void); int q(void); int k;\n\
         void f(int a) { for (;;) { int x; if (a) x = p(); else x = q(); k = x; } }\n",
    );
    assert_eq!(text.matches("mov dword ptr _k, eax").count(), 2, "{text}");
}
