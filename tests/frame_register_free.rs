//! A function that needs no frame pointer holds a value in it (LLVM's `hasFP`, decided before allocation), and
//! addresses its frame through the stack pointer at the depth every way into a block agrees on.

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
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
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

/// A spill reloaded by `pop` into a frame cell made the frame register's release fail the compile ("cannot be addressed
/// through the stack pointer", x_life at -O2): the cell is addressed with the stack already taken back.
#[test]
fn test_a_pop_into_a_frame_cell_is_addressed_after_the_stack_is_taken_back() {
    let kernel = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/target/llrm-x86-m32/vsgcc/kernels/x_life/x_life.c"
    ))
    .unwrap();
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), kernel).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

/// An array on the stack indexed by a register is `[bp+index+disp]`, SS-based, with no base value: it kept the frame
/// register's name once that held a value, and read through whatever EBP held (nib/idioms at -O2 -m32, a checksum of
/// 104000 for 9692800).
#[test]
fn test_a_stack_array_indexed_by_a_register_is_addressed_through_the_stack_pointer() {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run/nib/idioms.nib");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let read_through_ebp =
        text.lines().any(|line| line.contains("[ebp+edx]") || line.contains("[ebp+ecx]") || line.contains("[ebp+esi]"));
    assert!(!read_through_ebp || text.contains("mov ebp,"), "a frame cell read through an EBP nothing set: {text}");
}

/// A function that only read a stack argument pushed and popped the frame register it never used (`push ebp` in
/// bench/hanoi's kernel, 1.4x the clocks of the framed one): a frame cell names the register as an address, not as a
/// use.
#[test]
fn test_a_function_that_only_reads_a_stack_argument_does_not_save_the_frame_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int g(int);\nint f(int n) { return g(n) + 1; }\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("ebp"), "{text}");
}

/// At -Os `masm::frame_omitted` took the framed body when it was shorter, though a value already lived in the freed
/// frame register: "the frame register was given to a value and the frame cannot be addressed through the stack
/// pointer" (c/fill_nest, c/stack_arrays_copy, nib/idioms, torture 990513-1 at -Os).
#[test]
fn test_size_keeps_the_stack_addressed_frame_where_a_value_holds_the_frame_register() {
    for program in ["fill_nest", "stack_arrays_copy"] {
        let source = format!("{}/tests/run/c/{program}.c", env!("CARGO_MANIFEST_DIR"));
        let scratch = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
            .current_dir(scratch.path())
            .args(["-m32", "-march=i486", "-Os", "-S", "-o", "a.s", &source])
            .output()
            .unwrap();
        assert!(output.status.success(), "{program}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

/// A spill swap `xchg edi, [cell]` was refused by `masm::stack_addressed`, which allowed an exchange of registers only:
/// with a value in the freed frame register the compile failed (vsgcc x_hanoi2 at -O2 and -Os, "the frame register was
/// given to a value ...").
#[test]
fn test_an_exchange_with_a_frame_cell_is_addressed_through_the_stack_pointer() {
    let source = "
static long moves; static int peg[3][16], top[3];
static void mv(int n, int from, int to, int via)
{
    if (n == 0) return;
    mv(n - 1, from, via, to);
    peg[to][top[to]++] = peg[from][--top[from]]; ++moves;
    mv(n - 1, via, to, from);
}
long f(int n) { int i; for (i = 0; i < n; ++i) peg[0][top[0]++] = n - i; mv(n, 0, 2, 1); return moves * 100 + top[2]; }
";
    for level in ["-O2", "-Os"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
            .current_dir(scratch.path())
            .args(["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{level}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
