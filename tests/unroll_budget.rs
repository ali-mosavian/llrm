//! A completely unrolled loop is bounded by its target's `unroll_budget`, read
//! from the description.

use std::process::Command;

/// Eight trips of `steps` multiply-xor steps.
fn kernel(steps: usize) -> String {
    let body: String = (0..steps)
        .map(|j| format!("        s += (a[i] ^ {}) * (s | {}); s ^= s >> {};\n", 3 + j, 1 + 2 * j, 1 + j % 4))
        .collect();
    format!(
        "int f(const int *a)\n{{\n    int i, s = 0;\n    for (i = 0; i < 8; i++) {{\n{body}    }}\n    return s;\n}}\n"
    )
}

/// What the pass says of the eight-trip loop at `flags`: (the size it compares,
/// the budget it names, the boost percent, admitted).
fn verdict(
    flags: &[&str],
    steps: usize,
) -> (i64, i64, i64, bool) {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), kernel(steps)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
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
        .find(|line| line.contains(" x8: ") && line.contains("scaled"))
        .unwrap_or_else(|| panic!("{flags:?} {steps}: {log}"));
    let after = |key: &str| -> i64 {
        line.split(key)
            .nth(1)
            .unwrap()
            .trim_start()
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    (after("scaled "), after("budget "), after("boost "), line.ends_with("admitted"))
}

/// Both targets took GCC's 200 under LLVM's 400% boost, a limit over either
/// reference's: x86-m32's matmul unrolled to 2633 B where clang's is 1789 and
/// gcc's 402. The edge loops come from sweeping the size: each target's
/// pass names the budget its description states, admits the loops whose
/// (scaled) copies fit it under the boost, and refuses the next one.
#[test]
fn test_each_target_unrolls_to_the_budget_its_description_states() {
    for (flags, budget) in
        [(&["-m32", "-O3", "-march=i486"][..], 150), (&["-m32", "-Omax", "-march=i486"][..], 300), (&["-O3"][..], 200)]
    {
        let sweep: Vec<_> = (1..=10).map(|steps| verdict(flags, steps)).collect();
        for (steps, &(scaled, named, boost, admitted)) in sweep.iter().enumerate() {
            assert_eq!(named, budget, "{flags:?} x{}: the budget the description states", steps + 1);
            assert_eq!(
                admitted,
                scaled <= budget * boost / 100,
                "{flags:?} x{}: scaled {scaled}, boost {boost}%",
                steps + 1
            );
        }
        assert!(
            sweep.first().unwrap().3 && !sweep.last().unwrap().3,
            "{flags:?}: the sweep crosses the budget: {sweep:?}"
        );
        assert!(
            sweep.windows(2).all(|pair| pair[0].3 || !pair[1].3),
            "{flags:?}: a bigger loop is admitted where a smaller is not: {sweep:?}"
        );
    }
    let rolled = |flags: &[&str]| {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), kernel(4)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
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
}
