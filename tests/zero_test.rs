//! A register compared with zero is `test r, r`, whatever else is around it.

mod common;

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/parity/sum_three/sum_three.c");
    let output = common::cc::llrm_c()
        .args(flags)
        .current_dir(scratch.path())
        .args(["-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The lines `<op> r, r` of `text`.
fn same_pair(
    text: &str,
    op: &str,
) -> usize {
    text.lines()
        .filter(|line| {
            line.trim()
                .strip_prefix(op)
                .and_then(|rest| rest.strip_prefix(' '))
                .is_some_and(|rest| rest.split_once(", ").is_some_and(|(left, right)| left == right))
        })
        .count()
}

fn zero_compares(text: &str) -> Vec<String> {
    let cmp = regex::Regex::new(r"(?m)^\s+cmp \w+, 0$").unwrap();
    cmp.find_iter(text).map(|one| one.as_str().trim().to_owned()).collect()
}

/// `movzx ecx, [eax]; cmp ecx, 0; je` (`-ftree-ch`; `cmp cx, 0` in 16-bit code)
/// kept its 3-byte compare where the listing without the header copy had `or
/// edi, edi`: two rules, two answers, and neither was the `test` gcc and clang
/// emit, which writes no register.
#[test]
fn a_compare_with_zero_is_a_test_with_and_without_a_header_copy() {
    for flags in [["-m32", "-O2", "-ftree-ch"], ["-m32", "-O2", ""], ["-m16", "-O2", "-ftree-ch"], ["-m16", "-O2", ""]]
    {
        let flags: Vec<&str> = flags.into_iter().filter(|one| !one.is_empty()).collect();
        let text = listing(&flags);
        assert!(zero_compares(&text).is_empty(), "{flags:?}: {:?}", zero_compares(&text));
        assert!(same_pair(&text, "test") > 0 && same_pair(&text, "or") == 0, "{flags:?}:\n{text}");
    }
}
