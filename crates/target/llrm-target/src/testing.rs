//! Checks every target crate calls from its own tests, so a rule lives once.

use llrm_mir::datalayout::DataLayout;

/// The stack facts of a target's datalayout:
/// 1. it declares `S` (the passes that raise an alloca to a preferred alignment are bounded by it; unset, they are not);
/// 2. `S` is at most `kept`, the alignment its frames keep (`Target::stack_alignment`), or a pass raises past what the frame keeps;
/// 3. where the pin has a datalayout string for the triple (`pin`), ours equals it.
/// A target with no pin string states `S` in its description and it equals the kept value (6502, z80). Not asserted: S == kept in
/// general, they are two facts (m68k: S16, kept 4, pin stack 8).
pub fn assert_stack_facts(
    target: &str,
    datalayout: &str,
    kept: u64,
    pin: Option<&str>,
) {
    let stated = DataLayout::parse(datalayout).unwrap_or_else(|why| panic!("{target}: datalayout does not parse: {why}")).stack_align;
    let row = format!("{target}: S {stated:?}, kept {kept}, pin {pin:?}, datalayout {datalayout}");
    let Some(stated) = stated else { panic!("datalayout without `S`: {row}") };
    assert!(stated <= kept, "S above what the frames keep: {row}");
    match pin {
        Some(pin) => assert_eq!(datalayout, pin, "differs from the pin's string: {row}"),
        None => assert_eq!(stated, kept, "no pin string: S is stated and equals the kept value: {row}"),
    }
}

/// A target whose pin's datalayout string names no `S` (m16: the clang fork's) names none: the pin's alloca raise to a type's preferred
/// alignment is unbounded there and ours follows it. `reason` is the target's `stack_alignment_unstated`; a datalayout that declares an
/// `S` anyway is caught by name.
pub fn assert_unstated_stack(
    target: &str,
    datalayout: &str,
    reason: Option<&str>,
) {
    let reason = reason.unwrap_or_else(|| panic!("{target}: no stack_alignment_unstated reason in its datalayout.toml"));
    let stated = DataLayout::parse(datalayout).unwrap_or_else(|why| panic!("{target}: datalayout does not parse: {why}")).stack_align;
    assert!(stated.is_none(), "declares an S though its pin string has none ({reason}): {target}: S {stated:?}, datalayout {datalayout}");
}
