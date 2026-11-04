//! A spilled constant is stored from its immediate, not through a register.

use std::process::Command;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// bench/nbody_fixed at -O2 spilled each of its eleven 64-bit constants as
/// `mov rax, -4096` + `mov qword ptr [rsp+120], rax` (two instructions) where
/// the pin stores `mov qword ptr [rsp-80], -4096`: the fold probe addressed
/// the spill cell through the 16-bit frame register, which long mode refuses.
#[test]
fn a_spilled_constant_is_one_store_of_the_immediate() {
    let text = listing(include_str!("../bench/nbody_fixed/nbody_fixed.c"), "-O2");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<_> = lines
        .windows(2)
        .filter(|pair| {
            pair[0].strip_prefix("mov rax, ").is_some_and(|imm| imm.parse::<i32>().is_ok())
                && pair[1].starts_with("mov qword ptr [rsp")
                && pair[1].ends_with(", rax")
        })
        .collect();
    assert!(pairs.is_empty(), "{pairs:?}");
}

/// bench/nbody_fixed at -O2 reloaded each spilled loop-carried sum into one
/// register and copied it to another (`mov rax, [rsp+96]` + `mov rdx, rax`)
/// where the pin reloads straight into the register it uses: the fold probe
/// had no 8-byte register to stand for the copy's other side, so a fold with
/// a second 64-bit register operand was always refused.
#[test]
fn a_reload_goes_straight_into_the_register_the_copy_names() {
    let text = listing(include_str!("../bench/nbody_fixed/nbody_fixed.c"), "-O2");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<_> = lines
        .windows(2)
        .filter(|pair| {
            let Some((into, _)) = pair[0].strip_prefix("mov ").and_then(|rest| rest.split_once(", qword ptr [rsp"))
            else {
                return false;
            };
            pair[1].strip_prefix("mov ").is_some_and(|rest| rest.ends_with(&format!(", {into}")))
        })
        .collect();
    assert!(pairs.is_empty(), "{pairs:?}");
}

/// A word value defined by a dword zero was folded to `mov dword ptr [rsp], 0`
/// and reloaded as a qword: the upper half was stack garbage, so the counter
/// of a 3x8x64 byte-plane init loop ran wild and the program segfaulted (jpeg_single at -O2).
#[test]
fn a_dword_constant_defining_a_qword_value_is_stored_as_a_qword() {
    let text = listing(
        "unsigned char planes[3][8][64];\n\
         void init(void) {\n\
             for (int c = 0; c < 3; c++) for (int y = 0; y < 8; y++) for (int x = 0; x < 64; x++) planes[c][y][x] = c + y + x;\n\
         }\n",
        "-O2",
    );
    let cell = |line: &str, width: &str| {
        let rest = line.trim().split(&format!("{width} ptr [")).nth(1)?;
        Some(rest.split(']').next()?.to_string())
    };
    let dwords: Vec<String> = text.lines().filter(|line| line.trim().starts_with("mov dword ptr [rsp")).filter_map(|line| cell(line, "dword")).collect();
    let torn: Vec<_> = dwords.iter().filter(|slot| text.lines().any(|line| cell(line, "qword").as_ref() == Some(*slot))).collect();
    assert!(torn.is_empty(), "dword store to a slot read as a qword: {torn:?}\n{text}");
}

/// The same loop spilled a symbol's address as `mov qword ptr [rsp+8], offset
/// planes+384`; the object writer encoded it with displacement 0 and a PC32
/// fixup, overwriting the loop counter's slot, so the program crashed.
#[test]
fn a_spilled_symbol_address_is_not_stored_as_an_immediate_at_m64() {
    let text = listing(
        "unsigned char planes[3][8][64];\n\
         void init(void) {\n\
             for (int c = 0; c < 3; c++) for (int y = 0; y < 8; y++) for (int x = 0; x < 64; x++) planes[c][y][x] = c + y + x;\n\
         }\n",
        "-O2",
    );
    let stores: Vec<_> = text.lines().map(str::trim).filter(|line| line.starts_with("mov qword ptr [") && line.contains(", offset ")).collect();
    assert!(stores.is_empty(), "{stores:?}");
}
