//! A state machine's dispatch.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const MACHINE: &str = "int f(int n)\n{\n    int s = 0, i, acc = 0;\n    for (i = 0; i < n; ++i) {\n        switch (s) {\n        case 0: s = (i & 1) ? 1 : 2; break;\n        case 1: s = 2; acc += 3; break;\n        case 2: s = 0; acc += 5; break;\n        default: s = 0; break;\n        }\n    }\n    return acc;\n}\n";

/// `switch (state)` in a loop whose cases each set the next state was a compare of the state on every trip (x_switch:
/// gcc -O2 alone threads it, 2.3x of the clocks of everyone else's): each case now jumps to the next one's code, and no
/// compare of the state is left.
#[test]
fn a_state_machine_loop_has_no_dispatch_left() {
    let text = listing(MACHINE, &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(!text.contains("cmp "), "{text}");
}

/// Tuned for size the dispatch stays: the copies are code.
#[test]
fn tuned_for_size_the_dispatch_stays() {
    let text = listing(MACHINE, &["-m32", "-mabi=sysv", "-Os", "-march=i486"]);
    assert!(text.contains("cmp "), "{text}");
}
