//! -O1 on x86-m32 makes no copies of a recursion (gcc's recursive inlining
//! needs -O2). SPEED-O1 had put it on as a side effect: bintree -O1 was 995 B
//! against the bar's 292 B, and fib -O1 six calls of itself where its
//! tail-call elimination leaves one.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
                       int f(int n) { return fib(n); }\n";

fn self_calls(level: &str) -> usize {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("o1-rec{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", level, "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    listing.matches("call fib").count()
}

#[test]
fn test_m32_o1_keeps_the_calls_a_recursion_has_and_o2_copies_it() {
    assert_eq!(self_calls("-O1"), 1, "-O1 copied the recursion");
    assert!(self_calls("-O2") > 1, "-O2 no longer copies the recursion");
}
