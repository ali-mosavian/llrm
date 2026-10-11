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
    // Six levels of the nest test `n` against 2, 3 or 4 (the copied tests too).
    let tests = compares(", 2") + compares(", 3") + compares(", 4");
    assert!(tests >= 12, "{tests} tests of n\n{}", function.join("\n"));
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

/// mandel's `x * x >> 8` is a 32-bit `imul` and `sar` where the ranges prove
/// the product fits. Behind the copy, they lost the bound of `x` (its header
/// computes what the loop tested, and the step is tested after the increment),
/// so the fixed-point product stayed `imul` + `shrd` (+32.5% instructions).
#[test]
fn test_a_fixed_point_product_in_a_rotated_loop_is_still_narrowed() {
    let source = format!("{}/bench/mandel/mandel.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("shrd"), "{text}");
}

/// With the header copied, mandel's counter leaves on `i + 1 < 32` false with
/// 32. The exit read the stepped counter, which kept the old one alive past its
/// step: `lea eax, [edx+1]; or eax, eax; je; mov edx, eax` for the trip
/// where `inc edx; je` does.
#[test]
fn test_a_counted_exits_number_lets_its_counter_step_in_place() {
    let source = format!("{}/bench/mandel/mandel.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let in_place = lines.windows(2).any(|pair| pair[0].starts_with("inc e") && pair[1].starts_with("je "));
    assert!(in_place, "{text}");
}

/// `for (x = 1; x < n - 1; ++x)` behind its copied header is counted by the
/// guard `n > 2` (`1 < n - 1`): its counter walks to zero (`inc; jne`). The
/// guard was read only at the constants it stood at, and the loop kept its
/// `cmp; jl` on a test of the counter (x_life -O3 +4.3% clocks).
#[test]
fn test_a_loop_behind_a_guard_at_other_constants_is_counted() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "static unsigned char g[2][24][24];
void f(int n, int cur, int y)
{
    int x;
    for (x = 1; x < n - 1; ++x) {
        int c = g[cur][y][x - 1] + g[cur][y][x] + g[cur][y][x + 1];
        g[1 - cur][y][x] = (unsigned char)(c == 3);
    }
}
",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O3", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let counted = lines.windows(2).any(|pair| pair[0].starts_with("inc e") && pair[1].starts_with("jne "));
    assert!(counted, "{text}");
}

/// mandel's `iteration < 32` behind the copied header, once its exit reads a
/// number, tested the counter before the step (`cmp ax, 31`), so the step took
/// a register of its own: `inc bx; cmp ax, 31` and a copy back. The test of the
/// step is `inc ax; cmp ax, 32`: one register (mandel C -O2 m16 was +10.1%
/// instructions). The break reads a table, so only the loop's own test is
/// copied.
#[test]
fn test_a_latch_test_of_a_counter_whose_step_nothing_else_reads_tests_the_step() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "static short tab[8];
long f(void)
{
    long x = 0, y = 0;
    short it;
    for (it = 0; it < 32; ++it) {
        long xx = (x * x) >> 8;
        long yy = (y * y) >> 8;
        if (tab[(xx + yy) & 7] > 1024) break;
        y = ((x * y) >> 7) + 3;
        x = xx - yy + 5;
    }
    return it;
}
",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let stepped = lines
        .windows(2)
        .any(|pair| pair[0].starts_with("inc ") && pair[1].starts_with("cmp ") && pair[1].ends_with(", 32"));
    assert!(stepped, "{text}");
}

/// gcc copies the chain of header tests (`copy_headers`: each block after the
/// first with one predecessor and no call, within 20 insns), not only the
/// loop's own: `for (d = 2; d * d <= i; ++d) if (i % d == 0) break;` has
/// both its tests copied, so d = 2 folds into them (`test esi, 1`, `cmp esi,
/// 4`) and the idiv stays for the later trips. x_primes -O1 +25% over gcc with
/// one test copied.
#[test]
fn test_a_loop_with_two_tests_that_leave_it_has_both_copied() {
    let text = listing("x_primes", &["-ftree-ch", "-O1"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let folded = lines.iter().any(|line| line.starts_with("test e") && line.ends_with(", 1"));
    assert!(folded, "{text}");
}

/// With no flag the copy is on and `Hoist` runs before the counters are
/// chosen: `sum_three`'s three row loads leave the loop and its counter runs
/// to zero, `add ax, [p+i] x3; add i, 2; jne` (5 instructions a trip). Hoist
/// after LSR, or the copy off, left `inc ecx` beside `add edi, 2` (6).
#[test]
fn test_sum_three_loop_is_five_instructions_by_default() {
    let source = format!("{}/bench/parity/sum_three/sum_three.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let loop_ = lines
        .windows(5)
        .any(
            |w| w[..3].iter().all(|line| line.starts_with("add ax, word ptr"))
                && w[3].starts_with("add e")
                && w[3].ends_with(", 2")
                && w[4].starts_with("jne "),
        );
    assert!(loop_, "{text}");
}

/// Tuned for size the copy is off and the loop is entered at its body only
/// after the last `Rotate`: `Hoist` ahead of it found no loop it could take
/// the three row loads out of, and the loop reloaded `[row+4]` each trip
/// (`mov ebp, [ecx+4]; add ax, [ebp+edi]` three times).
#[test]
fn test_tuned_for_size_the_row_loads_still_leave_the_loop() {
    let source = format!("{}/bench/parity/sum_three/sum_three.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let loop_ = lines.windows(3).any(|w| w.iter().all(|line| line.starts_with("add ax, word ptr")));
    assert!(loop_, "{text}");
}
