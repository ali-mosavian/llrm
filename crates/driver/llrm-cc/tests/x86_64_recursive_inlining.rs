//! x86-64 -O3 recursive inlining (gcc `recursive_inlining`). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(
    name: &str,
    source: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("recinline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("r.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", level, "-w", "-S", "-o"])
        .arg(dir.join("r.s"))
        .arg(dir.join("r.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("r.s")).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

/// A recursion that makes one call of itself and keeps a frame (`volatile` pad)
/// that no tail-call elimination can remove: gcc copies it into itself. It was
/// copied nowhere: the pass assumed the pin's bar on frames in a recursive
/// caller (stackover -O3 675,016 instrs, gcc 607,410).
#[test]
fn a_linear_recursion_with_a_frame_is_copied_into_itself_on_m64() {
    let text = assembly(
        "frame",
        "int deep(int n)\n{\n    volatile char pad[48];\n    pad[0] = (char)n;\n    if (n == 0) return 0;\n    return 1 + deep(n - 1) + pad[0] - (char)n;\n}\n",
        "-O3",
    );
    let stores = body(&text, "deep").matches("mov byte ptr [rsp").count();
    assert!(stores >= 3, "`deep` has {stores} pad stores, so no copy of itself in it:\n{text}");
}

/// A linear recursion that tail-call elimination turns into a loop is no
/// recursion to copy (gcc runs `tailr1` first): copying it peeled the loop
/// (recpow +3.7% clk).
#[test]
fn a_linear_recursion_that_becomes_a_loop_is_not_copied_on_m64() {
    let text =
        assembly("loop", "long sum(long n)\n{\n    if (n == 0) return 0;\n    return n + sum(n - 1);\n}\n", "-O3");
    assert!(!body(&text, "sum").contains("call sum"), "the recursion is a loop:\n{text}");
    assert!(body(&text, "sum").lines().count() < 30, "`sum` was copied into itself:\n{text}");
}
