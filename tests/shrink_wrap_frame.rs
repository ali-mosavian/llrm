//! ShrinkWrap places the whole prologue: a function that leaves before it touches its frame builds none for that path.

use std::process::Command;

const KERNEL: &str = "
extern int g(int *);
int f(int n)
{
    int a[20];
    if (n == 0) return 7;
    a[3] = n;
    return g(a) + a[3];
}
";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The text up to the first return: the path `n == 0` takes.
fn early_path(listing: &str) -> String {
    let end = listing.lines().position(|line| line.trim().starts_with("ret")).expect("a return");
    listing.lines().take(end + 1).collect::<Vec<_>>().join("\n")
}

/// Every call paid `sub esp, 84` (m32, stack-addressed) or `push bp / mov bp, sp / sub sp, 40` (m16, framed) before the
/// test that leaves at once (gcc and LLVM set the frame up after it).
#[test]
fn test_the_path_that_leaves_before_the_frame_is_used_builds_none() {
    for flags in [&["-O2", "-m32", "-march=i486"][..], &["-O2", "-m16"]] {
        let text = listing(flags);
        let early = early_path(&text);
        assert!(
            !early.contains("sub esp") && !early.contains("sub sp") && !early.contains("push bp"),
            "{flags:?}:\n{text}"
        );
        assert!(
            text.contains("sub esp, 84") || text.contains("sub sp, 40"),
            "the late path still builds its frame: {flags:?}:\n{text}"
        );
    }
}
