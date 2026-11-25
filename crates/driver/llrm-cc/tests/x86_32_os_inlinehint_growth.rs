//! `static inline sf_clz32` (clang's `inlinehint`) of QuickrBasic's `sfm16_dbg`
//! was inlined at all six of its calls at -Os, 388 B larger than the one copy
//! gcc keeps: the pin's hint threshold lets the callee in whatever it costs,
//! and the units cannot say what the copies fold to. The finished object
//! decides (INLINEHINT-GROWTH-TRIAL). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-inlinehint-growth-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_hinted_callee_that_grows_the_object_stays_one_copy_at_os() {
    let text = listing("m32_os_inlinehint_growth.c");
    let calls = text.lines().filter(|line| line.contains("call") && line.contains("lead_zeros")).count();
    assert!(
        text.contains("<lead_zeros>:") && calls >= 5,
        "the callee is inlined at its six calls (calls to it: {calls})\n{text}"
    );
}

/// The six calls cost 46 clocks of 193 (+24%, past the 2% cap that put the
/// copies back) and save 295 B: a clock for every six bytes. INLINE-HINT-RATE
/// keeps the calls where the work added is at most a clock a byte saved.
#[test]
fn entries_that_each_run_once_keep_the_calls_for_bytes() {
    let text = listing("m32_os_inlinehint_growth_lib.c");
    let calls = text.lines().filter(|line| line.contains("call") && line.contains("lead_zeros")).count();
    assert_eq!(calls, 6, "the six copies were kept (-295 B for +46 clocks):\n{text}");
}

#[test]
fn six_calls_on_a_path_main_runs_a_thousand_times_are_not_kept() {
    let text = listing("m32_os_inlinehint_growth_hot.c");
    let calls = text.lines().filter(|line| line.contains("call") && line.contains("mix")).count();
    assert_eq!(calls, 0, "{calls} calls kept on a hot path (+43% clocks for -147 B):\n{text}");
}

fn calls_to(
    text: &str,
    callee: &str,
) -> usize {
    text.lines().filter(|line| line.contains("call") && line.contains(callee)).count()
}

#[test]
fn six_loop_calls_inside_the_clock_cap_are_kept_for_bytes() {
    let text = listing("m32_os_loop_sites_within_cap.c");
    let calls = calls_to(&text, "lead_zeros");
    assert_eq!(
        calls, 6,
        "the loop's six copies stay inlined (-428 B for +0.0% clocks, INLINEHINT-LOOP-TRIAL):\n{text}"
    );
}

#[test]
fn a_refused_loop_bundle_does_not_put_back_the_cold_one() {
    let text = listing("m32_os_loop_bundle_refused_keeps_cold.c");
    let (cold, hot) = (calls_to(&text, "lead_zeros"), calls_to(&text, "mix"));
    assert!(
        cold >= 5 && hot == 0,
        "cold calls {cold} (want the six kept, -295 B), loop calls {hot} (want 0, +79% clocks):\n{text}"
    );
}
