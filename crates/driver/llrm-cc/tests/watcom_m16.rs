//! Watcom m16 programs the first i8086 fork could not compile. Needs the fork's
//! clang (`LLRM_CLANG_M16`) and the Open Watcom headers (`LLRM_OW`).

use std::path::Path;
use std::process::Command;

fn compiles(
    name: &str,
    source: &str,
) -> Result<(), String> {
    let (Some(_), Some(_)) = (std::env::var_os("LLRM_CLANG_M16"), std::env::var_os("LLRM_OW")) else {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
        return Ok(());
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat16-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c = dir.join(format!("{name}.c"));
    std::fs::write(&c, source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join(format!("{name}.obj")))
        .arg(&c)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    if run.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&run.stderr).into_owned()) }
}

/// `bench/lru` stopped at "expected member name or ';'": `far` before the `*`
/// of a later declarator in one declaration.
#[test]
fn far_before_the_star_of_a_later_declarator() {
    compiles("lru", "struct N { short far *a, far *b; short near *c, far *d; };\nstruct N n;\n").unwrap();
}

/// `long_double_layout.c` stopped in clang at "cannot compile this static
/// initializer yet": the datalayout left `f80` at LLVM's 128 bits.
#[test]
fn a_long_double_between_a_char_and_a_short_with_an_initializer() {
    compiles("ldl", "struct M { char t; long double v; short a; };\nstruct M m = { 7, 4.25L, 9 };\n").unwrap();
}
