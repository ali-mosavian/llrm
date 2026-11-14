//! x86-32 -O3: a function that reads a stack argument in its entry block is not
//! shrink-wrapped, as the pin's `useOrDefCSROrFI` counts the argument's frame
//! index as a frame use. x_collatz wrapped its `push`es past the early exit, so
//! the loop's exit paid a `jmp` to the shared bare `ret`: 3 clk on 168,668,
//! over clang's bar by the 3. Needs the Open Watcom tree (tools/ow-fetch.sh)
//! and `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "long f(int n)\n{\n    long best = 0; int i;\n    for (i = 1; i <= n; ++i) {\n        unsigned long x = (unsigned long)i; int steps = 0;\n        while (x != 1) { x = (x & 1) ? 3 * x + 1 : x >> 1; ++steps; }\n        if (steps > best) best = steps;\n    }\n    return best;\n}\n";

#[test]
fn a_stack_argument_read_at_entry_keeps_the_saves_at_entry() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("shrink-wrap-args-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-O3", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = |what: &str| lines.iter().position(|line| line.starts_with(what));
    let (push, load) =
        (at("push").expect("a saved register"), at("mov ecx, dword ptr [esp").expect("the argument load"));
    assert!(push < load, "the saves wait for a later block while the entry reads the argument:\n{text}");
    let popped_then_jumped = lines.windows(2).any(|pair| pair[0].starts_with("pop") && pair[1].starts_with("jmp"));
    assert!(!popped_then_jumped, "an epilogue that jumps to a shared bare ret:\n{text}");
}
