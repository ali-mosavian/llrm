//! The -Os size bound kept `blend` a call in the inner loop of a fill (a site
//! 961 times the caller's entry), where the pin's `optsize` threshold takes it:
//! QuickrBasic `pixmem.c`, `q45d25` 2,335,019 instructions against clang's
//! 1,443,306. The hot-site yield trusts the units' growth, which can cost bytes
//! (`suggest.c` +92 B), so HOT-INLINE-TRIAL compiles again with the calls kept
//! and the finished object decides. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

/// The object of `m32_os_hot_site_inline.c` at -Os: its listing and its code
/// and data bytes.
fn object(trial: bool) -> (String, u32) {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let name = format!("x86-os-hot-site-inline-{}-{trial}", std::process::id());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"));
    command.env("LLRM_CC_TARGET", "x86_32-linux").env("LLRM_LLC", llc);
    if !trial {
        command.env("LLRM_NO_HOT_INLINE_TRIAL", "1");
    }
    let run = command
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_hot_site_inline.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    let size = Command::new("size").arg(&object).output().unwrap();
    let bytes = String::from_utf8_lossy(&size.stdout)
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().next())
        .and_then(|text| text.parse().ok())
        .expect("size's text column");
    (String::from_utf8_lossy(&dump.stdout).into_owned(), bytes)
}

fn calls_in_fill(listing: &str) -> usize {
    let fill = listing.split("<fill>:").nth(1).expect("fill");
    fill.split("\n\n").next().unwrap().lines().filter(|line| line.contains("call")).count()
}

#[test]
fn a_blend_in_a_fills_inner_loop_is_inlined_at_os() {
    let (listing, _) = object(false);
    assert_eq!(calls_in_fill(&listing), 0, "fill still calls blend in its loop:\n{listing}");
}

#[test]
fn the_trial_puts_back_a_hot_inline_the_object_says_costs_bytes() {
    let (_, yielded) = object(false);
    let (listing, tried) = object(true);
    assert!(tried < yielded, "the trial kept the yield's {yielded} B, not the call's {tried} B:\n{listing}");
}
