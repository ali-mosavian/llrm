//! x86-64: with `LLRM_IPRA_BINDS_LOCAL` a function that binds locally takes
//! part in the callee-written-registers facts (gcc `-fipa-ra`), though it has
//! external linkage. Off by default, as the pin: QB core IPRA on minus off was
//! +112 B at Os and +542 B at O2 (row IPRA-DSO-LOCAL). A caller keeps its
//! values across a call of such a leaf in scratch registers, which cost no
//! push; across a call of one that calls an unseen routine it still needs the
//! saved registers. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

const SAVED: [&str; 6] = ["push rbx", "push rbp", "push r12", "push r13", "push r14", "push r15"];

fn saves_a_register(text: &str) -> bool {
    SAVED.iter().any(|push| text.contains(push))
}

fn assembly(source: &str) -> String {
    assembly_with(source, &[])
}

/// The assembly with the facts of locally binding functions on.
fn assembly_with(
    source: &str,
    flags: &[&str],
) -> String {
    assembly_env(source, flags, true)
}

fn assembly_env(
    source: &str,
    flags: &[&str],
    binds_local: bool,
) -> String {
    assembly_run(source, "-Os", flags, binds_local, &[])
}

fn assembly_run(
    source: &str,
    level: &str,
    flags: &[&str],
    binds_local: bool,
    env: &[(&str, &str)],
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ipra-local-{}-{source}-{level}-{}-{binds_local}-{}",
        std::process::id(),
        flags.join("_"),
        env.iter().map(|(name, _)| *name).collect::<Vec<_>>().join("_")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"));
    if binds_local {
        command.env("LLRM_IPRA_BINDS_LOCAL", "1");
    } else {
        command.env_remove("LLRM_IPRA_BINDS_LOCAL");
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let run = command
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w"])
        .args(flags)
        .args(["-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// Default at -O2 (the pin): a caller keeps values across an external leaf in
/// saved registers, as `RegUsageInfoCollector` takes private functions only.
/// IPRA was on for m64 at every level until 2026-11-22: QB core +112 B at Os,
/// +542 B at O2.
#[test]
fn by_default_at_o2_a_caller_saves_registers_across_an_external_leaf() {
    let text = assembly_run("ipra_local_leaf.c", "-O2", &[], false, &[]);
    assert!(saves_a_register(&text), "the caller relied on an external leaf's registers at -O2:\n{text}");
}

/// IPRA-OS-TRIAL: at -Os a module with a locally binding callee is compiled
/// again with the facts on and the smaller object kept. `ipra_local_leaf.c`
/// is 95 B without them and 72 B with; `vector_float_compare_loop.c` is 4 B
/// larger with them, so it stays as the pin has it.
#[test]
fn at_os_the_smaller_object_of_the_two_ipra_shapes_is_kept() {
    let off = [("LLRM_NO_IPRA_TRIAL", "1")];
    let leaf = assembly_run("ipra_local_leaf.c", "-Os", &[], false, &[]);
    assert!(!saves_a_register(&leaf), "the trial kept the larger object:\n{leaf}");
    let pinned = assembly_run("ipra_local_leaf.c", "-Os", &[], false, &off);
    assert!(saves_a_register(&pinned), "no trial is the pin's shape:\n{pinned}");
    let compare = assembly_run("vector_float_compare_loop.c", "-Os", &[], false, &[]);
    let compare_pinned = assembly_run("vector_float_compare_loop.c", "-Os", &[], false, &off);
    assert_eq!(compare, compare_pinned, "the facts grew this object: it is put back");
}

/// Five values kept across five calls of an external leaf were pushed to
/// callee-saved registers (96 B against gcc's 70 B,
/// over_aligned_local_after_even_pushes.c).
#[test]
fn a_caller_keeps_values_in_scratch_registers_across_a_local_leaf() {
    let text = assembly("ipra_local_leaf.c");
    assert!(!saves_a_register(&text), "the caller saved registers the leaf never writes:\n{text}");
}

/// What the callee writes is bounded by what it calls: values kept across
/// `wraps` stay in saved registers.
#[test]
fn a_caller_still_saves_registers_across_a_local_function_that_calls_an_unseen_one() {
    let text = assembly("ipra_local_caller_of_unknown.c");
    assert!(saves_a_register(&text), "the caller kept values in registers `unseen` may overwrite:\n{text}");
}

/// Shared-library code (`-fPIC`) lets another module interpose a
/// default-visibility definition, so no caller may rely on what its body
/// writes.
#[test]
fn a_caller_still_saves_registers_across_a_default_visibility_leaf_in_shared_code() {
    let text = assembly_with("ipra_local_leaf.c", &["-fPIC"]);
    assert!(saves_a_register(&text), "the caller relied on an interposable leaf:\n{text}");
}

/// A callee that only reads `edi` and `esi` leaves them alone: the caller kept
/// `a` and `b` in them across both calls. The arrival of the arguments was
/// counted as a write, so every call of `f` and `g` was preceded by `mov edi, .
/// .; mov esi, ..` (4 moves for 2 calls; select_of_constants `main` +1963 B).
#[test]
fn a_caller_keeps_its_arguments_in_the_argument_registers_across_a_callee_that_only_reads_them() {
    let text = assembly("ipra_arg_copy.c");
    let chain =
        text.split("chain proc near").nth(1).and_then(|rest| rest.split("chain endp").next()).expect("a chain body");
    let moves: Vec<&str> = chain
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("mov ") && (line.contains(" edi,") || line.contains(" esi,")))
        .collect();
    assert!(moves.is_empty(), "argument copies before the calls: {moves:?}\n{chain}");
}

/// A function that tail-calls another passes its arguments on in the registers
/// it was given: the `jmp` reads `edi` and `esi`, it does not write them.
/// Counting the `jmp`'s requirements as writes made every such function "write"
/// them, so each call of one was preceded by argument copies
/// (select_of_constants -Os, 136 of 413 functions).
#[test]
fn a_caller_keeps_its_arguments_across_a_callee_that_tail_calls_with_them() {
    let text = assembly("ipra_tail_call.c");
    let chain = text
        .split("chain_tail proc near")
        .nth(1)
        .and_then(|rest| rest.split("chain_tail endp").next())
        .expect("a chain_tail body");
    let moves: Vec<&str> = chain
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("mov ") && (line.contains(" edi,") || line.contains(" esi,")))
        .collect();
    assert!(moves.is_empty(), "argument copies before the calls: {moves:?}\n{chain}");
}

/// The zero a divide reads in `rdx` was one `xor esi, esi` before the loop and
/// `mov rdx, rsi` at each divide: the loop held a scratch register for it
/// across the calls of the leaf, so a value went to `rbx` and was pushed
/// (div_constants_ulong -O1, 4362 B; the pin remakes the zero at each divide:
/// 4334 B with this).
#[test]
fn a_loop_makes_a_divides_zero_at_the_divide_and_keeps_no_register_for_it() {
    let text = assembly_with("ipra_zero_divide.c", &["-O1", "-fno-inline-functions"]);
    let body = text.split("g proc near").nth(1).and_then(|rest| rest.split("g endp").next()).expect("a g body");
    assert!(!saves_a_register(body), "g saved a register around the calls:\n{body}");
    let zeros = body.lines().map(str::trim).filter(|line| *line == "xor edx, edx").count();
    assert_eq!(zeros, 2, "one zero at each divide:\n{body}");
    assert!(!body.contains("mov rdx, r"), "a held zero was copied into rdx:\n{body}");
}
