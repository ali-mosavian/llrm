//! A pointer made from a base and a scaled index is one `lea`.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `&p[a]` as a value was `shl ecx, 2` and `add eax, ecx`, as bintree's insert loop paid three instructions for a
/// pointer gcc makes with one `lea` (#834). A load or a store through it still takes the address mode itself.
#[test]
fn a_pointer_to_an_element_is_one_lea() {
    let text = listing(
        "int *g(int *p, int a) { return &p[a]; }\nint h(int *p, int a, int *s) { int *q = &p[a]; *s = (int)q; return *q; }\n",
        &["-m32", "-mabi=sysv", "-O2", "-march=i486"],
    );
    assert!(text.contains("lea eax, [eax+ecx*4]") && !text.contains("shl "), "{text}");
}
