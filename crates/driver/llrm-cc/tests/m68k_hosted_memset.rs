//! m68k rows passed `-ffreestanding`, which sets `no-builtins` and turns the
//! loop idioms off: a plain fill loop stayed a store loop (hosted-default
//! builds form `memset`, as clang does on every hosted target).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "char t[200];
void f(char v)
{
    int i;
    for (i = 0; i < 200; ++i) t[i] = v;
}
";

#[test]
fn a_fill_loop_on_m68k_is_a_memset_call() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68khosted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("w.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "m68k-68000")
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("w.s"))
        .arg(dir.join("w.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("w.s")).unwrap();
    assert!(text.contains("memset"), "a store loop, no memset:\n{text}");
}
