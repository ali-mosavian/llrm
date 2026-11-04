//! A step of a compared value sets the compare's flags.

mod common;

fn listing(kernel: &str) -> String {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// x_insertion's `n - 1 > 0` guard was `lea eax, [ecx-1]; cmp ecx, 1; jle`: the
/// compare read `n` after the step had defined `n - 1`, so the step could not
/// be made in place and the flags were made twice. With the header copy the
/// guard is the compare of `n` alone, and no `lea` steps it.
#[test]
fn test_a_guard_on_a_value_and_its_step_is_one_instruction_and_its_flags() {
    let text = listing("x_insertion");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // The guard is a compare of `n` against 1 and a jump on it (the pin's
    // order has `n <= 0` leave first, so the jump is `jne` now, was `jle`).
    let guard = lines
        .windows(2)
        .find(|pair| pair[0].starts_with("cmp ") && pair[0].ends_with(", 1") && pair[1].starts_with('j'))
        .unwrap_or_else(|| panic!("{text}"));
    // No `lea` steps the register the guard compares (`j = i - 1` is another
    // value, in another register).
    let compared = guard[0].strip_prefix("cmp ").and_then(|rest| rest.split(',').next()).unwrap();
    let step = format!("[{compared}-1]");
    assert!(!lines.iter().any(|line| line.starts_with("lea ") && line.ends_with(&step)), "{text}");
}

/// sieve's `multiple += i` loop was `lea ax, [edi+esi]; cmp di, dx; mov di, ax;
/// jb`: the step made before the compare of the value it replaces, so both
/// lived and the step was copied back (+12.4% instructions behind the header
/// copy). The step is made in place: `mov byte ptr [bp+di-1024], 1; add di, si;
/// cmp di, bx; jb` (the stores' address is the stepped register itself).
#[test]
fn test_a_step_made_before_the_compare_of_its_value_is_made_after_it_in_place() {
    let source = format!("{}/bench/sieve/sieve.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    // The step is made in place whichever side of the compare it is on (the
    // pin's order has `add di, si` ahead of `cmp di, dx` now): the three
    // instructions before the back edge are the loop's stores, steps and the
    // compare, with no register `mov` or `lea` that makes the step in another
    // register and copies it back.
    let in_place = lines.windows(4).any(|four| {
        four[3].starts_with("jb ")
            && four[..3].iter().filter(|line| line.starts_with("cmp ")).count() == 1
            && four[..3].iter().any(|line| line.starts_with("add "))
            && four[..3]
                .iter()
                .all(|line| line.starts_with("cmp ") || line.starts_with("add ") || line.starts_with("mov byte ptr ["))
    });
    assert!(in_place, "{text}");
}
