//! `countdown`'s `acc * 3` is made once, and its `n - 1` stays a `dec`.

use std::process::Command;

fn countdown(flags: &[&str]) -> (String, Vec<String>) {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "long countdown(long n, long acc) { if (n == 0) return acc; return countdown(n - 1, acc * 3 + n); }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64"])
        .args(flags)
        .args(["-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines = text
        .lines()
        .skip_while(|line| !line.starts_with("countdown proc"))
        .skip(1)
        .take_while(|line| !line.starts_with("countdown endp"))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.ends_with(':'))
        .collect();
    (text, lines)
}

/// The exit's `lea [acc*3]` was made again after the loop (the pin's MachineCSE
/// shares it; ours never took a `lea` for a candidate), and once shared the
/// `dec` of `n` became an `lea` and the `add` a commuted add: a dead flags
/// write between them stopped `rescheduleMIBelowKill`, and at -O2 the loop
/// counter's `dec`, which leaves CF alone, made the block's end look live. 45
/// -> 48 B at -Os, 170 -> 173 at -O2.
#[test]
fn the_decrement_stays_a_dec() {
    for flags in [&["-Os"][..], &["-O2"][..]] {
        let (text, lines) = countdown(flags);
        let decrements = lines.iter().filter(|line| line.starts_with("lea ") && line.ends_with("-1]")).count();
        assert_eq!(decrements, 0, "{flags:?}\n{text}");
    }
}

/// The `lea` CSE of #7261, out for the m32 regressions (quicksort -O3,
/// `x_life`): re-land with the m32 bench rows.
#[test]
#[ignore = "lea CSE: re-land with an m32 clock check"]
fn the_triple_is_made_once() {
    let (text, lines) = countdown(&["-Os"]);
    let triples = lines.iter().filter(|line| line.starts_with("lea ") && line.ends_with("*2]")).count();
    assert_eq!(triples, 1, "{text}");
}
