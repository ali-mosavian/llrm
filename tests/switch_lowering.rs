//! A switch is not a chain of compares: x_switch_dense was 11 in a row (445 B,
//! 1.23x gcc's clocks) and x_switch_ranges 58 (770 B, 3.2x).

use std::process::Command;

fn compares(
    kernel: &str,
    flags: &[&str],
) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-fobject-format=elf", "-S", "-o", "a.s"])
        .args(flags)
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s"))
        .unwrap()
        .lines()
        .filter(|line| line.trim().starts_with("cmp"))
        .count()
}

#[test]
fn a_switch_of_constants_is_a_load_and_a_character_class_a_few_ranges() {
    for flags in [["-O2"], ["-Os"]] {
        let (dense, ranges) = (compares("x_switch_dense", &flags), compares("x_switch_ranges", &flags));
        assert!(dense <= 4, "{flags:?}: x_switch_dense has {dense} compares");
        assert!(ranges <= 16, "{flags:?}: x_switch_ranges has {ranges} compares");
    }
}

/// Without `-ftree-switch-conversion` the same cases are a tree of compares.
#[test]
fn without_switch_conversion_the_constants_are_compares() {
    assert!(compares("x_switch_dense", &["-O2", "-fno-tree-switch-conversion"]) >= 11);
}
