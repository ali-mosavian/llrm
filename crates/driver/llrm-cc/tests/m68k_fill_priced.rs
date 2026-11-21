//! A fill loop that keeps a computed store beside the constant one saves one store a trip: with every operation one
//! clock (`Machine::costs` before it read the tables) a four-trip loop became a `memset` call, 130 clocks of call for
//! 16 of stores. Priced, the short loop stays and the long one is the call.

use std::path::Path;
use std::process::Command;

fn compile(trips: u32) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68kfill-{}-{trips}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = format!("short a[{trips}], c[{trips}];\nvoid f(short k)\n{{\n    int i;\n    for (i = 0; i < {trips}; ++i) {{ a[i] = i * k; c[i] = 7; }}\n}}\n");
    std::fs::write(dir.join("w.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "m68k-68000")
        .args(["-O2", "-w", "-S", "-fhosted", "-o"])
        .arg(dir.join("w.s"))
        .arg(dir.join("w.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("w.s")).unwrap()
}

#[test]
fn a_four_trip_fill_beside_a_computed_store_stays_a_loop() {
    let text = compile(4);
    assert!(!text.contains("memset"), "a call for four stores:\n{text}");
}

#[test]
fn a_long_fill_beside_a_computed_store_is_the_call() {
    let text = compile(400);
    assert!(text.contains("memset"), "800 bytes of stores:\n{text}");
}
