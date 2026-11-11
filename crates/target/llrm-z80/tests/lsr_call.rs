//! LSR in a loop whose call takes every register.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// The lines of the first loop, from its header label to the jump back to it.
fn first_loop(asm: &str) -> Vec<&str> {
    let lines: Vec<&str> = asm.lines().collect();
    for (end, line) in lines.iter().enumerate() {
        let Some(jump) = line.trim().strip_prefix("jr ").or_else(|| line.trim().strip_prefix("jp ")) else { continue };
        let target = jump.rsplit(',').next().unwrap_or(jump).trim();
        if let Some(start) = lines[..end].iter().position(|one| one.trim() == format!("{target}:")) {
            return lines[start..=end].to_vec();
        }
    }
    panic!("no loop in {asm}");
}

/// The frame bytes the loop stores: `ld -N(ix), r`.
fn stored_slots<'a>(lines: &[&'a str]) -> std::collections::BTreeSet<&'a str> {
    lines
        .iter()
        .filter_map(|line| line.trim().strip_prefix("ld ").and_then(|rest| rest.split_once(", ")))
        .filter(|(to, _)| to.ends_with("(ix)"))
        .map(|(to, _)| to)
        .collect()
}

/// huffman_decode -Os kept three pointer recurrences and the length counter
/// across `getbit`, which clobbers every register: eight frame bytes stored a
/// trip (four words), and 16 frame accesses. One index recurrence and the
/// counter are two words.
#[test]
fn a_loop_with_a_call_keeps_one_recurrence_in_the_frame() {
    let asm = text(include_str!("kernels/ir/decode.ll"), "decode", 0, &Request::new(Level::Os)).expect("compiles");
    let slots = stored_slots(&first_loop(&asm));
    assert!(slots.len() <= 4, "{} frame bytes stored a trip, {slots:?}:\n{asm}", slots.len());
}
