//! m16 `grep` -Os: the hit test read `movzx edx, byte ptr [si+..]; test edx,
//! edx` (3 clk on the 486) where the extension is used only in the rare arm.
//! -O1 kept `mov al, [..]; test al, al`: -Os ran 37.0M clk against 29.7M, over
//! bcc's 33.9M, at no byte gain (11 B against 10).
//! Needs the m16 clang (`LLRM_CLANG_M16`) and the Open Watcom headers
//! (tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const HIT: &str = "extern unsigned char tab[256];
extern unsigned char byte_at(unsigned i);
unsigned long sum_hits(unsigned n)
{
    unsigned long sum = 0;
    unsigned i;
    for (i = 0; i < n; ++i) {
        unsigned char h = tab[byte_at(i)];
        if (h) sum = ((sum << 1) + h) & 0x1FFFFFFFUL;
    }
    return sum;
}
";

#[test]
fn a_widening_that_only_the_rare_arm_reads_is_not_hoisted_to_its_load_at_os() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_CLANG_M16 is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16cold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hit.c"), HIT).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-o"])
        .arg(dir.join("hit.s"))
        .arg(dir.join("hit.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("hit.s")).unwrap();
    assert!(
        !text.lines().any(|line| line.trim().starts_with("test e") && line.contains(", e")),
        "the hit test is a 32-bit test of a widened load:\n{text}"
    );
}
