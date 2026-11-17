//! m16 `parity` (Nib) -O2: a 32-byte zero array took `rep stosd` (the fitted
//! 4-store limit), 46 clocks over the eight stores: 370 clk against the
//! best C compiler's 369. At speed the fill is priced against the stores
//! (SD223), so the pin's store count is the limit.
//! Needs the m16 clang (`LLRM_CLANG_M16`) and the Open Watcom headers
//! (tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const CLEAR: &str = "extern void use(char *p);
void f(void)
{
    char a[32] = {0};
    use(a);
}
";

fn listing(level: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16fill{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), CLEAR).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("c.s")).unwrap()
}

fn tools_present() -> bool {
    let present = std::env::var_os("LLRM_CLANG_M16").is_some() && llrm_cc::ow_present();
    if !present {
        eprintln!("skipped: LLRM_CLANG_M16 is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
    }
    present
}

#[test]
fn a_32_byte_zero_array_is_stores_not_a_string_fill_at_o2() {
    if !tools_present() {
        return;
    }
    let text = listing("-O2");
    assert!(!text.contains("rep stos"), "the clear is a string fill:\n{text}");
}

/// Size still takes the fill: 15 bytes against 35.
#[test]
fn a_32_byte_zero_array_is_a_string_fill_at_os() {
    if !tools_present() {
        return;
    }
    let text = listing("-Os");
    assert!(text.contains("rep stos"), "premise: -Os fills:\n{text}");
}
