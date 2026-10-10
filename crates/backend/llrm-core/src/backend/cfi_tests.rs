use iced_x86::Register::{BP, EBP, ESI, ESP, SP};
use llrm_object::debug::FrameRow;

use super::rows;

fn row(
    offset: usize,
    register: &str,
    cfa: i64,
    saved: &[(&str, i64)],
) -> FrameRow {
    FrameRow {
        offset,
        cfa_register: register.into(),
        cfa_offset: cfa,
        saved: saved.iter().map(|&(name, at)| (name.to_owned(), at)).collect(),
    }
}

fn of(
    code: &[u8],
    pops: &[(usize, i64)],
) -> Result<Vec<FrameRow>, String> {
    rows(code, 32, EBP, ESP, 4, pops)
}

/// `push ebp; mov ebp, esp; sub esp, 8; nop; leave; ret`: the frame address is
/// the stack pointer's plus four at the entry, plus eight once ebp is saved,
/// the frame register's plus eight from `mov` to `leave`, and the
/// stack pointer's plus four again for the `ret`, ebp restored.
#[test]
fn a_frame_register_function_is_the_stack_pointer_then_the_frame_register_then_the_stack_pointer() {
    let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0x90, 0xC9, 0xC3];
    assert_eq!(
        of(&code, &[]).unwrap(),
        [
            row(0, "esp", 4, &[]),
            row(1, "esp", 8, &[("ebp", -8)]),
            row(3, "ebp", 8, &[("ebp", -8)]),
            row(8, "esp", 4, &[])
        ]
    );
}

/// A function with no frame register: each argument pushed moves the frame
/// address from the stack pointer, and the `add esp` after the call moves it
/// back. Without the rows, a debugger stopped in the callee took the
/// caller's frame to start four bytes after where it does.
#[test]
fn each_push_and_pop_moves_the_frame_address_from_the_stack_pointer() {
    // push 2; push 1; call f; add esp, 8; ret
    let code = [0x6A, 0x02, 0x6A, 0x01, 0xE8, 0, 0, 0, 0, 0x83, 0xC4, 0x08, 0xC3];
    assert_eq!(
        of(&code, &[]).unwrap(),
        [row(0, "esp", 4, &[]), row(2, "esp", 8, &[]), row(4, "esp", 12, &[]), row(12, "esp", 4, &[])]
    );
}

/// A callee that pops its arguments leaves the stack higher after the call with
/// no instruction to say so: the compiler's own record of what the callee pops
/// (`pops`, by where the call ends) is what moves the frame address.
#[test]
fn a_call_whose_callee_pops_its_arguments_moves_the_frame_address_by_what_it_pops() {
    let code = [0x6A, 0x02, 0x6A, 0x01, 0xE8, 0, 0, 0, 0, 0xC3];
    assert_eq!(
        of(&code, &[(9, 8)]).unwrap(),
        [row(0, "esp", 4, &[]), row(2, "esp", 8, &[]), row(4, "esp", 12, &[]), row(9, "esp", 4, &[])]
    );
}

/// A register saved where it is first needed and restored before each return: a
/// rule from its push to its pop on each path, and the branch's target has the
/// state its jump had, not the one the return before it left.
#[test]
fn a_saved_register_is_saved_from_its_push_to_each_pop_on_every_path() {
    // push esi; test eax, eax; jz 7; pop esi; ret; pop esi; ret
    let code = [0x56, 0x85, 0xC0, 0x74, 0x02, 0x5E, 0xC3, 0x5E, 0xC3];
    assert_eq!(
        of(&code, &[]).unwrap(),
        [
            row(0, "esp", 4, &[]),
            row(1, "esp", 8, &[("esi", -8)]),
            row(6, "esp", 4, &[]),
            row(7, "esp", 8, &[("esi", -8)]),
            row(8, "esp", 4, &[])
        ]
    );
    let _ = ESI;
}

/// Two paths that reach one place with different depths have no single rule:
/// refused, since a wrong one would unwind into the wrong frame.
#[test]
fn two_stack_depths_at_one_place_are_refused() {
    // test eax, eax; jz 5; push eax; ret
    let code = [0x85, 0xC0, 0x74, 0x01, 0x50, 0xC3];
    assert!(of(&code, &[]).unwrap_err().contains("two stack depths"));
}

/// A register pushed after it was written is a value, not the caller's: no
/// save.
#[test]
fn a_push_of_a_register_written_since_entry_is_no_save() {
    // mov esi, 1; push esi; add esp, 4; ret
    let code = [0xBE, 1, 0, 0, 0, 0x56, 0x83, 0xC4, 0x04, 0xC3];
    assert!(of(&code, &[]).unwrap().iter().all(|one| one.saved.is_empty()));
}

/// `push bp; mov bp, sp; sub sp, 8; leave; retf` in 16-bit code: the frame
/// register and the stack pointer are BP and SP, which the operands widen to
/// EBP and ESP; `leave` pops a word, not a dword. The frame address stayed the
/// stack pointer's plus six after `mov` and was minus two at the `retf`, so a
/// 16-bit function's frame description was wrong from its first row.
#[test]
fn a_16_bit_frame_is_followed_in_words() {
    let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0xC9, 0xCB];
    assert_eq!(
        rows(&code, 16, BP, SP, 4, &[]).unwrap(),
        [row(0, "sp", 4, &[]), row(1, "sp", 6, &[("bp", -6)]), row(3, "bp", 6, &[("bp", -6)]), row(7, "sp", 4, &[])]
    );
}
