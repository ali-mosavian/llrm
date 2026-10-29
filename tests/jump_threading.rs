//! A state machine's dispatch.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const MACHINE: &str = "int f(int n)\n{\n    int s = 0, i, acc = 0;\n    for (i = 0; i < n; ++i) {\n        switch (s) {\n        case 0: s = (i & 1) ? 1 : 2; break;\n        case 1: s = 2; acc += 3; break;\n        case 2: s = 0; acc += 5; break;\n        default: s = 0; break;\n        }\n    }\n    return acc;\n}\n";

/// `switch (state)` in a loop whose cases each set the next state is a compare
/// of the state on every trip in the pin (`cmp esi, 2; cmp esi, 1`, and the
/// loop's `cmp ecx, edx`); ours is the same listing, so the state is compared
/// twice and nothing more (the translated jump threading no longer threads it
/// further than the pin does).
#[test]
fn a_state_machine_loop_keeps_the_pins_two_compares_of_the_state() {
    let text = listing(MACHINE, &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    let dispatch = text.lines().map(str::trim).filter(|line| line.starts_with("cmp ") && !line.contains(", e")).count();
    assert_eq!(dispatch, 2, "{text}");
}

/// Tuned for size the dispatch stays: the copies are code.
#[test]
fn tuned_for_size_the_dispatch_stays() {
    let text = listing(MACHINE, &["-m32", "-mabi=sysv", "-Os", "-march=i486"]);
    assert!(text.contains("cmp "), "{text}");
}
