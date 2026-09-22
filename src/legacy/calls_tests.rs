//! Port of `tests/test_calls.py`'s absorb half. The tests that find their
//! site with `sites`/`match` wait for the BC raise, which owns them. The
//! crate builds iced without a formatter, so `str(instruction)` checks
//! compare the decoded instruction's parts instead.

use iced_x86::{Mnemonic, OpKind};

use super::*;
use crate::frontend::declen::decode;

fn hx(text: &str) -> Vec<u8> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap()).collect()
}

fn decoded(code: &[u8]) -> Vec<Instruction> {
    Decoder::with_ip(BITNESS, code, 0, DecoderOptions::NONE).into_iter().collect()
}

fn one(code: &str) -> Insn {
    decode(&hx(code), 0).unwrap()
}

fn site(name: &str, pushed: Vec<Operand>, consume: Vec<Insn>) -> CallSite {
    CallSite { at: 0, end: 0, start: 0, name: name.to_owned(), pushed, consume }
}

fn static_operand(offset: i64, base: Register) -> Operand {
    Operand {
        addr: Some(Addr { base, ..Addr::new(Space::Segment, offset) }),
        at: Some(offset as usize),
        length: 1,
        ..Operand::new(Kind::Static)
    }
}

fn constant_operand(value: i64) -> Operand {
    Operand { value, length: 1, ..Operand::new(Kind::Constant) }
}

fn encoded(instruction: Instruction) -> Instruction {
    let block = InstructionBlock::new(&[instruction], 0);
    let code = BlockEncoder::encode(BITNESS, block, BlockEncoderOptions::NONE).unwrap().code_buffer;
    decoded(&code)[0]
}

#[test]
fn test_absorbed_compare_against_a_constant_wraps_eax_too() {
    let site = site(COMPARE, vec![static_operand(0x10, Register::None), constant_operand(70000)], vec![]);
    let emitted = absorb(&site, Flag::NONE, true).unwrap();
    let decoded = decoded(&emitted.code);
    assert!(decoded[0].mnemonic() == Mnemonic::Push && decoded[0].op0_register() == Register::EAX);
    let last = decoded.last().unwrap();
    assert!(last.mnemonic() == Mnemonic::Pop && last.op0_register() == Register::EAX);
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Cmp));
}

#[test]
fn test_squaring_the_same_address_loads_it_once() {
    let x = static_operand(0x76, Register::None);
    let site = site(MULTIPLY, vec![x.clone(), x], vec![]);
    let emitted = absorb(&site, Flag::NONE, true).unwrap();
    // mov eax,ds:[x] / imul eax,eax / push eax,pop ax,pop dx -- one load, not two
    assert_eq!(emitted.code, hx("66 A1 00 00  66 0F AF C0  66 50 58 5A"));
    assert_eq!(emitted.relocations, [(2, 0x76)], "one fixup, not two, for the one address read");
}

/// A word pair then a dword push: byte-counting from the top lands on the
/// boundary between two arguments regardless of their shapes.
#[test]
fn test_grouped_splits_mixed_shapes_by_byte_count() {
    let (hi, lo) = (one("52"), one("50"));
    let dword = one("66 FF 36 00 00");
    assert_eq!(grouped(&[hi.clone(), lo.clone(), dword.clone()]), Some(vec![vec![hi, lo], vec![dword]]));
}

/// (word, dword, word) sums to 8 but no 4-byte prefix from the top is a real argument.
#[test]
fn test_grouped_refuses_a_word_pair_split_across_two_arguments() {
    let (w1, w2) = (one("52"), one("50"));
    let dword = one("66 FF 36 00 00");
    assert_eq!(grouped(&[w1, dword, w2]), None);
}

#[test]
fn test_consume_refuses_rather_than_crashes_on_an_ungroupable_frame() {
    let (w1, w2) = (one("52"), one("50"));
    let dword = one("66 FF 36 00 00");
    let site = site(MULTIPLY, vec![], vec![w1, dword, w2]);
    assert!(consume(&site, Flag::NONE, true).is_err());
}

