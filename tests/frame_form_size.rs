//! At -Os a function keeps its frame register where the framed body is shorter
//! (`masm::frame_omitted`, DEVIATE FF1).

use std::process::Command;

/// matmul.bas at m32 -Os: `[esp+d]` is a byte longer than `[ebp+d]` and past
/// 127 three more, so its cells cost more than `push ebp; mov ebp, esp` and
/// the `pop`. The pin's `hasFP` alone omits the register: matmul bq32 -Os grew
/// 566 -> 659 bytes (fpbench 767 -> 907, jpeg_fixed 5749 -> 5874).
#[test]
fn test_a_size_tuned_frame_keeps_its_frame_register_where_that_is_shorter() {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .arg(format!("{}/bench/matmul/matmul.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "-fqb-runtime=llrm", "-Os", "-march=i486", "-m32", "-S", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let kernel: Vec<&str> = text.lines().skip_while(|line| !line.starts_with("BENCHMATMUL proc")).collect();
    let kernel = &kernel[..kernel.iter().position(|line| line.starts_with("BENCHMATMUL endp")).expect("endp")];
    assert!(kernel.iter().any(|line| line.trim() == "mov ebp, esp"), "{kernel:#?}");
    assert!(!kernel.iter().any(|line| line.contains("[esp+")), "{kernel:#?}");
}
