//! No flat-target assembly counts a `rep`, `loop` or `jecxz` with part of ECX.
//! A count built in CL, CH or CX leaves bits 16-31 to whatever the caller held:
//! `xor ch, ch` before `repe scasb` in gdscan.asm ran PAINT's scan over 0xB0000
//! bytes when a C function left the screen address in ECX (color_masks crashed
//! on x86-m32). A count is set whole: `movzx ecx, cl`, `mov ecx, ...`.

use std::fs;
use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// 32-bit assembly directories of the runtime: what runs with ECX as the count.
const DIRECTORIES: &[&str] =
    &["runtime/qb/dos/m32", "runtime/qb/dev/dos/m32", "runtime/shared/dos/m32", "runtime/c/x86-m32"];

/// The registers an operand names, lowercase, from `mov  cl, byte ptr [x]`'s
/// destination.
fn destination(code: &str) -> Option<(String, String)> {
    let mut parts = code.splitn(2, char::is_whitespace);
    let mnemonic = parts.next()?.to_ascii_lowercase();
    let first = parts.next()?.split(',').next()?.trim().to_ascii_lowercase();
    Some((mnemonic, first))
}

/// The `(line, text)` of each count user whose count register was last set in
/// part.
fn flagged(text: &str) -> Vec<(usize, String)> {
    let mut partial: Option<usize> = None;
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let code = line.split(';').next().unwrap_or("").trim();
        let lower = code.to_ascii_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        if words.contains(&"proc") || words.contains(&"endp") {
            partial = None;
            continue;
        }
        let Some((mnemonic, first)) = destination(code) else { continue };
        let counts =
            mnemonic.starts_with("rep") || mnemonic.starts_with("loop") || mnemonic == "jecxz" || mnemonic == "jcxz";
        if counts {
            if let Some(at) = partial {
                found.push((index + 1, format!("{code} (count set in part at line {at})")));
            }
            continue;
        }
        let writes =
            !matches!(
                mnemonic.as_str(),
                "cmp" | "test" | "push" | "call" | "jmp" | "ret" | "db" | "dw" | "dd"
            );
        if !writes {
            continue;
        }
        match first.as_str() {
            "ecx" => partial = None,
            "cl" | "ch" | "cx" => partial = Some(index + 1),
            _ => {}
        }
    }
    found
}

fn assembly() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in DIRECTORIES {
        for entry in fs::read_dir(Path::new(ROOT).join(directory)).expect("a runtime directory") {
            let path = entry.expect("an entry").path();
            if path.extension().is_some_and(|ext| ext == "asm") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn test_no_flat_assembly_counts_with_part_of_ecx() {
    let mut all = Vec::new();
    for path in assembly() {
        let text = fs::read_to_string(&path).expect("readable assembly");
        for (line, code) in flagged(&text) {
            all.push(format!("{}:{line}: {code}", path.strip_prefix(ROOT).unwrap().display()));
        }
    }
    assert!(all.is_empty(), "set a count with the whole of ecx (movzx ecx, cl):\n{}", all.join("\n"));
}

/// gdscan.asm's `xor ch, ch` + `repe scasb`, as it stood when color_masks
/// crashed.
#[test]
fn test_the_scan_that_counted_with_cl_alone_is_flagged() {
    let old = "scan_one proc near\n  mov cl, byte ptr [ebx]\n  xor ch, ch\n  cld\n  repe scasb\n  ret\nscan_one endp\n";
    assert_eq!(flagged(old).len(), 1, "{:?}", flagged(old));
    let fixed = old.replace("xor ch, ch", "movzx ecx, cl");
    assert!(flagged(&fixed).is_empty(), "{:?}", flagged(&fixed));
}