#[test]
fn test_popped_into_is_a_bare_pop() {
    assert_eq!(popped_into(Register::ECX), Instruction::with1(Code::Pop_r32, Register::ECX).unwrap());
}

/// bp holds the frame compare_consume() builds, since sp cannot be a 16-bit base.
#[test]
fn test_consume_pops_a_dword_and_a_word_pair_for_compare() {
    let (hi, lo) = (one("52"), one("50"));
    let dword = one("66 FF 36 00 00");
    let site = site(COMPARE, vec![], vec![hi, lo, dword]);
    let emitted = consume(&site, Flag::NONE, true).unwrap();
    assert!(emitted.relocations.is_empty(), "nothing here is a relocated address");
    let decoded = decoded(&emitted.code);
    assert!(decoded[0].mnemonic() == Mnemonic::Push && decoded[0].op0_register() == Register::BP);
    assert!(decoded[1].mnemonic() == Mnemonic::Push && decoded[1].op0_register() == Register::EDX);
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Cmp));
    // Two pushes (-6), the lea's own displacement (+12) and the final pop (+2):
    // iced's stack_pointer_increment does not follow an arbitrary write to sp.
    let lea = decoded.iter().find(|insn| insn.mnemonic() == Mnemonic::Lea).unwrap();
    assert_eq!(lea.op0_register(), Register::SP);
    assert_eq!(lea.memory_displacement64(), 12);
    assert_eq!(decoded.iter().map(|insn| insn.stack_pointer_increment()).sum::<i32>() + 12, 8);
    let last = decoded.last().unwrap();
    assert!(last.mnemonic() == Mnemonic::Pop && last.op0_register() == Register::BP);
}

/// Walks a 16-bit stream tracking sp/bp and the registers push/pop/mov touch,
/// and fails the moment something reads memory below the current sp: DOS
/// services interrupts at instruction boundaries, onto the live stack.
fn simulated_reads_never_land_below_sp(code: &[u8], entry_sp: i64) {
    let mut regs: IndexMap<Register, i64> =
        IndexMap::from_iter([(Register::BP, 0x4000), (Register::EDX, 0x1234_5678), (Register::SP, entry_sp)]);
    let mut mem: IndexMap<i64, i64> = IndexMap::default();
    let widths = |reg: Register| {
        if matches!(reg, Register::EAX | Register::ECX | Register::EDX | Register::EBX) { 4 } else { 2 }
    };
    let mem_addr = |regs: &IndexMap<Register, i64>, insn: &Instruction| {
        let base = insn.memory_base();
        assert!(regs.contains_key(&base), "unhandled base register {base:?}");
        regs[&base] + insn.memory_displacement64() as i16 as i64
    };
    let read_mem = |regs: &IndexMap<Register, i64>, mem: &IndexMap<i64, i64>, addr: i64, size: u32| {
        assert!(addr >= regs[&Register::SP], "read at {addr:#x} is below sp {:#x}", regs[&Register::SP]);
        mem.get(&addr).copied().unwrap_or(0) & ((1i64 << (size * 8)) - 1)
    };
    for insn in decoded(code) {
        match insn.mnemonic() {
            Mnemonic::Push => {
                let size = widths(insn.op0_register());
                *regs.get_mut(&Register::SP).unwrap() -= size as i64;
                let value = regs[&insn.op0_register()] & ((1i64 << (size * 8)) - 1);
                mem.insert(regs[&Register::SP], value);
            }
            Mnemonic::Pop => {
                let size = widths(insn.op0_register());
                let value = read_mem(&regs, &mem, regs[&Register::SP], size);
                regs.insert(insn.op0_register(), value);
                *regs.get_mut(&Register::SP).unwrap() += size as i64;
            }
            Mnemonic::Mov if insn.op1_kind() == OpKind::Memory => {
                let value = read_mem(&regs, &mem, mem_addr(&regs, &insn), widths(insn.op0_register()));
                regs.insert(insn.op0_register(), value);
            }
            Mnemonic::Mov if insn.op0_kind() == OpKind::Memory => {
                let size = widths(insn.op1_register());
                let value = regs[&insn.op1_register()] & ((1i64 << (size * 8)) - 1);
                mem.insert(mem_addr(&regs, &insn), value);
            }
            Mnemonic::Mov => {
                let value = regs[&insn.op1_register()];
                regs.insert(insn.op0_register(), value);
            }
            Mnemonic::Cmp => {
                read_mem(&regs, &mem, mem_addr(&regs, &insn), widths(insn.op0_register()));
            }
            Mnemonic::Lea => {
                let value = mem_addr(&regs, &insn);
                regs.insert(insn.op0_register(), value);
            }
            other => panic!("simulator does not know {other:?}"),
        }
    }
}

