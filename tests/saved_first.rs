//! A flat m32 frame pushes its callee-saved registers before it reserves the
//! locals, as the pin does (`push ebp; mov ebp, esp; push esi; sub esp, N`).

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// The indirect call keeps the frame register; three registers are kept.
const SOURCE: &str = r#"extern int g(int *, int);
int h(int n, int *p, int (*fp)(int *, int))
{
    int a[8];
    int s = 0;
    int i;
    for (i = 0; i < n; i++) { a[i & 7] = p[i]; s += fp(a, i); s += g(a, s); }
    return s + a[3];
}
"#;

fn instructions(text: &str) -> Vec<String> {
    text.lines().map(|line| line.trim().to_owned()).filter(|line| !line.is_empty() && !line.starts_with('.')).collect()
}

/// The registers were pushed after `sub esp`, so every local sat above them and
/// the epilogue popped them before it released the locals.
#[test]
fn test_callee_saved_registers_are_pushed_before_the_locals_are_reserved() {
    let text = listing(SOURCE, &["-O2", "-m32", "-mabi=sysv", "-march=i486"]);
    let lines = instructions(&text);
    let entry = lines.iter().position(|line| line == "push ebp").expect("a frame");
    assert_eq!(lines[entry + 1], "mov ebp, esp");
    let sub = lines.iter().position(|line| line.starts_with("sub esp,")).expect("locals");
    assert!(lines[entry + 2..sub].iter().all(|line| line.starts_with("push ")), "{lines:?}");
    assert!(sub > entry + 2, "registers pushed before sub esp: {lines:?}");
    let ret = lines.iter().position(|line| line == "ret").expect("a return");
    assert!(lines[ret - 5].starts_with("add esp,"), "locals released first: {:?}", &lines[ret - 6..=ret]);
    assert_eq!(lines[ret - 1], "pop ebp");
}

/// Three registers pushed first moved the locals 12 bytes down, so a double
/// array sat at 4 mod 8 from BP: nbody -O2 took +15% clocks on the 486 (3
/// clocks per misaligned access).
#[test]
fn test_locals_stay_8_aligned_below_three_pushed_registers() {
    let source = r#"extern int g(double *, int);
int h(int n, int *p, int (*fp)(double *, int))
{
    double a[3];
    int s = 0;
    int i;
    a[0] = 1.5;
    for (i = 0; i < n; i++) { a[i % 3] = p[i]; s += fp(a, i); s += g(a, s); }
    return s + (int)a[1];
}
"#;
    let text = listing(source, &["-O2", "-m32", "-mabi=sysv", "-march=i486"]);
    let array = text.lines().find_map(|line| line.trim().strip_prefix("lea ecx, [ebp-")).expect("the array's address");
    let distance: i64 = array.trim_end_matches(']').parse().unwrap();
    assert_eq!(distance % 8, 0, "{text}");
}

/// A wrapped frame (set up where it is first used, not at the entry) pushed
/// its registers after `sub esp`, the old order, because the wrap kept the
/// frame's own sequence apart from them: the Save block now has the pin's
/// order and the early return builds none.
#[test]
fn test_a_shrink_wrapped_frame_pushes_its_registers_before_it_reserves() {
    let source = r#"extern int g(int *);
int h(int n, int *p)
{
    int a[20];
    int i, s = 0;
    if (n == 0) return 7;
    for (i = 0; i < n; i++) { a[i % 20] = p[i]; s += g(a); }
    return s + a[3];
}
"#;
    let text = listing(source, &["-O2", "-m32", "-march=i486"]);
    let lines = instructions(&text);
    let first_push = lines.iter().position(|line| line.starts_with("push ")).expect("a push");
    let sub = lines.iter().position(|line| line.starts_with("sub esp,")).expect("locals");
    assert!(lines[..first_push].iter().any(|line| line.starts_with('j')), "the early return comes first: {lines:?}");
    assert!(
        lines[first_push..sub].iter().all(|line| line.starts_with("push ") || line.ends_with(':')),
        "registers pushed before sub esp: {lines:?}"
    );
}
