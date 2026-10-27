//! A call's argument cleanup at -Os in 32-bit code.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// One dword of arguments was cleaned by `pop cx; pop cx`: two prefixed
/// 16-bit pops (4 bytes, 2 instructions) where `pop ecx` is one byte. d_faces
/// -Os had ten of them.
#[test]
fn a_dword_of_arguments_is_popped_by_one_dword_pop() {
    let text = listing(
        "extern int g(int); int f(int a) { return g(a) + g(a + 1); }\n",
        &["-m32", "-mabi=sysv", "-Os", "-march=i486"],
    );
    assert!(text.contains("pop ecx") && !text.contains("pop cx"), "{text}");
}
