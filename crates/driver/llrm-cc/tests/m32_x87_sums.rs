//! m32 (i486): a float result is added where it is made, not sunk to the
//! return (DEVIATE SINK-X87): the eight results of `x_ld_horner` would
//! otherwise be live across every later loop and overflow the x87 stack.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32sum-{level}-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_x87_sums.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn function(
    asm: &str,
    name: &str,
) -> Vec<String> {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines()
        .skip(start)
        .take_while(|line| line.trim() != format!("{name} endp"))
        .map(|line| line.trim().to_owned())
        .collect()
}

fn count(
    body: &[String],
    mnemonic: &str,
) -> usize {
    body.iter().filter(|line| line.split_whitespace().next() == Some(mnemonic)).count()
}

/// InstCombine moved the six later `t += r` to the return, so seven results
/// were live at the last loop: a 10-byte spill and a `fld` of the multiplier
/// from memory each trip of that loop (x_ld_horner -O2 19324 clk, gcc 19274).
#[test]
fn test_the_running_sum_is_added_where_each_result_is_made() {
    for level in ["-O2"] {
        let asm = compiled(level);
        let body = function(&asm, "sums");
        let spills: Vec<_> = body.iter().filter(|line| line.starts_with("fstp tbyte ptr [esp")).collect();
        assert!(spills.is_empty(), "{level}: a sum spilled {spills:?}:\n{}", body.join("\n"));
        assert!(count(&body, "fld") > 0, "{level}: no float code:\n{}", body.join("\n"));
    }
}
