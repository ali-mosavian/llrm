//! A call through a pointer passes its arguments in the convention's registers.

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

/// `int g(int x, int (*q)(int)) { int t = x + 1; return q(x) + t; }` passed `x
/// + 1` to `q`: the pass that gives a far indirect call its frame cell took a
/// near 32-bit pointer for one and emptied the call's uses, so nothing kept `x`
/// alive across the `inc` that was made in its register (x_funcptr returned
/// 36004 for 1259702 under the default convention, -O1 and up).
#[test]
fn a_call_through_a_pointer_passes_the_argument_not_the_value_made_from_it() {
    let text =
        listing("int g(int x, int (*q)(int)) { int t = x + 1; return q(x) + t; }\n", &["-m32", "-O2", "-march=i486"]);
    assert!(!text.contains("mov eax, ecx"), "{text}");
}
