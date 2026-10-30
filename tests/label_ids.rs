//! A block made after layout is labelled by the two halves of its id, not by
//! its ten-digit number (`masm::label`).

use std::process::Command;

fn listing(
    args: &[&str],
    file: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("a.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .args(args)
        .arg("-S")
        .arg("-o")
        .arg(&out)
        .arg(file)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(&out).unwrap()
}

/// The digits after `L<n>_` of every label a listing names.
fn label_digits(asm: &str) -> Vec<usize> {
    asm.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter_map(|word| word.strip_prefix('L')?.split_once('_').map(|(_, rest)| rest.len()))
        .collect()
}

#[test]
fn a_label_of_a_block_made_by_a_critical_edge_split_is_short() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/mandel/mandel.nib");
    let asm = listing(&["-Os"], root);
    let longest = label_digits(&asm).into_iter().max().unwrap_or(0);
    assert!(label_digits(&asm).len() > 1, "premise: the listing has labels: {asm}");
    assert!(longest <= 6, "a label has {longest} digits: {asm}");
}

#[test]
fn the_check_counts_the_digits_it_names() {
    assert_eq!(label_digits("L0_4294967297:\n jmp L0_12\n"), vec![10, 2]);
}
