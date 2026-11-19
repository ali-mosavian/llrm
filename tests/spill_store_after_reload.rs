//! A value reloaded from a spill cell is not stored to a cell again.

use std::process::Command;

/// stft_single's butterfly loop at `-O2 -march=i486` reloaded a base index
/// from `[bp-2]` into si, read it, then stored si to a second cell and
/// reloaded it from there: one store a trip for a value the loop never
/// changes. Spills of siblings share their original's slot (the pin's
/// `InlineSpiller::spill`) and the store after the load is dropped.
#[test]
fn test_a_reloaded_register_is_not_stored_back_to_the_cell_it_came_from() {
    let source = format!("{}/bench/stft_single/stft_single.nib", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args(["-O2", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let all: Vec<&str> = text.lines().map(str::trim).collect();
    // The butterfly loop: its first array read, with the lines around it.
    let first = all.iter().position(|line| line.starts_with("fld dword ptr $var_im[bx+si]")).expect("butterfly loop");
    let lines = &all[first.saturating_sub(6)..all.len().min(first + 60)];
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("mov ") else { continue };
        let Some((register, cell)) = rest.split_once(", word ptr [bp-") else { continue };
        let store = format!(", {register}");
        let rewritten = |line: &str| {
            let first = line.split_once(' ').map_or("", |(_, operands)| operands.split(',').next().unwrap_or(""));
            first.trim() == register
        };
        let window = &lines[at + 1..lines.len().min(at + 6)];
        let stored = window.iter().position(|next| next.starts_with("mov word ptr [bp-") && next.ends_with(&store));
        if stored.is_some_and(|end| !window[..end].iter().any(|line| rewritten(line))) {
            found.push(format!("{register} <- [bp-{cell} stored again within 5 lines of line {at}"));
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