/// compare_consume() borrows bp inside the call's dead argument space; every
/// register it restores from there must be read at or above sp.
#[test]
fn test_consume_compare_never_reads_below_the_stack_pointer() {
    let (hi, lo) = (one("52"), one("50"));
    let dword = one("66 FF 36 00 00");
    let site = site(COMPARE, vec![], vec![hi, lo, dword]);
    let emitted = consume(&site, Flag::NONE, true).unwrap();
    simulated_reads_never_land_below_sp(&emitted.code, 0x2000);
}

/// A static-looking push is still popped, never reloaded: reloading one push
/// while leaving it on the stack would leak four bytes per call.
#[test]
fn test_consume_pops_every_argument_even_one_shaped_like_a_static() {
    let static_looking = one("66 FF 36 00 00");
    let (hi, lo) = (one("52"), one("50"));
    let site = site(MULTIPLY, vec![], vec![static_looking, hi, lo]);
    let emitted = consume(&site, Flag::NONE, true).unwrap();
    assert!(emitted.relocations.is_empty(), "nothing here is a relocated address to reuse a fixup for");
    let decoded = decoded(&emitted.code);
    assert!(
        !decoded.iter().any(|insn| insn.is_ip_rel_memory_operand() || insn.memory_base() != Register::None),
        "nothing reads the static-looking push's address -- it was popped, not reloaded"
    );
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Pop));
    assert_eq!(decoded.iter().map(|insn| insn.stack_pointer_increment()).sum::<i32>(), 8);
}

/// DIVIDE pushes the divisor first: swapping which lands in eax is a silent wrong answer.
#[test]
fn test_consume_divide_puts_the_dividend_in_eax_not_the_divisor() {
    let divisor = one("66 FF 36 00 00");
    let dividend = one("66 FF 76 00 00");
    let site = site(DIVIDE, vec![], vec![divisor, dividend]);
    let emitted = consume(&site, Flag::NONE, true).unwrap();
    // pop eax / pop ecx / cdq / idiv ecx / mov ebx,edx / restore
    assert_eq!(emitted.code, hx("66 58  66 59  66 99  66 F7 F9  66 8B DA  66 50 58 5A"));
}

#[test]
fn test_consume_refuses_a_compare_whose_synthesised_flags_are_read() {
    let dword = one("66 FF 36 00 00");
    let site = site(COMPARE, vec![], vec![dword.clone(), dword]);
    assert!(consume(&site, Flag::CF, true).is_err());
}

#[test]
fn test_consume_refuses_a_call_whose_arity_does_not_match_what_was_pushed() {
    let site = site(MULTIPLY, vec![], vec![one("66 FF 36 00 00")]);
    assert!(consume(&site, Flag::NONE, true).is_err());
}

