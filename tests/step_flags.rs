//! A step of a compared value sets the compare's flags.

use std::process::Command;

fn listing(kernel: &str) -> String {
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// x_insertion's `n - 1 > 0` guard was `lea eax, [ecx-1]; cmp ecx, 1; jle`: the
/// compare read `n` after the step had defined `n - 1`, so the step could not
/// be made in place and the flags were made twice.
#[test]
fn test_a_guard_on_a_value_and_its_step_is_one_instruction_and_its_flags() {
    let text = listing("x_insertion");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(2)
            .any(|pair| pair[0].starts_with("add ") && pair[0].ends_with(", -1") && pair[1].starts_with("jle ")),
        "{text}"
    );
    assert!(!lines.iter().any(|line| line.starts_with("lea ") && line.ends_with("-1]")), "{text}");
}

/// sieve's `multiple += i` loop was `lea ax, [edi+esi]; cmp di, dx; mov di, ax;
/// jb`: the step made before the compare of the value it replaces, so both
/// lived and the step was copied back (+12.4% instructions behind the header
/// copy). The lea sets no flags: after the compare it is in place, `cmp di, dx;
/// lea di, [edi+esi]; jb`.
#[test]
fn test_a_step_made_before_the_compare_of_its_value_is_made_after_it_in_place() {
    let source = format!("{}/bench/sieve/sieve.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-ftree-ch", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let in_place = lines
        .windows(3)
        .any(
            |three| three[0].starts_with("cmp ") && three[1].starts_with("lea ") && {
                let (into, rest) = three[1]["lea ".len()..].split_once(", ").unwrap_or_default();
                rest.contains(&format!("e{into}")) && three[2].starts_with("jb ")
            },
        );
    assert!(in_place, "{text}");
}
