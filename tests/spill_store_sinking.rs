//! A spill store runs only on the path that reads the slot.

mod common;

/// hanoi's `n == 0` exit stored the slot of the accumulator before the test, a
/// slot only the loop path reads: the exit touched the frame, so no prologue
/// could be placed after it (x_hanoi at -O2 ran 19 instructions for each leaf
/// call gcc ran in 3). Its calls touch memory here (a visit counter): with
/// none, the two equal calls are one and the recursion has no loop.
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
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_CANDIDATES", "spiller")
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let entry: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("_hanoi proc"))
        .skip_while(|line| !line.trim().starts_with("L0_0:"))
        .take_while(|line| !line.trim().starts_with('j'))
        .collect();
    assert!(!entry.is_empty(), "no entry block: {text}");
    assert!(entry.iter().all(|line| !line.contains("[esp+")), "the entry stores a slot: {entry:?}\n{text}");
}
