//! A global's address that several loops of one nest need is made once.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O1", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// Three sibling loops over `a` hoisted three `mov r, offset a` into the same
/// block (the pin shares them: one `lea`): 7 B each at m64, 10 in bench/grep.
#[test]
fn sibling_loops_share_one_address_of_a_global() {
    let text = listing(
        "int a[100];
int f(int n)
{
    int s = 0;
    for (int k = 0; k < n; k++) {
        for (int i = 0; i < n; i++) s += a[i];
        for (int i = 0; i < n; i++) s += a[i] * 2;
        for (int i = 0; i < n; i++) s ^= a[i];
    }
    return s;
}
",
    );
    assert_eq!(text.matches("offset a").count(), 1, "{text}");
}
