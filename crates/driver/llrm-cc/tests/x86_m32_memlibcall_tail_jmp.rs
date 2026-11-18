//! A `memmove` or `memcpy` that ends a cdecl function and takes the function's
//! own arguments is a `jmp`. The sibling-call check got no arguments, so no
//! stack slot ever matched: `copy_mem` pushed three words, called, popped them
//! and returned (4 instructions more than the pin's `jmp memmove`).

use std::path::Path;
use std::process::Command;

fn listing(name: &str, source: &str) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("memlibcall-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), source).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-O2", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("c.s")).unwrap()
}

#[test]
fn test_m32_a_memmove_that_ends_the_function_is_a_jmp() {
    let text = listing("void", "extern void *memmove(void *, const void *, unsigned);\nvoid f(void *d, const void *s, unsigned n) { memmove(d, s, n); }\n");
    assert!(text.contains("jmp memmove") && !text.contains("call memmove"), "{text}");
}

#[test]
fn test_m32_a_memmove_returning_its_destination_is_a_jmp() {
    let text = listing("returned", "extern void *memmove(void *, const void *, unsigned);\nvoid *f(void *d, const void *s, unsigned n) { return memmove(d, s, n); }\n");
    assert!(text.contains("jmp memmove") && !text.contains("call memmove"), "{text}");
}

/// The arguments are not the caller's own in the same slots: the call stays.
#[test]
fn test_m32_a_memmove_with_other_arguments_stays_a_call() {
    let text = listing("swapped", "extern void *memmove(void *, const void *, unsigned);\nvoid f(void *d, const void *s, unsigned n) { memmove((char *)d + 1, s, n); }\n");
    assert!(text.contains("call memmove"), "{text}");
}
