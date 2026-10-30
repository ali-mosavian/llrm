//! A completely unrolled loop is bounded by its target's `unroll_budget`, read
//! from the description.

mod common;

/// Eight trips of `steps` multiply-xor steps.
fn kernel(steps: usize) -> String {
    let body: String = (0..steps)
        .map(|j| format!("        s += (a[i] ^ {}) * (s | {}); s ^= s >> {};\n", 3 + j, 1 + 2 * j, 1 + j % 4))
        .collect();
    format!(
        "int f(const int *a)\n{{\n    int i, s = 0;\n    for (i = 0; i < 8; i++) {{\n{body}    }}\n    return s;\n}}\n"
    )
}

/// What the pass says of the eight-trip loop at `flags`: the threshold it
/// names and whether it unrolls (`unrolling by 8`) or finds no strategy.
fn verdict(
    flags: &[&str],
    steps: usize,
) -> (i64, bool) {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), kernel(steps)).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .env("LLRM_DEBUG", "unroll")
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let log = String::from_utf8_lossy(&output.stderr).into_owned();
    let line = log
        .lines()
        .find(|line| line.contains("unrolling by 8 (") || line.contains("no viable strategy found (trips 8"))
        .unwrap_or_else(|| panic!("{flags:?} {steps}: {log}"));
    let threshold = line.split("threshold ").nth(1).unwrap().trim_end_matches(')').parse().unwrap();
    (threshold, line.contains("unrolling by 8"))
}

/// -O3 and -Omax unroll to LLVM's -O3 threshold, 300, for m32 (the
/// pin's `LoopUnrollPass` full-unroll analysis: the unrolled cost against the
/// threshold, `unrolled 293 rolled 320` is admitted, `unrolled 389` is not).
/// Before the translated pass each target's description stated a budget
/// (gcc's 200 under a 400% boost, then 300 and 400); the pass now takes the
/// pin's number. The edge loop comes from sweeping the size: the pass names
/// the same threshold for each, admits the small loops and refuses the next.
#[test]
fn test_each_target_unrolls_to_the_threshold_the_pin_states() {
    for (flags, budget) in [(&["-m32", "-O3", "-march=i486"][..], 300), (&["-m32", "-Omax", "-march=i486"][..], 300)] {
        let sweep: Vec<_> = (1..=10).map(|steps| verdict(flags, steps)).collect();
        for (steps, &(named, _)) in sweep.iter().enumerate() {
            assert_eq!(named, budget, "{flags:?} x{}: the threshold the pass states", steps + 1);
        }
        assert!(
            sweep.first().unwrap().1 && !sweep.last().unwrap().1,
            "{flags:?}: the sweep crosses the budget: {sweep:?}"
        );
        assert!(
            sweep.windows(2).all(|pair| pair[0].1 || !pair[1].1),
            "{flags:?}: a bigger loop is admitted where a smaller is not: {sweep:?}"
        );
    }
    let rolled = |flags: &[&str]| {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), kernel(4)).unwrap();
        let output = common::cc::llrm_c()
            .current_dir(scratch.path())
            .args(flags)
            .args(["-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        assert!(output.status.success());
        std::fs::read_to_string(scratch.path().join("a.s"))
            .unwrap()
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('j') && !line.starts_with("jmp"))
            .count()
    };
    assert_eq!(rolled(&["-O2"]), 1, "gcc's -O2 copies a loop out only where the code does not grow");
    // m16's table states the pin's size threshold, 0, at every level: no
    // full unroll, whatever the budget the pin's -O3 would have.
    assert_eq!(rolled(&["-O3"]), 1, "m16 keeps the loop at -O3");
}
