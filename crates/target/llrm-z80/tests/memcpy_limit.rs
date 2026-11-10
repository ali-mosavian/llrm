//! A constant `memcpy` of more than one word is an `ldir`. The limit was the pin's x86 default (8 word pairs at speed, 4
//! at size), so `variadic` copied a 12 B struct argument twice as six word pairs, 72 B each: 547 B / 4729 T against 443 B /
//! 4523 T with the copies as `ldir` (hand edit, same judge).

use llrm_mir::target::Machine;
use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request, machine};
use llrm_z80::expand::{Line, price};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

fn copy(bytes: u32) -> String {
    format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg)\n\
         define void @cp(ptr %d, ptr %s) {{\n  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %d, ptr align 1 %s, i16 {bytes}, i1 false)\n  ret void\n}}\n"
    )
}

fn lines(text: &[&str]) -> Vec<Line> {
    text.iter().map(|one| Line::Inst((*one).to_owned())).collect()
}

/// The limit is the break-even of the two forms on the printer's own price table, not a number chosen: one word inline
/// is cheaper than the `ldir` block, two are dearer, with the operands in `hl` and `de` or in frame slots, in bytes
/// (-Os) and in bytes weighed at 3 T (-O2).
#[test]
fn the_limit_is_where_the_ldir_block_starts_to_win() {
    let pointers = ["ld a, (hl)", "ld (de), a", "inc hl", "inc de"];
    let frame = ["ld c, -2(ix)", "ld b, -1(ix)", "ld -4(ix), c", "ld -3(ix), b"];
    let block = ["ld hl, #_s", "ld de, #_d", "ld bc, #4", "ldir"];
    let framed_block = ["push ix", "pop de", "ld hl, #-4", "add hl, de", "ex de, hl", "push ix", "pop hl", "ld bc, #-2", "add hl, bc", "ld bc, #4", "ldir"];
    let ldir_clocks = |bytes: u32, setup: u32| setup + 21 * bytes - 5;
    let mut limit = 0;
    for words in 1..=4u32 {
        let inline_pointers = (2 * words * price(&lines(&pointers)).0, 2 * words * price(&lines(&pointers)).1);
        let inline_frame = (words * price(&lines(&frame)).0, words * price(&lines(&frame)).1);
        let (block_bytes, block_setup) = price(&lines(&block));
        let (framed_bytes, framed_setup) = price(&lines(&framed_block));
        // 4 pointers-form instructions copy one byte: two bytes a word
        let cheaper = |inline: (u32, u32), bytes: u32, setup: u32| {
            let ldir = (bytes, ldir_clocks(2 * words, setup));
            inline.0 < ldir.0 && 3 * inline.0 + inline.1 < 3 * ldir.0 + ldir.1
        };
        if cheaper(inline_pointers, block_bytes, block_setup) && cheaper(inline_frame, framed_bytes, framed_setup) {
            limit = words;
        }
    }
    assert_eq!(machine().tuning().max_stores_per_memcpy as u32, limit);
    assert_eq!(machine().tuning().max_stores_per_memcpy_opt_size as u32, limit);
}

/// At speed an `ldir`; tuned for size the runtime's `__memcpy` (`emit_memcpy_sized`: 4 B less at each site). Neither is the
/// six word pairs.
#[test]
fn a_struct_sized_copy_is_one_ldir_at_speed_and_one_call_at_size() {
    let o2 = text(&copy(12), "cp", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(o2.contains("ldir"), "-O2: a 12 B copy was inline:\n{o2}");
    let os = text(&copy(12), "cp", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(os.contains("__memcpy"), "-Os: a 12 B copy was inline:\n{os}");
    for body in [&o2, &os] {
        assert!(body.matches("ld (hl)").count() < 3, "word pairs left:\n{body}");
    }
}

/// A struct passed by value is copied into the outgoing arguments inside the call sequence, where the copy's `__memcpy` is
/// a nested call: its `ExternalSymbol` callee was left unconverted and `main` of `variadic` -Os was REFUSED
/// (`CannotSelect(ExternalSymbol)`), once the copy was no longer six inline word pairs.
#[test]
fn a_struct_passed_by_value_is_copied_by_a_call_inside_the_call_sequence() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n%struct.big = type {{ [6 x i16] }}\n\
         define i16 @cp(ptr %s) {{\n  %r = call i16 @f(ptr byval(%struct.big) align 1 %s, i16 7)\n  ret i16 %r\n}}\n\
         declare i16 @f(ptr byval(%struct.big) align 1, i16)\n"
    );
    for level in [Level::O2, Level::Os] {
        let body = text(&ir, "cp", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(body.contains("ldir") || body.contains("call __memcpy"), "{level:?}: copy is not the runtime's:\n{body}");
        assert!(!body.contains("call memcpy"), "{level:?}: the C name, not the runtime's `__memcpy`, would not link:\n{body}");
    }
}

/// The byval copy's `memcpy` call is lowered inside the variadic call it is for. It took that call's state: its count and
/// source went to the stack (`ld bc,#12; push bc` ... `call __memcpy` with only `hl` set), `variadic` -Os never reported, and
/// the variadic call lost the flag that its unnamed arguments go on the stack.
#[test]
fn the_copy_inside_a_variadic_call_takes_its_arguments_in_registers() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n%struct.big = type {{ [6 x i16] }}\n\
         define i16 @cp(ptr %s) {{\n  %r = call i16 (ptr, ...) @f(ptr byval(%struct.big) align 1 %s, i16 7)\n  ret i16 %r\n}}\n\
         declare i16 @f(ptr byval(%struct.big) align 1, ...)\n"
    );
    let body = text(&ir, "cp", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let before_call = body.split("call __memcpy").next().expect("a call of the runtime's memcpy");
    assert!(!before_call.contains("push bc"), "the count was pushed, not passed in bc:\n{body}");
    assert!(before_call.contains("ld bc, #12"), "no count in bc:\n{body}");
}

#[test]
fn a_one_word_copy_stays_inline() {
    for level in [Level::O2, Level::Os] {
        let body = text(&copy(2), "cp", 0, &Request::new(level)).expect("compiles");
        assert!(!body.contains("ldir") && !body.contains("__memcpy"), "{level:?}: a 2 B copy left the inline path:\n{body}");
    }
}
