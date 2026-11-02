//! A global's address that several loops of one nest need is made once.

use std::process::Command;

fn listing(source: &str) -> String {
    listing_at(source, "-O1")
}

fn listing_at(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", level, "-S", "-o", "a.s", "a.c"])
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

/// bench/histo at -Os made `offset counts` three times (hold, `memset`
/// argument, walk pointer) where the pin shares one: the address feeding the
/// walk pointer's phi sat behind a copy, which `isProfitableToCSE` (heuristic
/// 2: only copies read it) refused; the pin's phi is no copy. 184 B against the
/// pin's 172.
#[test]
fn a_global_address_feeding_a_phi_is_shared() {
    let text = listing_at(include_str!("../bench/histo/histo.c"), "-Os");
    assert_eq!(text.matches("offset counts").count(), 1, "{text}");
    assert_eq!(text.matches("offset data").count(), 1, "{text}");
}
