//! A loop's phi results are its arguments' registers where they do not
//! interfere.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let source = format!("{}/{source}", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486"])
        .args(if flags.iter().any(|flag| flag.starts_with("-O")) { &[][..] } else { &["-O2"][..] })
        .args(flags)
        .args(["-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// frames' `digits` loop (entered behind the copy of its test) has a value read
/// after the loop (the sum) and a counter that its own step defines: the phi
/// copies on the back edge went in a block of their own, jumped over from
/// the loop's entry (`jmp` each time the loop ran; hanoi's nest had one per
/// level). gcc's out-of-SSA gives the phi's result the argument's register
/// (`tree-ssa-coalesce`): the step defines the variable and no copy is left.
#[test]
fn test_a_loop_variable_read_after_the_loop_is_not_copied_on_its_back_edge() {
    let text = listing("bench/frames/frames.c", &["-ftree-ch"]);
    let function: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("_bench_frames proc"))
        .take_while(|line| !line.starts_with("_bench_frames endp"))
        .collect();
    let jumps = function.iter().filter(|line| line.trim().starts_with("jmp ")).count();
    assert_eq!(jumps, 0, "{}", function.join("\n"));
}

/// x_life's `cur = 1 - cur` is a variable the loop defines from itself in the
/// non-first operand of a subtraction: given the variable's register outright,
/// the two-address `sub` overwrote what it read and the program ran with `cur =
/// 0` for good (`sub eax, eax` where `mov eax, 1; sub eax, [cur]` was; x_life
/// -O1 returned another count than the others).
#[test]
fn test_a_variable_a_subtraction_reads_as_its_second_operand_is_not_defined_by_it() {
    let text = listing("crates/target/llrm-x86-m32/vsgcc/kernels/x_life/x_life.c", &["-O1"]);
    let same = text.lines().map(str::trim).find(|line| {
        line.strip_prefix("sub ").and_then(|rest| rest.split_once(", ")).is_some_and(|(one, other)| one == other)
    });
    assert_eq!(same, None, "{text}");
}
