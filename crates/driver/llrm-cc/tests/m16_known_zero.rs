//! m16 `sieve` -Os: `mov dx, ax` copied a zero into `dx`, which `xor edx, edx`
//! at entry had already zeroed: 112 B, one over Open Watcom's 111.
//! Needs the m16 clang (`LLRM_CLANG_M16`) and the Open Watcom headers
//! (tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SIEVE: &str = "extern void report(long value);
unsigned long bench_sieve(unsigned short limit)
{
    unsigned char composite[1024];
    unsigned short i, multiple, count = 0;
    unsigned long sum = 0;
    for (i = 0; i < limit; ++i) composite[i] = 0;
    for (i = 2; i < limit; ++i) {
        if (composite[i]) continue;
        ++count;
        sum += i;
        if (i <= 31)
            for (multiple = (unsigned short)(i * i); multiple < limit; multiple = (unsigned short)(multiple + i))
                composite[multiple] = 1;
    }
    return ((unsigned long)count << 16) ^ sum;
}
";

#[test]
fn a_zero_is_not_moved_into_a_register_that_already_holds_it() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_CLANG_M16 is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16zero-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sieve.c"), SIEVE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-march=i386", "-o"])
        .arg(dir.join("sieve.s"))
        .arg(dir.join("sieve.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("sieve.s")).unwrap();
    assert!(
        !text.lines().any(|line| line.trim() == "mov dx, ax"),
        "a zero was copied into a register that holds it:\n{text}"
    );
}
