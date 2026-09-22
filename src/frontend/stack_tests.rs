//! Port of `tests/test_stack.py`, plus this module's own `touches_sp` test.
//!
//! Python's `is` checks become equality: an `Insn` carries its own address.

use super::{frames, touches_sp};
use crate::frontend::blocks::{Block, Ends};
use crate::frontend::declen::{decode, run};
use crate::support::hash::IndexMap;

fn block_of(hexcode: &str) -> Block {
    let digits: String = hexcode.chars().filter(|c| !c.is_whitespace()).collect();
    let code: Vec<u8> = (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap()).collect();
    let (insns, stuck) = run(&code, 0, code.len());
    assert!(stuck.is_none(), "the test's own bytes must decode cleanly");
    Block { at: 0, end: code.len(), insns, ends: Ends::FallsThrough, succ: Vec::new() }
}

fn arity(name: &str) -> Option<i64> {
    match name {
        "F1" => Some(1),
        "F2" => Some(2),
        _ => None,
    }
}

fn calls<const N: usize>(items: [(usize, &str); N]) -> IndexMap<i64, String> {
    items.into_iter().map(|(at, name)| (at as i64, name.to_owned())).collect()
}

#[test]
fn touches_sp_sees_writes_iced_gives_no_increment() {
    // Python: touches_sp is True for push ax, add sp,4 and leave; False for nop and mov ax,sp.
    for (bytes, expected) in [
        (&[0x50][..], true),
        (&[0x83, 0xC4, 0x04][..], true),
        (&[0xC9][..], true),
        (&[0x90][..], false),
        (&[0x89, 0xE0][..], false),
    ] {
        let insn = decode(bytes, 0).unwrap();
        assert_eq!(touches_sp(&insn), expected, "{bytes:02x?}");
    }
}

#[test]
fn test_two_contiguous_pushes_form_one_frame() {
    let block = block_of("66 FF 36 00 00  66 FF 36 00 00  9A 00 00 00 00");
    let call = block.insns.last().unwrap();
    let found = frames(&block, &calls([(call.at, "F2")]), &arity);
    assert_eq!(found.len(), 1);
    assert_eq!(&found[0].call, call);
    assert_eq!(found[0].pushed, block.insns[..2]);
}

#[test]
fn test_a_value_pushed_early_is_found_under_a_nested_call() {
    // push A -- for the OUTER call, but not consumed until the very end
    // push B / push C / call INNER(2) -- entirely self-contained, consumes B and C
    // push D -- the outer call's second argument
    // call OUTER(2) -- consumes A (stranded beneath the inner call) and D
    let block = block_of(concat!(
        "66 FF 36 00 00", // push A
        "66 FF 36 00 00", // push B
        "66 FF 36 00 00", // push C
        "9A 00 00 00 00", // call INNER
        "66 FF 36 00 00", // push D
        "9A 00 00 00 00", // call OUTER
    ));
    let [a, b, c, inner, d, outer] = <[_; 6]>::try_from(block.insns.clone()).unwrap();
    let found = frames(&block, &calls([(inner.at, "F2"), (outer.at, "F2")]), &arity);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].call, inner);
    assert_eq!(found[0].pushed, vec![b, c]);
    assert_eq!(found[1].call, outer);
    assert_eq!(found[1].pushed, vec![a, d], "a survives the nested call's own push and pop");
}

#[test]
fn test_an_ordinary_instruction_between_pushes_is_not_a_gap() {
    // mov ax,bp between two pushes touches neither sp nor either pushed value
    let block = block_of(concat!(
        "66 FF 36 00 00", // push A
        "8B C5",          // mov ax,bp -- provably does not touch sp
        "66 FF 36 00 00", // push B
        "9A 00 00 00 00", // call F2
    ));
    let [a, _mov, b, call] = <[_; 4]>::try_from(block.insns.clone()).unwrap();
    let found = frames(&block, &calls([(call.at, "F2")]), &arity);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pushed, vec![a, b]);
}

#[test]
fn test_a_pop_is_a_gap() {
    let block = block_of("66 FF 36 00 00  58  66 FF 36 00 00  9A 00 00 00 00");
    let call = block.insns.last().unwrap();
    assert!(frames(&block, &calls([(call.at, "F2")]), &arity).is_empty());
}

#[test]
fn test_arithmetic_on_sp_is_a_gap_even_though_iced_reports_no_increment() {
    // add sp,8 -- iced's own stack_pointer_increment is 0 for this, so only
    // the register-write check catches it
    let block = block_of("66 FF 36 00 00  83 C4 08  66 FF 36 00 00  9A 00 00 00 00");
    let call = block.insns.last().unwrap();
    assert!(frames(&block, &calls([(call.at, "F2")]), &arity).is_empty());
}

#[test]
fn test_a_call_needing_more_than_was_pushed_is_not_a_frame() {
    let block = block_of("66 FF 36 00 00  9A 00 00 00 00"); // one push, a call needing two
    let call = block.insns.last().unwrap();
    assert!(frames(&block, &calls([(call.at, "F2")]), &arity).is_empty());
}

#[test]
fn test_a_straddled_push_refuses_rather_than_over_counts() {
    // push dword(4) then push ax(2) -- 6 bytes on the stack, and F1 needs 4
    let block = block_of("66 FF 36 00 00  50  9A 00 00 00 00");
    let call = block.insns.last().unwrap();
    assert!(frames(&block, &calls([(call.at, "F1")]), &arity).is_empty());
}

#[test]
fn test_refusal_reset_must_not_be_reachable_past() {
    // push ax(2) / call F1 (needs 4, refused, resets) / push ax(2) / call F1
    let block = block_of("50  9A 00 00 00 00  50  9A 00 00 00 00");
    let (first_call, second_call) = (&block.insns[1], &block.insns[3]);
    let found = frames(&block, &calls([(first_call.at, "F1"), (second_call.at, "F1")]), &arity);
    assert!(found.is_empty());
}

#[test]
fn test_an_unrecognised_call_mid_block_is_a_gap() {
    let block = block_of("66 FF 36 00 00  9A 00 00 00 00  66 FF 36 00 00  9A 00 00 00 00");
    let (first_call, second_call) = (&block.insns[1], &block.insns[3]);
    let found = frames(&block, &calls([(first_call.at, "NOPE"), (second_call.at, "F1")]), &arity);
    assert_eq!(found.len(), 1);
    assert_eq!(&found[0].call, second_call);
    assert_eq!(found[0].pushed, vec![block.insns[2].clone()]);
}

#[test]
fn test_mixed_arity_in_one_block() {
    // F1(1) then F2(2), back to back, each popping a different amount.
    let block = block_of(concat!(
        "66 FF 36 00 00", // push A
        "9A 00 00 00 00", // call F1 -- consumes A alone
        "66 FF 36 00 00", // push B
        "66 FF 36 00 00", // push C
        "9A 00 00 00 00", // call F2 -- consumes B and C
    ));
    let [a, f1, b, c, f2] = <[_; 5]>::try_from(block.insns.clone()).unwrap();
    let found = frames(&block, &calls([(f1.at, "F1"), (f2.at, "F2")]), &arity);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].pushed, vec![a]);
    assert_eq!(found[1].pushed, vec![b, c]);
}

#[test]
fn test_arity_under_one_is_not_a_nameable_call() {
    let block = block_of("66 FF 36 00 00  9A 00 00 00 00");
    let call = block.insns.last().unwrap();
    let zero = |name: &str| (name == "F0").then_some(0);
    assert!(frames(&block, &calls([(call.at, "F0")]), &zero).is_empty());
}
