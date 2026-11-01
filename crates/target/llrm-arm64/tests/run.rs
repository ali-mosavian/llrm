//! The compiler against the pinned clang, end to end: each program in
//! `tests/programs` is built by this compiler (from clang's un-optimized IR)
//! and by the pinned `clang -O2`, linked with a freestanding runtime by the
//! pinned `ld.lld`, run under `qemu-aarch64`, and must print and exit the same.

use std::path::{Path, PathBuf};
use std::process::Command;

fn build() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64")))
}

struct Tools {
    clang: PathBuf,
    lld: PathBuf,
}

/// The pinned tools, or None (the test then says it skipped). The version they
/// print is the premise.
fn tools() -> Option<Tools> {
    let build = build();
    let (clang, lld) = (build.join("bin/clang"), build.join("bin/ld.lld"));
    if !clang.exists() || !lld.exists() || Command::new("qemu-aarch64").arg("--version").output().is_err() {
        return None;
    }
    let version = Command::new(&clang).arg("--version").output().ok()?;
    assert!(String::from_utf8_lossy(&version.stdout).contains("d1106deb71cc"), "not the pinned clang");
    Some(Tools { clang, lld })
}

fn run(command: &mut Command) -> String {
    let said = command.output().expect("the tool starts");
    assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8_lossy(&said.stdout).into_owned()
}

struct Outcome {
    stdout: Vec<u8>,
    status: Option<i32>,
}

fn executed(program: &Path) -> Outcome {
    let said = Command::new("qemu-aarch64").arg(program).output().expect("qemu starts");
    Outcome { stdout: said.stdout, status: said.status.code() }
}

fn compare(name: &str) {
    let Some(tools) = tools() else {
        eprintln!("skipped: needs the pinned LLVM with AArch64, and qemu-aarch64");
        return;
    };
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = here.join(format!("tests/programs/{name}.c"));
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (ir, ours, reference, runtime) = (dir.join("p.ll"), dir.join("ours.o"), dir.join("ref.o"), dir.join("rt.o"));
    let triple = "aarch64-unknown-linux-gnu";
    run(Command::new(&tools.clang)
        .args(["-target", triple, "-O2", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-lifetime-markers"])
        .args(["-S", "-emit-llvm", "-o"])
        .arg(&ir)
        .arg(&source));
    let llc = Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    // Unverified: MIR's verifier refuses the intrinsics clang emits that its
    // table lacks (`llvm.ctlz`, `cttz`, `bswap`, `va_end`, `stacksave`).
    // `ARM64_ISEL=dag` runs the same programs through the DAG selector.
    let isel = format!("--isel={}", std::env::var("ARM64_ISEL").unwrap_or_else(|_| "hand".into()));
    run(Command::new(llc).env("LLRM_VERIFY", "0").arg(isel).arg("-o").arg(&ours).arg(&ir));
    run(Command::new(&tools.clang).args(["-target", triple, "-O2", "-c", "-o"]).arg(&reference).arg(&source));
    run(Command::new(&tools.clang)
        .args(["-target", triple, "-O2", "-ffreestanding", "-fno-builtin", "-c", "-o"])
        .arg(&runtime)
        .arg(here.join("tests/rt/rt.c")));
    let link = |object: &Path, output: &Path| {
        run(Command::new(&tools.lld).args(["-static", "-o"]).arg(output).arg(object).arg(&runtime));
    };
    let (mine, theirs) = (dir.join("ours"), dir.join("ref"));
    link(&ours, &mine);
    link(&reference, &theirs);
    let expected = executed(&theirs);
    // Premise: the reference ran to an exit, so what it printed is the oracle.
    assert!(expected.status.is_some(), "the reference did not exit");
    let got = executed(&mine);
    assert_eq!(
        (String::from_utf8_lossy(&got.stdout), got.status),
        (String::from_utf8_lossy(&expected.stdout), expected.status),
        "{name}"
    );
}

#[test]
fn ret42() {
    compare("ret42");
}

#[test]
fn a_loop_and_a_call() {
    compare("loop");
}

#[test]
fn recursion() {
    compare("fib");
}

#[test]
fn globals_and_strings() {
    compare("globals");
}

#[test]
fn integer_arithmetic_of_every_width() {
    compare("arith");
}

#[test]
fn pointers_arrays_and_strings() {
    compare("pointers");
}

#[test]
fn structs_by_value_and_by_pointer() {
    compare("structs");
}

#[test]
fn switches() {
    compare("switches");
}

#[test]
fn floating_point() {
    compare("floats");
}

#[test]
fn many_arguments_and_function_pointers() {
    compare("manyargs");
}

#[test]
fn small_algorithms() {
    compare("algos");
}

#[test]
fn variadic_functions() {
    compare("varargs");
}

#[test]
fn aggregates_in_calls_and_results() {
    compare("aggregates");
}

#[test]
fn large_frames() {
    compare("bigframe");
}

#[test]
fn bit_manipulation_and_division_by_constants() {
    compare("bits");
}

#[test]
fn linked_lists_and_trees() {
    compare("list");
}

#[test]
fn sorting_with_callbacks() {
    compare("sorting");
}

#[test]
fn numeric_kernels() {
    compare("numeric");
}

#[test]
fn backtracking_and_128_bit_products() {
    compare("queens");
}

#[test]
fn a_switch_that_stays_a_switch() {
    compare("dispatch");
}

#[test]
fn variable_length_arrays() {
    compare("vla");
}

/// Every program as the Darwin target builds it is an object the pinned
/// `llvm-objdump` reads whole: each word of its code is an instruction, and
/// the relocations name symbols the object defines or imports.
#[test]
fn darwin_objects_disassemble_without_unknown_words() {
    let build = build();
    let objdump = build.join("bin/llvm-objdump");
    if !objdump.exists() {
        eprintln!("skipped: no pinned LLVM with AArch64");
        return;
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let llc = Path::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let scratch = tempfile::tempdir().unwrap();
    let mut checked = 0;
    for entry in std::fs::read_dir(here.join("tests/darwin")).unwrap() {
        let ir = entry.unwrap().path();
        if ir.extension().is_none_or(|one| one != "ll") {
            continue;
        }
        let object = scratch.path().join("p.o");
        run(Command::new(llc).env("LLRM_VERIFY", "0").arg("-o").arg(&object).arg(&ir));
        let listing = run(Command::new(&objdump).args(["-d", "-r"]).arg(&object));
        assert!(listing.contains("mach-o arm64"), "{}: {listing}", ir.display());
        assert!(!listing.contains("<unknown>") && !listing.contains(".long"), "{}: {listing}", ir.display());
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} programs");
}

#[test]
fn division_and_remainder_by_constants() {
    compare("division");
}

/// `x * -1` (all-ones constant) overflowed `value + 1` in the multiply
/// strength reduction and panicked the compiler.
#[test]
fn multiplication_by_all_ones_constants() {
    compare("mulconst");
}
