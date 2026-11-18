//! `-mregparm=3` and `regparm(3)` reached the backend as `inreg` parameters,
//! which the Open Watcom convention (`-mabi=watcom`) gives no registers:
//! the function took its arguments from the stack where a regparm caller put
//! them in EAX, EDX and ECX, and the compile said nothing. Only the i386
//! System V convention places them (`m32_regparm3.rs`).

use std::path::Path;
use std::process::Command;

fn compile(name: &str, source: &str, flags: &[&str]) -> std::process::Output {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("regparm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-mabi=watcom", "-march=i486", "-O2", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .args(flags)
        .output()
        .unwrap()
}

const SOURCE: &str = "int f(int a, int b, int c, int d) { return a * 3 + b * 5 + c * 7 + d; }\n";

#[test]
fn test_m32_the_regparm_flag_is_refused_not_ignored() {
    let run = compile("flag", SOURCE, &["-mregparm=3"]);
    assert!(!run.status.success(), "compiled with -mregparm=3 and no registers");
    assert!(String::from_utf8_lossy(&run.stderr).contains("regparm"), "{}", String::from_utf8_lossy(&run.stderr));
}

#[test]
fn test_m32_the_regparm_attribute_is_refused_not_ignored() {
    let source = "__attribute__((regparm(3))) int f(int a, int b, int c) { return a * 3 + b * 5 + c * 7; }\n";
    let run = compile("attribute", source, &[]);
    assert!(!run.status.success(), "compiled regparm(3) and no registers");
}

#[test]
fn test_m32_a_plain_function_still_compiles() {
    let run = compile("plain", SOURCE, &[]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
