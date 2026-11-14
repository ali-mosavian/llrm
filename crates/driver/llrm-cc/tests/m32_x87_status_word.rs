//! m32 (i486): x87 compares read the status word as a value (DEVIATE
//! X87-STATUS, gcc `ix86_expand_fp_compare`). Six predicates of one pair cost
//! two `fnstsw` and no `sahf` (a `sahf` set `setcc` bytes that spilled across
//! the adds: sfm16_float_cmp 32872 -> 29220 clk at -O2); `==` is one masked
//! compare, not `setnp`, `sete` and a join. -Os keeps the pin's `sahf`: the
//! word costs bytes there.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32st-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_x87_status_word.c"))
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

#[test]
fn test_six_predicates_of_one_pair_share_the_status_word() {
    for level in ["-O2", "-O3"] {
        let asm = compiled(level);
        let six = function(&asm, "six");
        assert_eq!(count(&six, "sahf"), 0, "{level}: sahf kept:\n{}", six.join("\n"));
        assert_eq!(count(&six, "fnstsw"), 2, "{level}: one fnstsw per compare:\n{}", six.join("\n"));
    }
}

#[test]
fn test_float_equality_is_one_masked_compare_of_the_word() {
    let asm = compiled("-O2");
    let equal = function(&asm, "equal");
    for banned in ["sahf", "setnp", "jp"] {
        assert_eq!(count(&equal, banned), 0, "{banned} in ==:\n{}", equal.join("\n"));
    }
    assert!(equal.iter().any(|line| line.starts_with("and ")), "no mask:\n{}", equal.join("\n"));
    // The compare that defines the word still takes its second operand from
    // memory: `fld; fld; fcompp` cost an instruction more than `fld; fcomp
    // [m]`.
    assert_eq!(count(&equal, "fcompp"), 0, "load not folded into the compare:\n{}", equal.join("\n"));
}

#[test]
fn test_size_keeps_sahf() {
    let asm = compiled("-Os");
    let six = function(&asm, "six");
    assert!(count(&six, "sahf") > 0, "-Os left the pin's sahf:\n{}", six.join("\n"));
}