/// gcc and clang at -O3 pick these on i386. Safe only because absorb() has
/// already refused any MULTIPLY site whose flags are read afterwards.
#[test]
fn test_a_constant_multiply_the_386_can_do_without_multiplying() {
    for (value, shift) in [(2, 1), (4, 2), (16, 4), (256, 8)] {
        let got = encoded(_without_multiplying(value).unwrap());
        // shl eax,{shift}
        assert_eq!(got.mnemonic(), Mnemonic::Shl, "{value}");
        assert_eq!(got.op0_register(), Register::EAX);
        assert_eq!(got.immediate8(), shift);
    }
    for (value, scale) in [(3, 2), (5, 4), (9, 8)] {
        let got = encoded(_without_multiplying(value).unwrap());
        // lea eax,[eax+eax*{scale}]
        assert_eq!(got.mnemonic(), Mnemonic::Lea, "{value}");
        assert_eq!(got.op0_register(), Register::EAX);
        assert_eq!((got.memory_base(), got.memory_index(), got.memory_index_scale()), (Register::EAX, Register::EAX, scale));
        assert_eq!(got.memory_displacement64(), 0);
    }
}

/// Refused rather than special-cased: an unmeasured sequence is a case nothing checks.
#[test]
fn test_anything_else_still_multiplies() {
    for value in [0, 1, -1, -4, 7, 10, 100] {
        assert!(_without_multiplying(value).is_none(), "{value}");
    }
}

/// A compare by 3 is not a lea by any reading.
#[test]
fn test_strength_reduction_only_applies_to_the_multiply() {
    let operand = Operand { value: 3, ..Operand::new(Kind::Constant) };
    assert_eq!(encoded(apply_to(COMPARE, &operand)).mnemonic(), Mnemonic::Cmp);
}

#[test]
fn test_a_power_of_two_divisor_is_recognised() {
    for value in [2i64, 16, 512, 262144, 1 << 31] {
        assert_eq!(_power_of_two(value), Some(63 - value.leading_zeros() as i64));
    }
}

/// 1 is excluded: dividing by it is a no-op, not a shift by zero.
#[test]
fn test_anything_else_is_not() {
    for value in [0i64, 1, -2, -512, 3, 7, 100, 1 << 32] {
        assert_eq!(_power_of_two(value), None, "{value}");
    }
}

const MASK: i64 = 0xFFFF_FFFF;

fn _signed(v: i64) -> i64 {
    if v & 0x8000_0000 != 0 { v - (1 << 32) } else { v }
}

/// The sequence dividing_by_a_power_of_two emits, executed.
fn _model(name: &str, x: i64, n: i64) -> i64 {
    let eax = x & MASK;
    let mut scratch = (_signed(x & MASK) >> 31) & MASK;
    scratch = (scratch & MASK) >> (32 - n);
    if name == REMAINDER {
        scratch = (scratch + eax) & MASK;
        scratch &= -(1i64 << n) & MASK;
        return _signed((eax - scratch) & MASK);
    }
    let eax = (eax + scratch) & MASK;
    _signed((_signed(eax) >> n) & MASK)
}

fn _truncating(a: i64, b: i64) -> i64 {
    let q = a.abs() / b.abs();
    if (a < 0) == (b < 0) { q } else { -q }
}

/// A signed shift alone rounds towards minus infinity; the bias is the whole
/// difference from idiv's truncation.
#[test]
fn test_the_shift_sequence_truncates_towards_zero_like_idiv() {
    let xs = [0, 1, -1, 2, -2, 511, 512, 513, -511, -512, -513, (1i64 << 31) - 1, -(1i64 << 31), 123456789, -123456789];
    for n in 1..32 {
        for x in xs {
            let divisor = 1i64 << n;
            assert_eq!(_model(DIVIDE, x, n), _truncating(x, divisor), "{n} {x}");
            assert_eq!(_model(REMAINDER, x, n), x - _truncating(x, divisor) * divisor, "{n} {x}");
        }
    }
}
