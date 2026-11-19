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
        common::cc::llrm_c().current_dir(scratch.path()).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap().lines().map(|one| one.trim().to_owned()).collect()
}

const M32: [&str; 4] = ["-m32", "-mabi=sysv", "-march=i486", "-O2"];
/// `char g(char *p) { return *p; }` is the pin's `movzbl (%eax), %eax` where
/// the extension is no slower than the load (`widens_load`, unit-tested); the
/// 486 and the Pentium take three clocks for it and one for `mov al, [eax]`.
#[test]
fn test_a_byte_load_into_a_dead_register_stays_where_the_extension_is_slower() {
    let text = asm("char g8(char *p) { return *p; }\nshort g16(short *p) { return *p; }\n", &M32);
    assert!(text.contains(&"mov al, byte ptr [eax]".to_owned()), "{text:?}");
    assert!(text.contains(&"mov ax, word ptr [eax]".to_owned()), "{text:?}");
}

/// -Os keeps the byte load: the zero extending form takes a byte more. The word
/// load is the same length either way and still widens.
#[test]
fn test_a_byte_load_stays_when_the_code_is_for_size() {
    let text = asm("char g8(char *p) { return *p; }\n", &["-m32", "-mabi=sysv", "-march=i486", "-Os"]);
    assert!(text.contains(&"mov al, byte ptr [eax]".to_owned()), "{text:?}");
}

/// A word load read wider by a loop is the pin's `movzx` (`implicit-def $eax`)
/// where the extension is no slower; on the 486 it stays `mov ax, [m]`.
#[test]
fn test_a_word_load_read_wider_by_a_loop_stays_where_the_extension_is_slower() {
    let text = asm(
        "unsigned short f(unsigned short *p, int n) {\n unsigned short x = *p;\n int i;\n for (i = 0; i < n; i++) x = x * 25173 + 13849;\n return x;\n}\n",
        &M32,
    );
    assert!(text.contains(&"mov ax, word ptr [eax]".to_owned()), "{text:?}");
}

/// x_strlen -O2 m32: the copy loop's `mov dl, [eax+buf]` became `movzx edx,
/// ...`, three clocks for one on the 486 (+348 k clk over gcc). The 486 and the
/// Pentium keep the partial load.
#[test]
fn test_a_byte_load_stays_on_a_cpu_where_the_extension_is_slower() {
    let source =
        "char buf[64]; int f(char *d) { char c; int i = 0; do { c = buf[i]; d[i++] = c; } while (c); return i; }\n";
    for march in ["i486", "pentium"] {
        let text = asm(source, &["-m32", "-mabi=sysv", &format!("-march={march}"), "-O2"]);
        assert!(!text.iter().any(|one| one.starts_with("movzx e") && one.contains("byte ptr")), "{march}: {text:?}");
        assert!(text.iter().any(|one| one.starts_with("mov ") && one.contains("byte ptr _buf")), "{march}: {text:?}");
    }
}
