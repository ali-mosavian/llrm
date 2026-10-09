//! An operation on an integer wider than the target's native one is its halves,
//! each a form of the target; a constant is an immediate in each, as the form
//! takes one.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `a + 0x100000005` made both halves in registers first (`mov ecx, 5; mov ebx,
/// 1; add eax, ecx; adc edx, ebx` between a push and a pop of each): the halves
/// are immediates now.
#[test]
fn a_constant_added_to_an_i64_is_an_immediate_in_each_half() {
    for flags in [&["-m32", "-O2"][..], &["-m16", "-O2"][..]] {
        let text = listing("long long f(long long a) { return a + 0x100000005LL; }\n", flags);
        assert!(text.contains("add eax, 5") && text.contains("adc edx, 1"), "{flags:?}: {text}");
        assert!(!text.contains("push ebx") && !text.contains("push ecx"), "{flags:?}: {text}");
    }
}

/// A subtraction of a variable keeps its borrow chain (`sub` then `sbb`), and a
/// constant on the left stays in a register: `sub` takes no immediate first
/// operand.
#[test]
fn a_subtraction_borrows_through_the_halves() {
    let text = listing(
        "long long f(long long a, long long b) { return a - b; }\nlong long g(long long a) { return 7 - a; }\n",
        &["-m32", "-O2"],
    );
    assert!(text.contains("sub ") && text.contains("sbb "), "{text}");
}

/// `a << (n & 31)` on an i64 made the branchless fix-up for counts from 32
/// (about 22 instructions: `shl; sar 31; xor -1; and; or` around the shifts)
/// though bit five of the count is known zero. A count not masked keeps it.
#[test]
fn a_masked_i64_shift_count_needs_no_fixup_for_counts_from_32() {
    let masked = listing(
        "unsigned long long f(unsigned long long a, unsigned long long n) { return a << (n & 31); }\n",
        &["-m32", "-O2"],
    );
    assert!(masked.contains("shld") && !masked.contains("sar ") && !masked.contains("xor "), "{masked}");
    let free = listing(
        "unsigned long long f(unsigned long long a, unsigned long long n) { return a << n; }\n",
        &["-m32", "-O2"],
    );
    assert!(free.contains("sar "), "{free}");
}
