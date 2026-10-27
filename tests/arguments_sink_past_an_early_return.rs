//! Incoming stack arguments are loaded where they are used.

mod common;

/// x_hanoi2's `mv`, as cdecl: `n` is tested at once, and `from`, `to` and `via`
/// are read after the early return for `n == 0`.
const SOURCE: &str = r#"static long moves; static int peg[3][16], top[3];
void mv(int n, int from, int to, int via)
{
    if (n == 0) return;
    mv(n - 1, from, via, to);
    peg[to][top[to]++] = peg[from][--top[from]]; ++moves;
    mv(n - 1, via, to, from);
}
"#;

/// `mv` loaded all four arguments in its entry block, before the test of `n`,
/// so the call that returns at once paid for three loads it did not need. The
/// pin's MachineSink moves a load of an incoming argument (an immutable fixed
/// stack object: a frame index, no register read, and invariant) into the one
/// block that uses it.
#[test]
fn an_argument_is_not_loaded_before_the_early_return_that_does_not_use_it() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body = text.split("_mv proc").nth(1).and_then(|rest| rest.split("_mv endp").next()).expect("mv");
    let before_test: Vec<&str> = body.lines().take_while(|line| !line.trim_start().starts_with("test ")).collect();
    let loads: Vec<&&str> = before_test.iter().filter(|line| line.contains("dword ptr [esp+")).collect();
    assert!(loads.len() <= 1, "arguments loaded before the test, beside n: {loads:?}\n{body}");
}
