//! x86-64: a function that binds locally takes part in the callee-written-registers facts (DEVIATE
//! IPRA-DSO-LOCAL, gcc `-fipa-ra`), though it has external linkage. A caller keeps its values across a call of
//! such a leaf in scratch registers, which cost no push; across a call of one that calls an unseen routine it
//! still needs the saved registers. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SAVED: [&str; 6] = ["push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15"];

fn saves_a_register(text: &str) -> bool {
    SAVED.iter().any(|push| text.contains(push))
}

fn assembly(source: &str) -> String {
    assembly_with(source, &[])
}

fn assembly_with(source: &str, flags: &[&str]) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ipra-local-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w"])
        .args(flags)
        .args(["-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// Five values kept across five calls of an external leaf were pushed to callee-saved registers (96 B against
/// gcc's 70 B, over_aligned_local_after_even_pushes.c).
#[test]
fn a_caller_keeps_values_in_scratch_registers_across_a_local_leaf() {
    let text = assembly("ipra_local_leaf.c");
    assert!(!saves_a_register(&text), "the caller saved registers the leaf never writes:\n{text}");
}

/// What the callee writes is bounded by what it calls: values kept across `wraps` stay in saved registers.
#[test]
fn a_caller_still_saves_registers_across_a_local_function_that_calls_an_unseen_one() {
    let text = assembly("ipra_local_caller_of_unknown.c");
    assert!(saves_a_register(&text), "the caller kept values in registers `unseen` may overwrite:\n{text}");
}

/// Shared-library code (`-fPIC`) lets another module interpose a default-visibility definition, so no caller may
/// rely on what its body writes.
#[test]
fn a_caller_still_saves_registers_across_a_default_visibility_leaf_in_shared_code() {
    let text = assembly_with("ipra_local_leaf.c", &["-fPIC"]);
    assert!(saves_a_register(&text), "the caller relied on an interposable leaf:\n{text}");
}

/// A callee that only reads `edi` and `esi` leaves them alone: the caller kept `a` and `b` in them across both
/// calls. The arrival of the arguments was counted as a write, so every call of `f` and `g` was preceded by
/// `mov edi, ..; mov esi, ..` (4 moves for 2 calls; select_of_constants `main` +1963 B).
#[test]
fn a_caller_keeps_its_arguments_in_the_argument_registers_across_a_callee_that_only_reads_them() {
    let text = assembly("ipra_arg_copy.c");
    let chain = text.split("chain proc near").nth(1).and_then(|rest| rest.split("chain endp").next()).expect("a chain body");
    let moves: Vec<&str> =
        chain.lines().map(str::trim).filter(|line| line.starts_with("mov ") && (line.contains(" edi,") || line.contains(" esi,"))).collect();
    assert!(moves.is_empty(), "argument copies before the calls: {moves:?}\n{chain}");
}
