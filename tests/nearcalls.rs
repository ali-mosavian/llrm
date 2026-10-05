//! A procedure only its own module calls is entered by a near call in every frontend: `call`
//! and `ret`, not `call far ptr` and `retf`. Nib's were all far, whatever called them.

use std::path::Path;
use std::process::Command;

fn compiled(tool: &str, name: &str, source: &str, arguments: &[&str]) -> String {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let (input, out) = (scratch.path().join(name), scratch.path().join("out.asm"));
    std::fs::write(&input, source).unwrap();
    let done = Command::new(bin.join(tool)).arg(&input).args(arguments).args(["-O2", "-S", "-o"]).arg(&out).output().unwrap();
    assert!(done.status.success(), "{tool}: {}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

fn procedure<'a>(asm: &'a str, name: &str) -> &'a str {
    let from = asm.find(&format!("{name} proc")).unwrap_or_else(|| panic!("{name} in {asm}"));
    asm[from..].split(&format!("{name} endp")).next().unwrap()
}

/// `down` is recursive, so it stays a call; `up` is what the module exports.
fn near_down(asm: &str, down: &str, up: &str) {
    let body = procedure(asm, down);
    assert!(body.starts_with(&format!("{down} proc near")), "{asm}");
    assert!(body.contains(&format!("call {down}\n")) && !body.contains("retf") && body.contains("ret"), "{asm}");
    assert!(procedure(asm, up).starts_with(&format!("{up} proc far")), "{asm}");
}

#[test]
fn test_c_enters_a_static_function_near() {
    let asm = compiled("llrm-c", "near.c", "static int down(int n) { return n ? down(n - 1) + n : 0; }\nint up(int n) { return down(n); }\n", &[]);
    near_down(&asm, "_down", "_up");
}

#[test]
fn test_nib_enters_an_unexported_function_near() {
    let source = "fn down(n: i16) -> i16:\n    if n == 0:\n        return 0\n    return down(n - 1) + n\n\n@export(\"cdecl16\")\nfn up(n: i16) -> i16:\n    return down(n)\n\nfn main() -> i16:\n    print(up(3))\n    return 0\n";
    near_down(&compiled("llrm-nib", "near.nib", source, &[]), "_down", "_up");
}

#[test]
fn test_basic_enters_a_module_internal_procedure_near() {
    let source = "DECLARE FUNCTION Down% (n AS INTEGER)\nPRINT Down%(3)\nFUNCTION Down% (n AS INTEGER)\n  IF n = 0 THEN Down% = 0 ELSE Down% = Down%(n - 1) + n\nEND FUNCTION\n";
    let asm = compiled("llrm-qb", "near.bas", source, &["--own-frames", "--whole-program", "--dialect", "pds71", "--runtime", "pds71"]);
    let body = procedure(&asm, "DOWN");
    assert!(body.starts_with("DOWN proc near") && body.contains("call DOWN\n") && !body.contains("retf"), "{asm}");
}
