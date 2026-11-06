//! At -Os a call's two words of arguments are cleaned by two pops, not
//! `add sp, 4` (cpu.rs `pops_arguments`, DEVIATE PO1).

use std::process::Command;

/// The pin pops only at minsize (X86FrameLowering.cpp:4116); we do at -Os for
/// the byte: qmove bas -Os, four `pop cx; pop cx`, 2 memory operands each.
#[test]
fn test_two_words_of_arguments_are_popped_at_os_where_the_pin_adds_to_sp() {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .arg(format!("{}/bench/parity/qmove/qmove.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "-fqb-runtime=llrm", "-Os", "-march=i486", "-m16", "-S", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(text.contains("pop cx\n    pop cx\n"), "{text}");
    assert!(!text.contains("add sp, 4"), "{text}");
}
