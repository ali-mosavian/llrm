//! What the header copy (`-ftree-ch`) leaves for the passes after it.

mod common;

use std::path::Path;
use std::process::Command;

fn listing(
    kernel: &str,
    flags: &[&str],
) -> String {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486"])
        .args(flags)
        .args(["-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `for (j = i - 1; j >= 0 && a[j] > v; --j)` behind the copy: the phi combine
/// (#4888) tests `j + 1 > 1`, so the step ends `dec; lea; cmp $1; jg`. The
/// pin's best on the C is four instructions from the step to the branch (`dec;
/// add -4; cmp $1; jg`); so is this. The IR before #4888 gave `dec; jge`; a
/// fold past the pin (the compare on the biased counter) is a later row
/// (#4952).
#[test]
fn test_a_loop_entered_behind_a_folded_guard_steps_and_branches_in_the_pins_four() {
    let text = listing("x_insertion", &["-ftree-ch"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // The pin's `cmp r, 1; jg` or, as now, the step's own flags: `dec; jg`.
    let branch = (0..lines.len())
        .find(|&at| {
            lines[at].starts_with("jg ") && lines[..at].iter().rev().take(3).any(|line| line.starts_with("dec e"))
        })
        .unwrap_or_else(|| panic!("no `dec r; ..; jg` ends the inner loop: {text}"));
    let step = lines[..branch].iter().rposition(|line| line.starts_with("dec e")).unwrap();
    assert!(
        branch - step < 4,
        "{} instructions from the step to the branch, the pin's best is 4: {text}",
        branch - step + 1
    );
}

/// gcc copies headers after `pass_ipa_inline` (passes.def:232), so the inliner
/// sizes a body before its test is copied. The self-inlined nest of six levels
/// this once pinned (2004 calls) is gone on purpose: the walk inliner does not
/// copy fib into itself (#3094, as the pin's does not). What the pin leaves is
/// ONE `call fib` and a loop (accumulator recursion elimination); so does this.
#[test]
fn test_fib_with_the_header_copy_on_keeps_its_self_inlined_nest() {
    let text = listing("x_fib", &["-ftree-ch"]);
    let function: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("_fib proc"))
        .take_while(|line| !line.starts_with("_fib endp"))
        .collect();
    let calls = function.iter().filter(|line| line.trim().starts_with("call _fib")).count();
    assert!(calls == 1, "{calls} recursive calls where the pin has one and a loop\n{}", function.join("\n"));
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
    let output = common::cc::llrm_c()
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
///
/// llrm-cc: the break reads the counter, so it cannot run to zero; the step
/// is tested stepped, `inc eax; cmp eax, 32; jne` (pin-shape: what LLVM leaves
/// of a counter that is also read after the loop).
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
    // The step and the test are of one register, whichever way the placement
    // lays the exit (`je exit; jmp top` or `jne top; jmp exit`).
    let in_place = lines
        .windows(4)
        .any(
            |quad| {
                quad[0]
                    .strip_prefix("inc e")
                    .is_some_and(
                        |register| {
                            // a `mov` may sit between the test and its branch;
                            // it keeps the flags
                            quad[1] == format!("cmp e{register}, 32")
                                && quad[2..].iter().any(|line| line.starts_with("je ") || line.starts_with("jne "))
                        },
                    )
            },
        );
    assert!(in_place, "{text}");
    assert!(!text.contains("lea eax, [edx+1]"), "the old counter is kept alive past its step: {text}");
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
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O3", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // The pin ends the loop on the equality of the stepped counter and the
    // bound (`incl; cmpl; jne`), not on a `cmp; jl` of the counter: the
    // trip count is known from the guard `n > 2`.
    let counted = lines
        .windows(3)
        .any(|trio| trio[0].starts_with("inc e") && trio[1].starts_with("cmp e") && trio[2].starts_with("jne "));
    assert!(counted, "{text}");
    assert!(!lines.iter().any(|line| line.starts_with("jl ")), "the counter is tested with jl: {text}");
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
        // The start is a parameter and `tab` is written elsewhere: with x = y =
        // 0 the orbit is the fixed point (5, 3), and a table nothing writes is
        // constant, so the loop folded to `mov eax, 32`.
        "static short tab[8];
long f(long x, long y)
{
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
void g(void) { tab[1] = 2000; }
",
    )
    .unwrap();
    let output = common::cc::llrm_c()
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
/// both its tests copied, so d = 2 folds into the second (`cmp esi, 4; jb`)
/// and the idiv stays for the later trips. x_primes -O1 +25% over gcc with
/// one test copied. The pin folds the same `cmp; jb` but not the `test esi, 1`
/// gcc gets from peeling d = 2, and ours follows the pin.
#[test]
fn test_a_loop_with_two_tests_that_leave_it_has_both_copied() {
    let text = listing("x_primes", &["-ftree-ch", "-O1"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // A `mov` the scheduler puts between the compare and its branch leaves the
    // flags alone.
    let folded = (0..lines.len()).any(|at| {
        lines[at].starts_with("cmp e")
            && lines[at].ends_with(", 4")
            && lines[at + 1..].iter().find(|line| !line.starts_with("mov ")).is_some_and(|line| line.starts_with("jb "))
    });
    assert!(folded, "{text}");
}

/// With no flag the copy is on and `Hoist` runs before the counters are
/// chosen: `sum_three`'s three row loads leave the loop and its counter runs
/// to zero, `add ax, [p+i] x3; add i, 2; jne` (5 instructions a trip). Hoist
/// after LSR, or the copy off, left `inc ecx` beside `add edi, 2` (6).
///
/// llrm-cc: the five was llrm-c's; the pin's `llc` on the same IR runs
/// `add x3; inc; cmp; jne`, six. The loop is held to the pin's length
/// (rewritten on observable output).
#[test]
fn test_sum_three_loop_is_five_instructions_by_default() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/parity/sum_three/sum_three.c");
    let flags = ["-O2", "-m32", "-mabi=sysv", "-march=i486"];
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let ours = common::pin::loops(&text, Some("sum_three"));
    let pin = common::pin::listing(&source, &flags);
    let pins = common::pin::loops(&pin, Some("sum_three"));
    assert_eq!(pins.len(), 1, "the pin has one loop in sum_three: {pin}");
    assert!(
        ours.len() == 1 && ours[0] <= pins[0],
        "loops of {ours:?} instructions, the pin's llc has {pins:?}: {text}"
    );
}

/// Tuned for size the copy is off and the loop is entered at its body only
/// after the last `Rotate`: `Hoist` ahead of it found no loop it could take
/// the three row loads out of, and the loop reloaded `[row+4]` each trip
/// (`mov ebp, [ecx+4]; add ax, [ebp+edi]` three times).
#[test]
fn test_tuned_for_size_the_row_loads_still_leave_the_loop() {
    let source = format!("{}/bench/parity/sum_three/sum_three.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
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

/// A loop entered behind its copied tests leaves from its header, where the
/// counter is tested after the step that is made for it anyway: `lea cx,
/// [si+1]; cmp si, 31; ...; mov si, cx` held the counter in two registers.
/// gcc's ivopts tests the stepped value, `inc dx; cmp dx, 32`. mandel's m16
/// loop ran 200737 instructions against 172712 with the copy off.
#[test]
fn test_a_counter_tested_in_the_header_after_its_step_is_tested_stepped() {
    let source = format!("{}/bench/mandel/mandel.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let stepped = lines
        .windows(2)
        .any(|pair| pair[0].strip_prefix("inc ").is_some_and(|register| pair[1] == format!("cmp {register}, 32")));
    assert!(stepped, "{text}");
}

/// A path that returns early stored its result in the frame cell the other
/// paths' sum lives in, and the shared return reloaded it: the path touched the
/// frame, so the registers and the frame were set up on it too (queens' leaf
/// call: `push si; push di; ... pop di; pop si; leave`, the most frequent
/// path). With the return copied for the path, the reload is forwarded and the
/// store dead before the pieces it needs are counted: `mov ax, 1; ret`.
///
/// llrm-cc: the pin's llc on the same IR (clang -O2 -m32, `place`) keeps the
/// frame and the cell on this path too: `mov [cell], 1; cmp; je exit`, the exit
/// reloading the cell. Held to that shape, not past it.
#[test]
fn test_an_early_return_through_a_spilled_sum_sets_up_nothing() {
    let source = format!("{}/bench/queens/queens.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.ends_with(':')).collect();
    let early = lines
        .windows(3)
        .any(
            |trio| trio[0].starts_with("mov word ptr [bp-")
                && trio[0].ends_with("], 1")
                && trio[1].starts_with("cmp ")
                && trio[2].starts_with("je "),
        );
    assert!(early, "{text}");
}

/// Behind the copy the old `JumpThread` cleaned the function with `Algebraic`
/// after LSR, and `icmp ne (x + 1), 2` became `icmp ne x, 1` for the counter's
/// own next value: `lea eax,[ecx+1]; add edi,24; cmp ecx,1; mov ecx,eax; jne`,
/// the counter in two registers (x_life +5% clocks). The step is tested where
/// it is made: `inc ecx; cmp n, ecx; je`.
#[test]
fn test_a_counted_inner_loop_tests_its_stepped_counter_after_the_cleanup() {
    let text = listing("x_life", &["-ftree-ch"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // The 3x3 neighbour loops are unrolled, as in the pin; what is left is the
    // x loop, which the pin ends on `inc eax; cmp [n], eax; je` as ours does.
    let stepped = lines
        .windows(3)
        .any(
            |w| w[0].strip_prefix("inc ").is_some_and(|register| w[1].starts_with("cmp ") && w[1].ends_with(register))
                && (w[2].starts_with("je ") || w[2].starts_with("jne ")),
        );
    assert!(stepped, "{text}");
}

/// A register only the loop uses was saved at the entry when the copy's guard
/// skipped the loop and joined the returns after it: the early return
/// (`hi - lo < 2`, most calls of recchop) paid `push ebp; push edi` and their
/// pops. The save point goes up the dominators to the block every return it
/// closes at is below: two registers at the entry, as with the copy off.
///
/// llrm-cc: clang's IR turns the second recursive call into a loop, which
/// holds four registers across the early return; the pin's `llc` on the same
/// IR pushes four at the entry as well, so the entry is held to the pin's count
/// (pin-shape).
#[test]
fn test_a_register_only_a_guarded_loop_uses_is_saved_after_the_early_return() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/recchop/recchop.c");
    let flags = ["-O1", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch"];
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let before_early_return = |listing: &str| {
        common::pin::instructions(listing, Some("chop"))
            .iter()
            .take_while(|line| !line.starts_with("jl "))
            .filter(|line| line.starts_with("push "))
            .count()
    };
    let (entry, pins) = (before_early_return(&text), before_early_return(&common::pin::listing(&source, &flags)));
    assert!(pins > 0 && entry <= pins, "{entry} pushes before the early return, the pin's llc has {pins}\n{text}");
}

/// A loop the guard enters at its second block (`while (at >= gap &&
/// values[at - gap] > value)`: the test of the value follows the guard) was
/// laid out [guard][body][test] and entered by a `jmp` to the test, one taken
/// jump a loop entry (shellsort -O1 +3.5% instructions). Turned to start at the
/// block it is entered at, the guard falls into the test and the bottom test
/// jumps back, as gcc's `rotate_loop` does for a loop entered in its middle.
/// The pin keeps three jumps in the function (two outer-loop entries and the
/// inner loop's exit back); ours has the same three.
#[test]
fn test_a_loop_entered_at_its_second_block_is_laid_out_from_it() {
    let source = format!("{}/bench/shellsort/shellsort.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O1", "-m32", "-mabi=sysv", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let jumps = text
        .lines()
        .map(str::trim)
        .skip_while(|line| !line.starts_with("_bench_shellsort proc"))
        .take_while(|line| !line.starts_with("_bench_shellsort endp"))
        .filter(|line| line.starts_with("jmp "))
        .count();
    assert!(jumps <= 3, "{jumps} jumps, the pin has 3\n{text}");
}
