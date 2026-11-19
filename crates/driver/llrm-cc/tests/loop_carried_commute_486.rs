//! `x &= x - 1` in a loop (popcount's pop loop) at m32 -O2 is `lea; inc; and;
//! jne`: no `mov` of `x` per iteration. The pin's two-address pass bails on the
//! read of `x` by the `x - 1` and keeps the copy (one more instruction per
//! iteration: x_popcount m32 -O2 299 B -> 281 B). DEVIATE TA-LC, on in the i486
//! table.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned pop(unsigned x) { unsigned c = 0; while (x) { x &= x - 1; ++c; } return c; }\n";

/// The instructions of the first loop whose back edge is a `jne` to its own
/// label.
fn pop_loop(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(label) = line.strip_suffix(':') else { continue };
        let body: Vec<String> =
            lines[at + 1..].iter().take_while(|next| !next.ends_with(':')).map(|next| (*next).to_owned()).collect();
        if body.last().is_some_and(|last| last == &format!("jne {label}")) {
            return body;
        }
    }
    Vec::new()
}

#[test]
fn the_pop_loop_has_no_copy_of_the_value_it_clears() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("popcopy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("p.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("p.s"))
        .arg(dir.join("p.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("p.s")).unwrap();
    let body = pop_loop(&text);
    assert!(!body.is_empty(), "no loop that branches back to itself with jne:\n{text}");
    let copies: Vec<&String> = body.iter().filter(|line| line.starts_with("mov ")).collect();
    assert!(copies.is_empty(), "a copy per iteration: {copies:?}\n{text}");
}
