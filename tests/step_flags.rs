//! A step of a compared value sets the compare's flags.

use std::process::Command;

fn listing(kernel: &str) -> String {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// x_insertion's `n - 1 > 0` guard was `lea eax, [ecx-1]; cmp ecx, 1; jle`: the
/// compare read `n` after the step had defined `n - 1`, so the step could not
/// be made in place and the flags were made twice.
#[test]
fn test_a_guard_on_a_value_and_its_step_is_one_instruction_and_its_flags() {
    let text = listing("x_insertion");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with("add ") && pair[0].ends_with(", -1") && pair[1].starts_with("jle ")),
        "{text}"
    );
    assert!(!lines.iter().any(|line| line.starts_with("lea ") && line.ends_with("-1]")), "{text}");
}
