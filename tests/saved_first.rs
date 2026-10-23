//! A flat m32 frame pushes its callee-saved registers before it reserves the
//! locals, as the pin does (`push ebp; mov ebp, esp; push esi; sub esp, N`).

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
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
