//! What the header copy (`-ftree-ch`) leaves for the passes after it.

use std::process::Command;

fn listing(
    kernel: &str,
    flags: &[&str],
) -> String {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486"])
        .args(flags)
        .args(["-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `for (j = i - 1; j >= 0 && a[j] > v; --j)` ends the counter's test on the
/// pointer stepping to its base (`add ebp, -4; je`), as it did before the copy.
/// Behind the copy the guard `j >= 0` is folded on the outer counter's range,
/// and the loop's entry was no longer proven: it kept `dec ebx; add esi, -4;
/// cmp ebx, 0; jl` (x_insertion +8% clocks).
#[test]
fn test_a_loop_entered_behind_a_folded_guard_is_still_counted_by_the_ranges() {
    let text = listing("x_insertion", &["-ftree-ch"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with("add ") && pair[0].ends_with(", -4") && pair[1].starts_with("je ")),
        "{text}"
    );
}

/// gcc copies headers after `pass_ipa_inline` (passes.def:232), so the inliner
/// sizes a body before its test is copied. Copied before it, fib's body counted
/// 9 operations against the 7 the self-inline admits and the nest of six levels
/// (2004 calls) was not made: 10946 calls, x_fib 268481 clocks against 210858
/// with the copy off.
#[test]
fn test_fib_with_the_header_copy_on_keeps_its_self_inlined_nest() {
    let text = listing("x_fib", &["-ftree-ch"]);
    let function: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("_fib proc"))
        .take_while(|line| !line.starts_with("_fib endp"))
        .collect();
    let compares = |bound: &str| {
        function.iter().filter(|line| line.trim().starts_with("cmp ") && line.trim().ends_with(bound)).count()
    };
    // The copied test reads the counter, not a step computed beside its update:
    // `cmp n, 3`, not `lea t, [n-1]; cmp t, 2`.
    let tests = compares(", 3") + compares(", 4");
    assert!(compares(", 2") <= 1, "{} tests of a step\n{}", compares(", 2"), function.join("\n"));
    assert!(tests >= 6, "{tests} tests of n\n{}", function.join("\n"));
}

/// `if (n == 0) return acc; ... n - 1` behind the copied guard is `do { } while
/// (n - 1 != 0)`, which the induction count proved only for a start the guard
/// rules out as the bound; unproven, LSR left the loop on `n` (`lea; cmp ebx,
/// 0; jne` and a `lea` for `3n` each trip: recsum +5.3% clocks, 26009 to 30010
/// instructions) where the loop without the copy ends on the flags of the step
/// of `3n` (`add ebx, -3; jne`).
#[test]
fn test_a_loop_behind_a_guard_that_rules_out_its_bound_ends_on_the_flags_of_its_step() {
    let source = format!("{}/bench/recsum/recsum.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with("add ") && pair[0].ends_with(", -3") && pair[1].starts_with("jne ")),
        "{text}"
    );
}

/// `if (n == 0) return acc; ... n - 1` behind the copied guard is `do { } while
/// (n - 1 != 0)`, which the induction count proved only for a start the guard
/// rules out as the bound; unproven, LSR left the loop on `n` (`lea; cmp ebx,
/// 0; jne` and a `lea` for `3n` each trip: recsum +5.3% clocks, 26009 to 30010
/// instructions) where the loop without the copy ends on the flags of the step
/// of `3n` (`add ebx, -3; jne`).
#[test]
fn test_a_loop_behind_a_guard_that_rules_out_its_bound_ends_on_the_flags_of_its_step() {
    let source = format!("{}/bench/recsum/recsum.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with("add ") && pair[0].ends_with(", -3") && pair[1].starts_with("jne ")),
        "{text}"
    );
}
