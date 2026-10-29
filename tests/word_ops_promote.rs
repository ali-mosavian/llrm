//! m32 runs a word `and`, `or` or `xor` on the whole register (its datalayout's
//! `promote`, as LLVM's x86 `isTypeDesirableForOp`): no operand-size prefix,
//! and the `and` of a word leaves the upper half zero, so no extension follows
//! it.

mod common;

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
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
    // On the whole register, whichever one the allocator gave it.
    for (op, immediate) in [("xor", "4660"), ("or", "4660"), ("and", "29236")] {
        let whole = text
            .lines()
            .map(str::trim)
            .any(
                |line| line.strip_prefix(op)
                    .and_then(|rest| rest.strip_prefix(" e"))
                    .is_some_and(|rest| rest.ends_with(&format!(", {immediate}"))),
            );
        assert!(whole, "{op} e?x, {immediate} is missing:\n{text}");
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
    // The pin loads the later argument and xors the earlier from memory
    // (`movzwl 8(%esp), %eax; xorw 4(%esp), %ax`).
    assert!(text.contains("xor ax, word ptr [esp+4]") || text.contains("xor ax, word ptr [esp+8]"), "{text}");
}

/// A word `add` of registers ran under the operand-size prefix (`add ax, 3;
/// add dx, ax`: two prefixed instructions a trip in a loop of 16-bit counters,
/// where gcc and clang run both on the whole register).
///
/// llrm-cc: the step `a + 3` runs on the whole register of whichever number
/// the allocator gave it, not `ecx` (pin-shape: the pin's `llc` gives it
/// `esi`).
#[test]
fn a_word_add_of_registers_runs_on_the_whole_register() {
    let text = listing(
        "short t[256];\n\
         short f(short a, short b, int n) {\n\
             short s = 0; int i;\n\
             for (i = 0; i < n; i++) { a = a + 3; b = b + a; s = s + t[(a + b) & 255]; }\n\
             return s;\n\
         }\n",
    );
    // The loop is what the first `jne` closes, from the label before it (the
    // label's number is the allocator's to choose).
    let lines: Vec<&str> = text.lines().collect();
    let back_edge = lines.iter().position(|line| line.trim_start().starts_with("jne ")).expect("a back edge");
    let top = lines[..back_edge].iter().rposition(|line| line.starts_with('L')).expect("the loop's label");
    let loop_body = lines[top..back_edge].join("\n");
    let words = loop_body.lines().filter(|line| line.trim_start().starts_with("add ") && !line.contains('e')).count();
    assert_eq!(words, 0, "a word add is left in the loop:\n{text}");
    let whole_step = loop_body.lines().map(str::trim).any(|line| line.starts_with("add e") && line.ends_with(", 3"));
    assert!(whole_step, "{text}");
}

/// `t(3 * n + 1)`: the word chain `mov; shl; add; add 1` is one `lea`, and the
/// promoted `add 1` (now a dword) ended it, leaving `lea bx, [..]; inc ebx`
/// (bintree: one more instruction a node).
#[test]
fn a_promoted_step_ends_a_word_address_chain_in_one_lea() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.bas"),
        "DEFINT A-Z\nDECLARE FUNCTION At% (t() AS INTEGER, n AS INTEGER)\nDIM t(0 TO 99) AS INTEGER\n\
         PRINT At%(t(), 3)\nFUNCTION At% (t() AS INTEGER, n AS INTEGER)\n    At% = t(3 * n + 1)\nEND FUNCTION\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(directory)
        .args(["a.bas", "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-m32", "-march=i486", "-S", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.contains("+1]"), "the chain is not one lea:\n{text}");
    assert!(!text.contains("inc e"), "an inc follows the lea:\n{text}");
}

/// A word `inc` whose flags end the loop stays a word: promoted, it needed
/// `test cx, cx` for the flags of the word (bench/histo: `inc ecx; test cx,
/// cx; jne` for `inc cx; jne`, 4096 instructions more).
#[test]
fn a_word_counter_that_ends_a_loop_keeps_its_flags() {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/histo/histo.bas");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args([source, "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-m32", "-march=i486", "-S", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("test cx, cx"), "the counter was promoted and tested again:\n{text}");
}

/// A word product by a constant was a chain of word `lea`s and adds, each under
/// the operand-size prefix (`lea bx, [..]`; `add si, si`: four of them for
/// `a * 11` and `b * 7`), where the dword chain is two `lea`s without one.
#[test]
fn a_word_product_by_a_constant_is_a_chain_on_the_whole_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.bas"),
        "DEFINT A-Z\nDECLARE FUNCTION F% (a AS INTEGER, b AS INTEGER)\nPRINT F%(3, 4)\n\
         FUNCTION F% (a AS INTEGER, b AS INTEGER)\n    F% = ((a * 11) + (b * 7)) AND 511\nEND FUNCTION\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["a.bas", "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-m32", "-march=i486", "-S", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let words = text.lines().filter(|line| line.trim_start().starts_with("lea ") && !line.contains("lea e")).count();
    assert_eq!(words, 0, "a word lea is left:\n{text}");
}
