//! What a C unit says of its symbols reaches the object file: an alias is a
//! symbol at its target's address, `visibility("hidden")` is `STV_HIDDEN`.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

/// `(name, st_value, st_other)` of every symbol of a little-endian ELF64 file.
fn symbols(bytes: &[u8]) -> Vec<(String, u64, u8)> {
    let word =
        |at: usize, width: usize| bytes[at..at + width].iter().rev().fold(0u64, |sum, &b| sum << 8 | u64::from(b));
    let (headers, size, number) = (word(0x28, 8) as usize, word(0x3A, 2) as usize, word(0x3C, 2) as usize);
    let header = |index: usize| headers + index * size;
    let symtab = (0..number).find(|&index| word(header(index) + 4, 4) == 2).expect("a symbol table");
    let strtab = word(header(symtab) + 0x28, 4) as usize;
    let (offset, length) = (word(header(symtab) + 0x18, 8) as usize, word(header(symtab) + 0x20, 8) as usize);
    let strings = word(header(strtab) + 0x18, 8) as usize;
    (0..length / 24)
        .map(|index| {
            let entry = offset + index * 24;
            let name = strings + word(entry, 4) as usize;
            let spelled = bytes[name..].iter().take_while(|b| **b != 0).map(|b| *b as char).collect();
            (spelled, word(entry + 8, 8), bytes[entry + 5])
        })
        .collect()
}

/// An alias was dropped from the object and a hidden function was written
/// with default visibility: another unit naming the alias failed to link,
/// and the image exported what the source hid.
#[test]
fn an_alias_is_a_symbol_at_its_targets_address_and_hidden_is_hidden() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("object_symbols_{}.o", std::process::id()));
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-c", "-m64", "-O2", "-fobject-format=elf"])
        .arg(root.join("tests/cc/object_symbols/alias.c"))
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let found = symbols(&std::fs::read(&out).unwrap());
    let _ = std::fs::remove_file(&out);
    let of =
        |name: &str| found.iter().find(|one| one.0 == name).unwrap_or_else(|| panic!("no symbol {name}: {found:?}"));
    assert_eq!(of("other").1, of("impl").1, "the alias is the target's address");
    assert_eq!((of("secret").2, of("impl").2), (2, 0));
}
