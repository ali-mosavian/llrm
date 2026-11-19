//! A static function is a local symbol of the ELF object, as the pin writes it.

use std::process::Command;

fn u16_at(
    bytes: &[u8],
    at: usize,
) -> usize {
    u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
}

fn u32_at(
    bytes: &[u8],
    at: usize,
) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn u64_at(
    bytes: &[u8],
    at: usize,
) -> usize {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize
}

/// The named symbols of an ELF64 object's symbol table: (name, is local).
fn symbols(elf: &[u8]) -> Vec<(String, bool)> {
    let (table, count, size) = (u64_at(elf, 0x28), u16_at(elf, 0x3C), u16_at(elf, 0x3A));
    let section = |index: usize| table + index * size;
    let symtab = (0..count).find(|&one| u32_at(elf, section(one) + 4) == 2).expect("a symbol table");
    let (at, length, link) =
        (u64_at(elf, section(symtab) + 0x18), u64_at(elf, section(symtab) + 0x20), u32_at(elf, section(symtab) + 0x28));
    let strings = u64_at(elf, section(link) + 0x18);
    (0..length / 24)
        .filter_map(|one| {
            let entry = at + one * 24;
            let start = strings + u32_at(elf, entry);
            let name = &elf[start..start + elf[start..].iter().position(|&byte| byte == 0).unwrap()];
            (!name.is_empty() && elf[entry + 4] & 0xF != 3 && elf[entry + 4] & 0xF != 4)
                .then(|| (String::from_utf8_lossy(name).into_owned(), elf[entry + 4] >> 4 == 0))
        })
        .collect()
}

/// `nm` of llrm-cc's object listed `main` alone where the pin's lists every
/// static function (34 for tests/run/c/mul_constants.c): a debugger and a
/// per-function size comparison saw none of them.
#[test]
fn test_a_static_function_is_a_local_symbol() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "static int a(int x) { return x * 3 + 1; }\n\
         static int b(int x) { return x * 5 + 2; }\n\
         int g(int);\n\
         int main(void) { return g(a(g(1))) + g(b(g(2))); }\n",
    )
    .unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-Oz", "-fno-inline-functions", "-c", "-o", "a.o", "a.c"])
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let found = symbols(&std::fs::read(scratch.path().join("a.o")).unwrap());
    let local: Vec<&str> = found.iter().filter(|one| one.1).map(|one| one.0.as_str()).collect();
    assert_eq!(local, ["a", "b"], "{found:?}");
    assert!(found.iter().any(|one| one.0 == "main" && !one.1), "{found:?}");
}
