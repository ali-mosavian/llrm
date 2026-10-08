//! A function that needs no frame pointer holds a value in it (LLVM's `hasFP`, decided before allocation), and addresses its
//! frame through the stack pointer at the depth every way into a block agrees on.

use std::process::Command;

/// A loop entered at its test, so its body's label is reached only by a branch that comes after it.
const KERNEL: &str = "
static int values[64];
static long chop(int lo, int hi)
{
    long sum = 0;
    int i, mid;
    if (hi - lo < 2) return 0;
    mid = (lo + hi) / 2;
    for (i = lo; i < hi; ++i) sum += values[i] & mid;
    return sum + chop(lo, mid) + chop(mid, hi);
}
long f(int n) { return chop(0, n); }
";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// A block reached only by a later branch was addressed at depth 0: `add dword ptr [esp-8], ebp` wrote below the stack
/// (recchop wrong at -O2).
#[test]
fn test_a_block_reached_by_a_later_branch_addresses_its_cells_at_the_depth_it_is_entered() {
    let text = listing(&["-m32", "-march=i486", "-O2"]);
    assert!(!text.contains("[esp-"), "a cell below the stack pointer: {text}");
    assert!(text.contains("ebp"), "the frame register holds no value: {text}");
    assert!(!text.contains("mov ebp, esp"), "{text}");
}

/// A spill reloaded by `pop` into a frame cell made the frame register's release fail the compile ("cannot be addressed through the
/// stack pointer", x_life at -O2): the cell is addressed with the stack already taken back.
#[test]
fn test_a_pop_into_a_frame_cell_is_addressed_after_the_stack_is_taken_back() {
    let kernel = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/target/llrm-x86-m32/vsgcc/kernels/x_life/x_life.c")).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), kernel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

/// An array on the stack indexed by a register is `[bp+index+disp]`, SS-based, with no base value: it kept the frame register's
/// name once that held a value, and read through whatever EBP held (nib/idioms at -O2 -m32, a checksum of 104000 for 9692800).
#[test]
fn test_a_stack_array_indexed_by_a_register_is_addressed_through_the_stack_pointer() {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run/nib/idioms.nib");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).current_dir(scratch.path()).args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", source]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let read_through_ebp = text.lines().any(|line| line.contains("[ebp+edx]") || line.contains("[ebp+ecx]") || line.contains("[ebp+esi]"));
    assert!(!read_through_ebp || text.contains("mov ebp,"), "a frame cell read through an EBP nothing set: {text}");
}

/// A function that only read a stack argument pushed and popped the frame register it never used (`push ebp` in bench/hanoi's
/// kernel, 1.4x the clocks of the framed one): a frame cell names the register as an address, not as a use.
#[test]
fn test_a_function_that_only_reads_a_stack_argument_does_not_save_the_frame_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int g(int);\nint f(int n) { return g(n) + 1; }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("ebp"), "{text}");
}
