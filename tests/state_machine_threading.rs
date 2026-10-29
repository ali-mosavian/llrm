//! A switch on a loop's state is threaded into the paths that decide it from
//! -O2, as gcc's `pass_thread_jumps` (`flag_thread_jumps &&
//! flag_expensive_optimizations`); -O1 has only the early threader, which
//! copies nothing.

mod common;

const MACHINE: &str = "extern void report(long value);\n\
    static int step(int s, int c)\n\
    {\n\
        switch (s) {\n\
        case 0: return c < 64 ? 1 : (c < 128 ? 2 : 0);\n\
        case 1: return c & 1 ? 3 : 0;\n\
        case 2: return c > 200 ? 4 : 1;\n\
        case 3: return (c & 6) == 6 ? 2 : 5;\n\
        case 4: return c ^ 3 ? 0 : 5;\n\
        default: return c < 10 ? 0 : 3;\n\
        }\n\
    }\n\
    long run(int n)\n\
    {\n\
        int i, s = 0; long sum = 0; unsigned r = 9;\n\
        for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; s = step(s, (int)((r >> 16) & 255)); sum += s * (i & 3); }\n\
        return sum;\n\
    }\n";

/// How many times the loop's `r = r * a + 12345` runs in the listing (its
/// constant, as an immediate or a `lea` displacement): once per path the state
/// machine was split into.
fn copies_of_the_step(level: &str) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), MACHINE).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(directory).args(["-m32", level, "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap().matches("12345").count()
}

/// -O1 threaded the switch into 13 copies of the loop's tail (x_switch: 1405
/// bytes at -O1, 402 now; gcc -O1 312), which only -O2 and up do.
#[test]
fn a_state_machine_is_not_split_into_its_paths_at_o1() {
    assert_eq!(copies_of_the_step("-O1"), 1);
    assert!(copies_of_the_step("-O2") > 1);
}
