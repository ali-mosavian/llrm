//! x86-32 -Os: the size trials kept the smaller shape whatever it cost in
//! clocks. `x_switch_ranges` refused its lookup table for 35 B at +11% clk
//! (127,846 against 115,143). The cap keeps the pin's shape where the work the
//! smaller one adds is over `os_trial_clock_cap_percent`. The copy-loop trial
//! is exempt (the words copied are not known). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn asm_at_os(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-clock-cap-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn a_switch_whose_compares_cost_eleven_percent_clocks_keeps_its_lookup_table() {
    let text = asm_at_os("os_switch_ranges.c");
    assert!(text.contains("switch$table"), "the switch lost its lookup table:\n{text}");
}
