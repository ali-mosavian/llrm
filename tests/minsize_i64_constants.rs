//! m64 -Oz: a small i64 constant (the pin's `MOV64ImmSExti8`) is loaded and
//! sign-extended to 64 bits.

mod common;

use std::process::Command;

/// m64 -Oz refused 31 corpus programs with `Unmapped("MOV64ImmSExti8")`.
const CALLEE: &str = "
long m1(void) { return -1; }
long m128(void) { return -128; }
long p127(void) { return 127; }
long p5(void) { return 5; }
long big(void) { return 0x123456789L; }
";

const HOST: &str = r#"
#include <stdio.h>
extern long m1(void), m128(void), p127(void), p5(void), big(void);
int main(void) {
  printf("%ld %ld %ld %ld %ld\n", m1(), m128(), p127(), p5(), big());
  return 0;
}
"#;

#[test]
fn test_m64_small_i64_constants_at_oz_are_sign_extended_to_64_bits() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG is not set (llrm-cc needs the pinned clang)");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(dir.join("callee.c"), CALLEE).unwrap();
    std::fs::write(dir.join("host.c"), HOST).unwrap();
    let built = common::cc::llrm_c()
        .current_dir(dir)
        .args(["-m64", "--target=x86_64-linux", "-Oz", "-c", "callee.c", "-o", "callee.o"])
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let linked = Command::new("cc").current_dir(dir).args(["host.c", "callee.o", "-o", "a.out"]).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new(dir.join("a.out")).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "-1 -128 127 5 4886718345");
}
