//! `loop-split@gcc` left the second loop's `if (!composite[i]) { ++count; sum
//! += i; }` arm as control flow, which the vectorizer reads as a block that
//! does not run every trip ("Control flow cannot be substituted for a select"):
//! bench/sieve -O3 on arm64 ran 17098 instructions, 2126 more than -O2,
//! where the same loop split by hand is vectorized. gcc's `pass_split_loop`
//! returns `TODO_cleanup_cfg`.

use std::path::Path;
use std::process::Command;

/// bench/sieve: the guard `i <= 31` splits the loop at 32; the second loop
/// counts what the first sieved.
const SOURCE: &str = "unsigned long f(unsigned short limit)\n{\n    unsigned char composite[1024];\n    unsigned short i, multiple, count = 0;\n    unsigned long sum = 0;\n\n    for (i = 0; i < limit; ++i) composite[i] = 0;\n    for (i = 2; i < limit; ++i) {\n        if (composite[i]) continue;\n        ++count;\n        sum += i;\n        if (i <= 31)\n            for (multiple = (unsigned short)(i * i); multiple < limit; multiple = (unsigned short)(multiple + i))\n                composite[multiple] = 1;\n    }\n    return ((unsigned long)count << 16) ^ sum;\n}\n";

#[test]
fn the_tail_loop_of_a_split_is_vectorized() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("splittail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "arm64-linux")
        .env("LLRM_DEBUG", "loop-vectorize")
        .args(["-O3", "-w", "-S", "-ffreestanding", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let log = String::from_utf8_lossy(&run.stderr);
    assert!(!log.contains("Control flow cannot be substituted"), "{log}");
    let loops = log.lines().filter(|line| line.contains(": vectorized VF")).count();
    assert_eq!(loops, 2, "the zero fill and the counting tail:\n{log}");
}
