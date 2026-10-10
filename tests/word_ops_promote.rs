//! m32 runs a word `and`, `or` or `xor` on the whole register (its datalayout's
//! `promote`, as LLVM's x86 `isTypeDesirableForOp`): no operand-size prefix,
//! and the `and` of a word leaves the upper half zero, so no extension follows
//! it.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `xor ax, 4660` and `or ax, 4660` were selected as the word, then extended
/// (`movzx eax, ax`); only an `and` of a word is exempt from an extension after
/// it, and was the one that regressed quicksort once the mask went under the
/// extension, for `mov dx, ax; and dx, 32767; movzx edx, dx`.
#[test]
fn a_word_and_or_xor_of_a_register_runs_on_the_whole_register() {
    let text = listing(
        "unsigned fx(unsigned short x, unsigned y) { unsigned short t = x ^ 0x1234; return t + y + x; }\n\
         unsigned fo(unsigned short x, unsigned y) { unsigned short t = x | 0x1234; return t + y + x; }\n\
         unsigned fa(unsigned short x, unsigned y) { unsigned short t = x & 0x7234; return t + y + x; }\n",
    );
    for form in ["xor eax, 4660", "or eax, 4660", "and eax, 29236"] {
        assert!(text.contains(form), "{form} is missing:\n{text}");
    }
    for word in ["xor ax,", "or ax,", "and ax,"] {
        assert!(!text.contains(word), "{word} is a word operation:\n{text}");
    }
}

/// The word operation stays where it reads a cell in place: `xor ax, [esp+8]`
/// is one instruction where the dword one needs a load first.
#[test]
fn a_word_operation_with_a_memory_operand_stays_a_word() {
    let text = listing("unsigned short g(unsigned short x, unsigned short y) { return x ^ y; }\n");
    assert!(text.contains("xor ax, word ptr [esp+8]"), "{text}");
}
