//! ShrinkWrap places the whole prologue: a function that leaves before it
//! touches its frame builds none for that path.

mod common;

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
    let output =
        common::cc::llrm_cc().current_dir(scratch.path()).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// The paths from the entry to a return, each cut at its first conditional
/// jump's two ways: the one laid next and the one branched to.
fn paths(listing: &str) -> Vec<String> {
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let first_return = |from: usize| lines[from..].iter().position(|line| line.starts_with("ret")).map(|at| from + at);
    let entry = lines.iter().position(|line| line.ends_with(':')).expect("a block");
    let jump =
        lines[entry..].iter().position(|line| line.starts_with('j') && !line.starts_with("jmp")).map(|at| entry + at);
    let mut found = Vec::new();
    if let Some(end) = first_return(entry) {
        found.push(lines[entry..=end].join("\n"));
    }
    if let Some(jump) = jump {
        let label = lines[jump].split_whitespace().nth(1).unwrap();
        if let Some(target) = lines.iter().position(|line| *line == format!("{label}:"))
            && let Some(end) = first_return(target)
        {
            found.push(format!("{}\n{}", lines[entry..=jump].join("\n"), lines[target..=end].join("\n")));
        }
    }
    found
}

/// Every call paid `sub esp, 84` (m32, stack-addressed) or `push bp / mov bp,
/// sp / sub sp, 40` (m16, framed) before the test that leaves at once (gcc and
/// LLVM set the frame up after it).
#[test]
fn test_the_path_that_leaves_before_the_frame_is_used_builds_none() {
    for flags in [&["-O2", "-m32", "-march=i486"][..], &["-O2", "-m16"]] {
        let text = listing(flags);
        // The early path is whichever of the two ways the placement lays first.
        assert!(
            paths(&text)
                .iter()
                .any(|path| !path.contains("sub esp") && !path.contains("sub sp") && !path.contains("push bp")),
            "{flags:?}:\n{text}"
        );
        assert!(
            text.contains("sub esp, 84") || text.contains("sub sp, 40"),
            "the late path still builds its frame: {flags:?}:\n{text}"
        );
    }
}
