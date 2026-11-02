//! A copy of an address of registers is the address again
//! (`reMaterializeTrivialDef`).

mod common;

/// bench/particle's fill loop: `i - 50` is stored, and masked twice. The
/// coalescer kept one `lea` and copied it (`mov edi, esi`) for the second
/// reader; the pin makes the `lea` again where the copy was.
///
/// vsgcc's x_isqrt, where this was first seen, no longer shows it: its compare
/// and subtract became one `sub` when the select became a branch.
#[test]
fn a_lea_read_by_two_uses_is_made_again_not_copied() {
    let source = r#"short px[101], py[101], pz[101], vx[101], vy[101], vz[101];

void fill(void)
{
    short i, p;

    for (i = 0; i <= 100; ++i) {
        p = i - 50;
        px[i] = p; py[i] = 2 * p; pz[i] = -p;
        vx[i] = p & 7; vy[i] = 3 - (p & 3); vz[i] = 1;
    }
}
"#;
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    // the loop: from the target of the last jump back, widened by jumps inside
    // it that go further back
    let back: Vec<(usize, usize)> = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| {
            let target = line.strip_prefix('j')?.split_once(' ')?.1;
            let head = lines[..at].iter().position(|one| one.strip_suffix(':') == Some(target))?;
            Some((head, at))
        })
        .collect();
    let &(mut first, last) = back.last().expect("a jump back");
    for &(head, at) in &back {
        if (first..=last).contains(&at) {
            first = first.min(head);
        }
    }
    assert!(first < last, "the loop: {asm}");
    let body = &lines[first..=last];
    let leas = body.iter().filter(|line| line.starts_with("lea ")).count();
    assert_eq!(leas, 2, "the loop makes the address twice:\n{asm}");
    let copies: Vec<&&str> = body
        .iter()
        .filter(|line| {
            line.strip_prefix("mov ")
                .and_then(|operands| operands.split_once(", "))
                .is_some_and(|(to, from)| to.starts_with('e') && from.starts_with('e'))
        })
        .collect();
    assert!(copies.is_empty(), "the loop copies {copies:?}:\n{asm}");
}
