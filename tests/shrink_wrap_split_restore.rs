//! A return that the early exit shares with the late path is not a reason to
//! save registers at the entry.

mod common;

/// x_hanoi2's `mv`, called from outside so that it takes its first two
/// arguments in registers: the loop falls into the return the `n == 0` test
/// jumps to.
const LOOP: &str = r#"static long moves; static int peg[3][16], top[3];
static void mv(int n, int from, int to, int via)
{
    if (n == 0) return;
    mv(n - 1, from, via, to);
    peg[to][top[to]++] = peg[from][--top[from]]; ++moves;
    mv(n - 1, via, to, from);
}
long bench(int n)
{
    mv(n, 0, 2, 1);
    return moves * 100 + top[2];
}
"#;

/// The late path is straight code that jumps to a join the early path reaches
/// too.
const JOIN: &str = r#"extern int flag, total, other;
extern int g(int);
int f(int n)
{
    int r = 0;
    if (flag) r = g(n) + g(n + 1);
    total += r;
    other ^= r * 3;
    return r;
}
"#;

fn listing(
    source: &str,
    name: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    text.split(&format!("_{name} proc"))
        .nth(1)
        .and_then(|rest| rest.split(&format!("_{name} endp")).next())
        .expect(name)
        .to_owned()
}

/// Both pushed their registers before the test and popped them on the one path
/// that did, so a call with `n == 0` paid for four pushes and four pops
/// (x_hanoi2 1.236 against the pin). The pin's restore is split: the late path
/// pops on its way into the shared `ret`, the early path jumps over the pops.
fn pays_nothing_on_the_early_path(body: &str) {
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    let jump =
        lines.iter().position(|line| line.starts_with("je ") || line.starts_with("jz ")).expect("the early exit");
    assert!(lines[..jump].iter().all(|line| !line.starts_with("push ")), "a push before the test:\n{body}");
    let target = lines[jump].split_whitespace().nth(1).expect("a label");
    let early = lines.iter().position(|line| *line == format!("{target}:")).expect("its label");
    assert!(lines[early..].iter().all(|line| !line.starts_with("pop ")), "a pop on the early path:\n{body}");
    assert!(lines[jump + 1..early].iter().any(|line| line.starts_with("pop ")), "no pop on the late path:\n{body}");
}

#[test]
fn a_loop_that_falls_into_the_shared_return_pops_there_and_the_early_exit_pays_nothing() {
    pays_nothing_on_the_early_path(&listing(LOOP, "mv"));
}

#[test]
fn a_late_path_that_jumps_to_a_join_pops_before_the_jump_and_the_early_exit_pays_nothing() {
    pays_nothing_on_the_early_path(&listing(JOIN, "f"));
}

/// The pin shrink-wraps at -Os as well (clang -Os -m32 -march=i486 on this
/// loop: `testl %ecx, %ecx; je` first, the four pushes after it), so the saves
/// follow the test there too. The earlier rule that kept them at the entry at
/// -Os was a deviation from the pin.
#[test]
fn at_os_the_registers_are_saved_after_the_test_as_the_pin_does() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), LOOP).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-Os", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body = text.split("_mv proc").nth(1).and_then(|rest| rest.split("_mv endp").next()).expect("mv");
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    let first_push = lines.iter().position(|line| line.starts_with("push ")).expect("a push");
    let test = lines.iter().position(|line| line.starts_with("test ")).expect("the test");
    assert!(test < first_push, "-Os saved before the test:\n{body}");
}
