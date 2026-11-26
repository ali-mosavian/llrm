//! At -O2 and -O3 a copy the byte rate refused is judged on finished programs,
//! in clocks measured by sim65. Needs `llrm-cc` built (`LLRM_CC`).

mod common;

use std::process::Command;

use common::pipeline::llrm_cc;
use llrm_6502::driver::inline_pays;

// whole runs, start-up included: 58,810 called and 57,683 inlined; 31,026 and
// 18,129.
const MIX_CLOCKS: i64 = 58_200;
const BOUND_CLOCKS: i64 = 22_000;

/// The assembly of `tests/data/{name}.c` and its clocks on sim65, `report`
/// discarding its argument.
fn measured(
    name: &str,
    level: &str,
) -> (String, i64) {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("inline-finished");
    std::fs::create_dir_all(&dir).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.c"));
    let out = dir.join(format!("{name}{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-S", "-w", "-fwhole-program", "-o"])
        .arg(&out)
        .arg(&source)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let body: String = text.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    let main = "  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr main\n";
    let extra = format!("{body}.segment \"CODE\"\nreport:\n  rts\n");
    let clocks = common::cycles_with(&format!("{name}{level}"), main, &extra);
    (text, clocks)
}

/// The trial summed each function's bytes before the outliner ran: two inlined
/// copies of a loop counted 71 B more than the calls though the linked program
/// was 32 B smaller, so the calls stayed (676 B and 53,680 clocks, 644 and
/// 52,573 inlined).
#[test]
fn copies_the_outliner_shares_are_not_refused_on_their_bytes_before_it() {
    for level in ["-O2", "-O3"] {
        let (text, clocks) = measured("inline_shared_loops", level);
        let calls = text.matches("jsr mix\n").count();
        assert!(calls == 0 && clocks < MIX_CLOCKS, "{level}: {calls} calls, {clocks} clocks");
    }
}

/// The rate refused a callee whose loop bound each site passes: kept 29,272
/// clocks, inlined 16,429 for 228 B. The clock model priced the call at ten
/// trips against the copy's 98, and judged before the unroll trial.
#[test]
fn a_copy_that_repays_its_bytes_once_unrolled_is_made() {
    for level in ["-O2", "-O3"] {
        let (_, clocks) = measured("inline_site_bound", level);
        assert!(clocks < BOUND_CLOCKS, "{level}: {clocks} clocks");
    }
}

/// The judge of -O2 and -O3: net = bytes + clocks / 3, the lower kept.
#[test]
fn a_copy_is_judged_at_three_clocks_a_byte() {
    assert!(!inline_pays(9000, 1201, 2), "past the growth bound of two callers");
    assert!(inline_pays(684, 228, 1) && !inline_pays(683, 228, 1));
    assert!(inline_pays(-90, -30, 1), "smaller, and the bytes repay the clocks");
    assert!(!inline_pays(-91, -30, 1));
}
