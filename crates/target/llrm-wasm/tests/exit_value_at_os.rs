//! parity/loop -Os: the sum `total += i * seed + 3` over a run-time trip count
//! was left a loop (DEVIATE 15, the 16-bit rule, fired on wasm32 because its
//! pointer is narrower than its legal `i64`): 235 instructions run where
//! clang's closed form runs 105, for 6 B fewer. The pin reads no optsize
//! there.

use std::path::Path;
use std::process::Command;

fn wat(level: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/exit_value_at_os.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", level, "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn has_loop(asm: &str) -> bool {
    asm.lines().any(|line| line.trim_start().starts_with("loop"))
}

#[test]
fn the_sum_of_a_counted_loop_is_a_closed_form_at_os() {
    let asm = wat("-Os");
    assert!(!has_loop(&asm), "the loop was kept at -Os:\n{asm}");
}

#[test]
fn the_sum_of_a_counted_loop_is_a_closed_form_at_o2() {
    let asm = wat("-O2");
    assert!(!has_loop(&asm), "the loop was kept at -O2:\n{asm}");
}
