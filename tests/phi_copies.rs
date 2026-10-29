//! A loop's phi results are its arguments' registers where they do not
//! interfere.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let source = format!("{}/{source}", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
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
    // No `jmp` between a loop's label and the branch that closes it: its copies
    // are not in a block jumped over each trip.
    let label_of = |line: &str| line.trim().strip_suffix(':').map(str::to_owned);
    let mut inside_a_loop: Vec<&&str> = Vec::new();
    for (at, line) in function.iter().enumerate() {
        let Some(target) = line.trim().split_whitespace().nth(1).filter(|_| line.trim().starts_with('j')) else {
            continue;
        };
        if let Some(head) = function[..at].iter().rposition(|above| label_of(above).as_deref() == Some(target)) {
            inside_a_loop.extend(function[head..at].iter().filter(|one| one.trim().starts_with("jmp ")));
        }
    }
    assert!(inside_a_loop.is_empty(), "{inside_a_loop:?}\n{}", function.join("\n"));
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

/// crc's loop copied the masked bit into a register of its own and the result
/// back into the accumulator (`mov esi, eax` ... `mov eax, ebx`): +17% clocks
/// and a fourth saved register.
#[test]
fn test_a_crc_step_keeps_the_accumulator_in_one_register() {
    // The bit loop is unrolled by 8 at -O2 as the pin's is (a fourth saved
    // register, ESI, then): the rolled loop is the premise.
    // llrm-cc: the clang IR selects a constant (`select ... 0xEDB88320`): the
    // old selector branches on it and needs a fourth register. The pin and the
    // DAG selector are branchless, so this runs `--isel=dag` until it is the
    // default (#4512).
    // The pin keeps the same shape (clang -O2 -fno-unroll-loops: `movl %eax,
    // %esi; shrl %esi; andl $1, %eax; negl; andl; xorl`, ESI saved), so the
    // loops are held to the pin's length instead of to a register count.
    let flags = ["--isel=dag", "-fno-unroll-loops", "-m32", "-mabi=sysv", "-march=i486", "-O2"];
    let text = listing("bench/crc/crc.c", &["--isel=dag", "-fno-unroll-loops"]);
    let pin = common::pin::listing(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/crc/crc.c"), &flags);
    let (ours, pins) = (common::pin::loops(&text, Some("bench_crc")), common::pin::loops(&pin, Some("bench_crc")));
    assert!(!ours.is_empty() && ours.len() == pins.len(), "loops {ours:?} against the pin\'s {pins:?}:\n{text}");
    assert!(ours.iter().zip(&pins).all(|(one, other)| one <= other), "loops {ours:?} against the pin\'s {pins:?}:\n{text}");
}

/// x_rle's run length leaves at 255: the number the exit's phi takes was made
/// in the loop's test block (`mov eax, 255; cmp ecx, 255; jl`), every trip, for
/// an exit taken once. It is made on the edge that leaves (`mov eax, 255;
/// jmp`): x_rle -O1 +4.4% clocks behind the header copy.
#[test]
fn test_a_constant_a_loop_exit_takes_is_made_on_its_edge_not_every_trip() {
    let text = listing("crates/target/llrm-x86-m32/vsgcc/kernels/x_rle/x_rle.c", &["-O1", "-ftree-ch"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let in_the_loop = lines
        .windows(2)
        .any(|pair| pair[0].starts_with("mov e") && pair[0].ends_with(", 255") && !pair[1].starts_with("jmp "));
    assert!(!in_the_loop, "{text}");
}
