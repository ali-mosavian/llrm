//! x86-64 PIE: a symbol is `[rip + sym]`, with no index or base register
//! beside it. LSR priced `@sym + reg` as one address and kept it, so the
//! backend formed `lea r,[rip+sym]` at every use in the loop: histo -O2 was
//! 88 B larger (2136 B object against 2048 B). The size no longer measures it
//! (the loops are vectorized now, 3032 B), so the test reads the loops: no
//! symbol address is made inside one. This is a PROXY for that symptom: it fails
//! on `-O0`, where the symbol is formed in the loop, but reverting the LSR
//! refusal alone leaves it green. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn no_symbol_address_is_made_inside_a_loop_at_x86_64_pie() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pie64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-fPIE", "-O2", "-w", "-S", "-o"])
        .arg(dir.join("h.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/pie64_histo.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let asm = std::fs::read_to_string(dir.join("h.s")).unwrap();
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let mut made = 0;
    let mut loops = 0;
    for (at, line) in lines.iter().enumerate() {
        let Some(target) = line.split_whitespace().nth(1).filter(|_| line.starts_with('j')) else { continue };
        let Some(label) = lines[..at].iter().position(|earlier| earlier.strip_suffix(':') == Some(target)) else {
            continue;
        };
        loops += 1;
        made += lines[label..at].iter().filter(|inside| inside.contains("offset ")).count();
    }
    assert!(loops > 0, "premise: the histogram has a loop\n{asm}");
    assert_eq!(made, 0, "a symbol's address is made inside a loop:\n{asm}");
}
