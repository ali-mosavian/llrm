//! The Watcom rows follow Open Watcom's libc: `wchar_t` and wide literals are 2
//! bytes (m32 had literals of 4 beside a 2-byte `wchar_t`, so `wcslen(L"ab")`
//! did not compile) and `long double` is 8 (clang gave 12 at m32, 10 at m16, to
//! a libc that reads 8). Needs the fork's clang (`LLRM_CLANG_M16`) and the Open
//! Watcom headers (`LLRM_OW`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn listing(
    name: &str,
    target: &str,
    source: &str,
) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("llrm-cc-watabi-{name}-{target}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (c, s) = (dir.join("a.c"), dir.join("a.s"));
    std::fs::write(&c, source).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", target)
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-S", "-o"])
        .arg(&s)
        .arg(&c)
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&s).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    if run.status.success() { Ok(text) } else { Err(String::from_utf8_lossy(&run.stderr).into_owned()) }
}

fn configured() -> bool {
    let set = std::env::var_os("LLRM_CLANG_M16").is_some() && std::env::var_os("LLRM_OW").is_some();
    if !set {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
    }
    set
}

#[test]
fn a_wide_string_literal_is_a_wchar_t_array_on_the_watcom_rows() {
    if !configured() {
        return;
    }
    let source = "#include <wchar.h>\nint n(void) { return (int)wcslen(L\"ab\"); }\n";
    for target in ["x86_32-wat", "x86_16-wat"] {
        listing("wide", target, source).unwrap_or_else(|why| panic!("{target}: {why}"));
    }
}

#[test]
fn a_long_double_is_8_bytes_on_the_watcom_rows() {
    if !configured() {
        return;
    }
    let source = "int size = sizeof(long double);\n";
    for target in ["x86_32-wat", "x86_16-wat"] {
        let text = listing("ld", target, source).unwrap();
        let line =
            text.lines().skip_while(|line| !line.starts_with("_size ")).nth(1).unwrap_or_default().trim().to_owned();
        assert!(
            line.starts_with("db 008h") || line.starts_with("dd 8") || line.starts_with("dw 8"),
            "{target}: {text}"
        );
    }
}
