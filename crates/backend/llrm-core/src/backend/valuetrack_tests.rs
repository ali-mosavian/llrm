use iced_x86::Register::{EAX, ECX, EDX};
use llrm_object::debug::FrameRow;

use super::{Mark, Place, Where, tracked};
use crate::model::lir::{DebugNote, NoteValue};

/// `push ebp; mov ebp, esp; mov eax, 5; mov [ebp-4], eax; add eax, 1; ret`
const FRAMED: [u8; 15] = [0x55, 0x89, 0xE5, 0xB8, 5, 0, 0, 0, 0x89, 0x45, 0xFC, 0x83, 0xC0, 1, 0xC3];

fn rows() -> Vec<FrameRow> {
    crate::backend::cfi::rows(&FRAMED, iced_x86::Register::EBP, iced_x86::Register::ESP, 4, &[]).expect("followable")
}

fn note(variable: u32, value: Option<u32>) -> DebugNote {
    DebugNote { variable, piece: None, value: value.map_or(NoteValue::Nothing, NoteValue::Value) }
}

/// A variable is where its value is: in the register the defining instruction wrote it, then, once that is overwritten, in the
/// frame cell a move copied it to. Reading the register alone lost it at the `add`; reading only the definition lost it at the move.
#[test]
fn a_variable_follows_its_value_from_the_register_to_the_cell_a_move_copied_it_to() {
    let marks = [(8, Mark::Def { tag: 7, place: Place::Register(EAX) }), (8, Mark::Note(0))];
    let found = tracked(&FRAMED, &rows(), 8, &[note(1, Some(7))], &marks);
    assert_eq!(found[&(1, None)], [(8, 14, Where::Place(Place::Register(EAX))), (14, 15, Where::Place(Place::Cell { disp: -4, bytes: 4 }))]);
}

/// Where another value is written over the register the variable was in, the variable has no place there: a register that
/// holds something else is no place for it.
#[test]
fn a_variable_has_no_place_once_nothing_holds_its_value() {
    // push ebp; mov ebp, esp; mov eax, 5; mov eax, 6; ret
    let code = [0x55, 0x89, 0xE5, 0xB8, 5, 0, 0, 0, 0xB8, 6, 0, 0, 0, 0xC3];
    let rows = crate::backend::cfi::rows(&code, iced_x86::Register::EBP, iced_x86::Register::ESP, 4, &[]).expect("followable");
    let marks = [(8, Mark::Def { tag: 7, place: Place::Register(EAX) }), (8, Mark::Note(0))];
    let found = tracked(&code, &rows, 8, &[note(1, Some(7))], &marks);
    assert_eq!(found[&(1, None)], [(8, 13, Where::Place(Place::Register(EAX)))]);
}

/// Where two paths join, the variable is where both say: one path moved the value to edx and overwrote eax, the other left it in
/// eax, so after the join it is in neither.
#[test]
fn paths_that_join_keep_only_the_place_both_hold() {
    // 0 mov eax,5; 5 test ecx,ecx; 7 je 14; 9 mov edx,eax; 11 xor eax,eax; 13 nop; 14 ret
    let code = [0xB8, 5, 0, 0, 0, 0x85, 0xC9, 0x74, 0x05, 0x89, 0xC2, 0x31, 0xC0, 0x90, 0xC3];
    let rows = crate::backend::cfi::rows(&code, iced_x86::Register::EBP, iced_x86::Register::ESP, 4, &[]).expect("followable");
    let marks = [(5, Mark::Def { tag: 7, place: Place::Register(EAX) }), (5, Mark::Note(0))];
    let found = tracked(&code, &rows, 8, &[note(1, Some(7))], &marks);
    assert_eq!(found[&(1, None)], [(5, 13, Where::Place(Place::Register(EAX))), (13, 14, Where::Place(Place::Register(EDX)))]);
}

/// A call loses what the callee clobbers, and only that.
#[test]
fn a_call_loses_the_registers_it_clobbers_and_keeps_the_others() {
    // mov eax, 5; mov edx, eax... encoded: mov eax,5; mov ecx,5; call +0; ret
    let code = [0xB8, 5, 0, 0, 0, 0xB9, 5, 0, 0, 0, 0xE8, 0, 0, 0, 0, 0xC3];
    let rows = crate::backend::cfi::rows(&code, iced_x86::Register::EBP, iced_x86::Register::ESP, 4, &[]).expect("followable");
    let marks = [
        (5, Mark::Def { tag: 7, place: Place::Register(EAX) }),
        (10, Mark::Def { tag: 8, place: Place::Register(ECX) }),
        (10, Mark::Note(0)),
        (10, Mark::Note(1)),
        // The call clobbers eax (bit 0) alone.
        (15, Mark::Clobbers(1)),
    ];
    let found = tracked(&code, &rows, 8, &[note(1, Some(7)), note(2, Some(8))], &marks);
    assert_eq!(found[&(1, None)], [(10, 15, Where::Place(Place::Register(EAX)))]);
    assert_eq!(found[&(2, None)], [(10, 16, Where::Place(Place::Register(ECX)))]);
}

/// A cell the function lets the address of out (`lea ecx, [ebp-4]`) may be written by the callee it is handed to: the value a variable
/// had in it is no longer known after the call. Before, `n` was read from `p.x`'s cell after `bump(&p, ..)` had added to it, and gdb
/// said `n = 23` where it is 3.
#[test]
fn a_cell_whose_address_went_out_is_lost_at_the_next_call() {
    // push ebp; mov ebp,esp; sub esp,8; mov eax,5; mov [ebp-4],eax; lea ecx,[ebp-4]; xor eax,eax; call +0; ret
    let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0xB8, 5, 0, 0, 0, 0x89, 0x45, 0xFC, 0x8D, 0x4D, 0xFC, 0x31, 0xC0, 0xE8, 0, 0, 0, 0, 0xC3];
    let rows = crate::backend::cfi::rows(&code, iced_x86::Register::EBP, iced_x86::Register::ESP, 4, &[]).expect("followable");
    let marks = [(11, Mark::Def { tag: 7, place: Place::Register(EAX) }), (11, Mark::Note(0))];
    let found = tracked(&code, &rows, 8, &[note(1, Some(7))], &marks);
    assert_eq!(found[&(1, None)], [(11, 19, Where::Place(Place::Register(EAX))), (19, 24, Where::Place(Place::Cell { disp: -4, bytes: 4 }))]);
}
