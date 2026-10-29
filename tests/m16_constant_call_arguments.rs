//! A constant pushed by two calls is pushed as an immediate each time at m16.

mod common;

const KERNEL: &str = "
extern void f(int a, int b, long c);
void g(int x) { f(257, 8, 3); f(x, 8, 3); }
";

/// Each constant was made in a register and held across the calls
/// (`mov eax, 3; mov si, 8; push eax; push si`), where the push of an
/// immediate is 2 to 3 bytes (`pushd 3; pushw 8`) and the register needs
/// saving too: demo-deedlines under the DAG selector, 593 `pushw imm` turned
/// into `mov; push` (+3186 B at -Os).
#[test]
fn test_a_constant_argument_of_two_calls_is_pushed_as_an_immediate() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args([
            "-m16",
            "-march=i386",
            "-mabi=cdecl",
            "-Xclang",
            "-fdefault-calling-conv=cdecl",
            "--isel=dag",
            "-Os",
            "-S",
            "-o",
            "-",
            "a.c",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert_eq!(asm.matches("pushd 3").count(), 2, "{asm}");
    assert_eq!(asm.matches("pushw 8").count(), 2, "{asm}");
    assert!(!asm.contains("mov eax, 3") && !asm.contains("mov si, 8"), "{asm}");
}
