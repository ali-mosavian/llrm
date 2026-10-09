//! A flat access to a constant address the platform says no program data
//! occupies (DOS/32A's zero-based low memory: the interrupt vectors, the video
//! buffer, the BIOS ROM) reaches no variable.

use std::process::Command;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// The text of `name`'s procedure.
fn procedure<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = text.find(&format!("{name}_ proc")).unwrap_or_else(|| panic!("no {name} in {text}"));
    let end = text[start..].find("endp").unwrap() + start;
    &text[start..end]
}

/// `return g` after a store to the video buffer reloaded `g`: the store was
/// taken to reach any variable, as no range of a flat target's memory was
/// foreign (`Machine::foreign_span` answered for real mode alone).
#[test]
fn a_store_to_the_video_buffer_does_not_reload_a_variable() {
    let text = listing("int g;\nint f(void) { g = 5; *(char *)0xB8000 = 1; return g; }\n");
    let f = procedure(&text, "f");
    assert!(f.contains("mov eax, 5") && !f.contains("mov eax, dword ptr _g"), "{f}");
}

/// A pointer that is not a constant may be anything, `g` included.
#[test]
fn a_store_through_a_pointer_still_reloads_a_variable() {
    let text = listing("int g;\nint h(char *p) { g = 5; *p = 1; return g; }\n");
    let h = procedure(&text, "h");
    assert!(h.contains("mov eax, dword ptr _g"), "{h}");
}

/// The extender's memory above 1 MB holds the program's data: not foreign.
#[test]
fn a_store_to_a_constant_address_in_extended_memory_still_reloads_a_variable() {
    let text = listing("int g;\nint f(void) { g = 5; *(char *)0x500000 = 1; return g; }\n");
    let f = procedure(&text, "f");
    assert!(f.contains("mov eax, dword ptr _g"), "{f}");
}
