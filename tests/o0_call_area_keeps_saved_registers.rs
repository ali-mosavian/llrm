//! At -O0 a function's outgoing arguments do not overwrite its saved registers.

mod common;

/// The reserved call area is the lowest bytes of the frame (`mov [esp], ecx`).
/// At -O0 the frame was `sub esp, 44; push esi`, so the first argument was
/// stored over the saved `esi` and `pop esi` handed the caller garbage.
#[test]
fn at_o0_saved_registers_are_pushed_before_the_call_area_is_reserved() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = "extern int __cdecl rd(int, int, int);\nextern void __cdecl sink(int);\n\
        int __cdecl f(int *p, int n) {\n    int a = p[0], b = p[1], c = p[2], d = p[3];\n    \
        int r = rd(a, b, c);\n    sink(r);\n    return a + b + c + d + r + n;\n}\n";
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(directory).args(["-O0", "-m32", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let body = &text[text.find("_f proc").unwrap()..];
    let push = body.find("push esi").expect(&text);
    let reserve = body.find("sub esp").expect(&text);
    assert!(push < reserve, "{text}");
}
