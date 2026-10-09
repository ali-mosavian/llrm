//! A parameter nothing reads leaves with its argument, round a recursion too.

use std::process::Command;

mod common;

const KERNEL: &str = "
static int hanoi(int n, int a, int b, int c)
{
    if (n == 0) return 0;
    return hanoi(n - 1, a, c, b) + 1 + hanoi(n - 1, c, b, a);
}
int f(int n) { return hanoi(n, 1, 3, 2); }
";

/// `-fno-inline-functions`: the test is about the pegs and the loop, not about gcc's recursive inlining (#948), which
/// copies the body into itself eight deep and spends every register on the copies.
fn body(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-O2", "-fno-inline-functions", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let after = text
        .split("hanoi_ proc")
        .nth(1)
        .or_else(|| text.split(&format!("{} proc", common::symbol("hanoi"))).nth(1))
        .unwrap_or_else(|| panic!("no hanoi: {text}"));
    after.split("endp").next().unwrap().to_owned()
}

/// The pegs trade places in each call and are never read: gcc's `hanoi.isra.0(n)` holds one value where llrm held
/// four (a count of moves, hanoi 2.2 times gcc's clocks). With one left the second call is a loop, as it is there.
#[test]
fn test_pegs_only_traded_among_themselves_are_not_passed_and_the_second_call_loops() {
    let flat = body(&["-m32", "-march=i486"]);
    assert_eq!(flat.lines().filter(|line| line.trim().starts_with("call")).count(), 1, "{flat}");
    assert!(!flat.contains("ebx"), "a second argument register is read: {flat}");
    // Real mode passed the three pegs on the stack, pushed at each call and read at its head; now in registers, still
    // not read.
    let real = body(&["-m16"]);
    assert!(!real.contains("[bp+6]") && !real.contains("[bp+8]"), "no second or third argument is read: {real}");
}
