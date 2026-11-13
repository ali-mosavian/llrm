//! A spill store runs only on the path that reads the slot.

mod common;

/// hanoi's `n == 0` exit stored the slot of the accumulator before the test, a
/// slot only the loop path reads: the exit touched the frame, so no prologue
/// could be placed after it (x_hanoi at -O2 ran 19 instructions for each leaf
/// call gcc ran in 3). Its calls touch memory here (a visit counter): with
/// none, the two equal calls are one and the recursion has no loop.
///
/// llrm-cc: clang's IR turns the second recursive call into a loop, which
/// holds `hanoi` in registers: no frame slot at all, as in the pin's `llc` on
/// the same IR. The entry is held to no slot and the function to no more slot
/// accesses than the pin's (rewritten on observable output; the entry's label
/// is `L<n>_0` by the function's place in the module).
const SOURCE: &str = "extern void report(long value);
volatile int visits;

static int hanoi(int n, int a, int b, int c)
{
    if (n == 0) return 0;
    visits = n;
    return hanoi(n - 1, a, c, b) + 1 + hanoi(n - 1, c, b, a);
}

long bench_hanoi(int n)
{
    return hanoi(n, 1, 3, 2);
}

int main(void)
{
    report(bench_hanoi(13));
    return 0;
}
";

#[test]
fn test_a_spill_store_ahead_of_a_branch_is_made_on_the_path_that_reads_the_slot() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("hanoi.c");
    std::fs::write(&source, SOURCE).unwrap();
    let flags = ["-O2", "-m32", "-mabi=sysv", "-march=i486"];
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_CANDIDATES", "spiller")
        .args(flags)
        .args(["-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let ours = common::pin::instructions(&text, Some("hanoi"));
    let entry: Vec<&String> = ours.iter().take_while(|line| !line.starts_with('j')).collect();
    assert!(!entry.is_empty(), "no entry block: {text}");
    assert!(entry.iter().all(|line| !line.contains("[esp+")), "the entry stores a slot: {entry:?}\n{text}");
    let slots = |lines: &[String]| lines.iter().filter(|line| line.contains("[esp")).count();
    let pin = common::pin::instructions(&common::pin::listing(&source, &flags), Some("hanoi"));
    assert!(!pin.is_empty(), "premise: the pin's listing holds hanoi");
    assert!(slots(&ours) <= slots(&pin), "{} frame accesses, the pin's llc has {}: {text}", slots(&ours), slots(&pin));
}
