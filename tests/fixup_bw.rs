//! A byte or word load whose register's other bytes are dead is the zero
//! extending dword load, as `X86FixupBWInsts` makes it.

mod common;

fn asm(
    source: &str,
    flags: &[&str],
) -> Vec<String> {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(scratch.path()).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap().lines().map(|one| one.trim().to_owned()).collect()
}

const M32: [&str; 4] = ["-m32", "-mabi=sysv", "-march=i486", "-O2"];

/// `char g(char *p) { return *p; }` was `mov al, [eax]`, a partial write of
/// EAX; the pin's llc has `movzbl (%eax), %eax`.
#[test]
fn test_a_byte_load_into_a_dead_register_is_the_zero_extending_load() {
    let text = asm("char g8(char *p) { return *p; }\nshort g16(short *p) { return *p; }\n", &M32);
    assert!(text.contains(&"movzx eax, byte ptr [eax]".to_owned()), "{text:?}");
    assert!(text.contains(&"movzx eax, word ptr [eax]".to_owned()), "{text:?}");
    assert!(!text.iter().any(|one| one.starts_with("mov al,") || one.starts_with("mov ax,")), "{text:?}");
}

/// A copy of the low byte or word is the dword copy.
#[test]
fn test_a_byte_or_word_copy_is_the_dword_copy_when_the_rest_is_dead() {
    let text = asm("short c16(short a, short b) { return b; }\nchar c8(char a, char b) { return b; }\n", &M32);
    assert!(text.contains(&"movzx eax, word ptr [esp+8]".to_owned()), "{text:?}");
    assert!(text.contains(&"movzx eax, byte ptr [esp+8]".to_owned()), "{text:?}");
}

/// -Os keeps the byte load: the zero extending form takes a byte more. The word
/// load is the same length either way and still widens.
#[test]
fn test_a_byte_load_stays_when_the_code_is_for_size() {
    let text = asm("char g8(char *p) { return *p; }\n", &["-m32", "-mabi=sysv", "-march=i486", "-Os"]);
    assert!(text.contains(&"mov al, byte ptr [eax]".to_owned()), "{text:?}");
}

/// A word loaded into the register a loop then reads as a dword stayed `mov ax,
/// [m]`: the rest of EAX is live (the loop reads it) though nothing defined it.
/// The pin's `MOV16rm` carries `implicit-def $eax` there and becomes the
/// `movzx` (quicksort, +0.8% clk).
#[test]
fn test_a_word_load_read_wider_by_a_loop_is_the_zero_extending_load() {
    let text = asm(
        "unsigned short f(unsigned short *p, int n) {\n unsigned short x = *p;\n int i;\n for (i = 0; i < n; i++) x = x * 25173 + 13849;\n return x;\n}\n",
        &M32,
    );
    assert!(text.contains(&"movzx eax, word ptr [eax]".to_owned()), "{text:?}");
    assert!(!text.iter().any(|one| one.starts_with("mov ax,")), "{text:?}");
}
