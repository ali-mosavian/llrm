//! Port of `tests/test_calls.py`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind};

use super::*;
use crate::frontends::bc::blocks::{code_map, instructions, partition};
use crate::frontends::bc::declen::decode;
use crate::legacy::lift::FIXUP;
use crate::objectfile::module::tests::{bare, fixtures, loaded};

/// helpers' `hx`.
fn hx(s: &str) -> Vec<u8> {
    let digits: String = s.split_whitespace().collect();
    (0..digits.len()).step_by(2).map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap()).collect()
}

fn one(s: &str) -> Insn {
    decode(&hx(s), 0).unwrap()
}

fn decoded(code: &[u8]) -> Vec<Instruction> {
    Decoder::with_ip(BITNESS, code, 0, DecoderOptions::NONE).into_iter().collect()
}

/// `corpus.loaded`, `corpus.reached` and `corpus.partitioned` of one object.
fn corpus(path: &Path) -> (Module, Vec<Insn>, Vec<Block>) {
    let parsed = loaded(path).unwrap();
    let reached = instructions(&parsed).unwrap();
    let blocks = partition(&parsed, &code_map(&parsed).unwrap());
    (parsed, reached, blocks)
}

fn found_sites(obj: &Path) -> BTreeMap<String, (Operand, Operand)> {
    let (parsed, reached, blocks) = corpus(obj);
    sites(&parsed, &reached, &blocks).into_iter().map(|site| (site.name.clone(), site.operands())).collect()
}

fn operator_objects() -> Vec<PathBuf> {
    ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"].iter().map(|name| fixtures().join(name)).collect()
}

#[test]
fn test_compare_and_divide_agree_on_which_operand_is_left() {
    for obj in operator_objects() {
        let by_name = found_sites(&obj);
        assert!(by_name.contains_key(COMPARE) && by_name.contains_key(DIVIDE), "{obj:?}");
        let (left, right) = &by_name[COMPARE];
        let (first, second) = &by_name[DIVIDE];
        assert_eq!((left.addr, right.addr), (first.addr, second.addr), "{obj:?}");
        assert_ne!(left.addr, right.addr, "and they are two different variables");
    }
}

#[test]
fn test_both_push_shapes_reach_the_same_operands() {
    let wide = found_sites(&fixtures().join("vbdos-g3.obj"));
    let narrow = found_sites(&fixtures().join("vbdos-g2.obj"));
    for name in [COMPARE, DIVIDE] {
        assert_eq!((wide[name].0.addr, wide[name].1.addr), (narrow[name].0.addr, narrow[name].1.addr));
    }
}

#[test]
fn test_only_comparison_pushes_its_left_operand_first() {
    for (name, left_first) in LEFT_FIRST.iter() {
        assert_eq!(*left_first, *name == COMPARE);
    }
}

#[test]
fn test_a_call_with_anything_between_the_pushes_is_refused() {
    let parsed = loaded(fixtures().join("vbdos-g3.obj")).unwrap();
    let reached = instructions(&parsed).unwrap();
    let index = reached
        .iter()
        .position(|insn| parsed.calls.get(&(insn.at as i64)).map(String::as_str) == Some(COMPARE))
        .unwrap();
    assert!(r#match(&parsed, &reached, index).is_some());
    // the same call one instruction further back has a push missing
    let shorter: Vec<Insn> = reached[..index - 1].iter().chain(&reached[index..]).cloned().collect();
    assert!(r#match(&parsed, &shorter, index - 1).is_none());
}

#[test]
fn test_a_pushed_constant_is_not_a_static() {
    let parsed = loaded(fixtures().join("vbdos-g3.obj")).unwrap();
    let immediate = one("66 68 78 56 34 12"); // push dword 0x12345678
    assert!(static_at(&parsed, &immediate).is_none());
}

#[test]
fn test_a_pushed_negative_dword_constant_keeps_its_sign() {
    // iced's immediate() is unsigned for every PUSH*_IMM* form; absorbed
    // unsigned, iced's own i32 builder raised -- found by tools/fuzzcheck.py.
    let pushed = one("66 68 a8 16 8c 8a"); // push dword -1970530648
    assert_eq!(constant_at(&pushed).unwrap().value, -1970530648);
}

/// `module.Module([], 1, "test", b"", 0, 0, operands={at: addr})`.
fn with_operand(at: usize, addr: Addr) -> Module {
    let mut fake = bare(&loaded(fixtures().join("vbdos-g3.obj")).unwrap(), Vec::new(), 0, 0);
    fake.operands.insert(at as i64, addr);
    fake
}

#[test]
fn test_an_indexed_push_is_a_static_operand_carrying_its_base() {
    let insn = one("66 FF B4 10 00"); // push dword [si+0x10]
    let fake = with_operand(insn.disp_at.unwrap(), Addr { index: 5, ..Addr::new(Space::Segment, 0) });
    let found = static_at(&fake, &insn).unwrap();
    assert_eq!(found.addr, Some(Addr { index: 5, base: Register::SI, ..Addr::new(Space::Segment, 0) }));
}

#[test]
fn test_a_scaled_index_push_is_not_a_static_operand() {
    let insn = one("66 67 FF 34 85 10 00 00 00"); // push dword [eax*4+0x10], no base at all
    let fake = with_operand(insn.disp_at.unwrap(), Addr { index: 5, ..Addr::new(Space::Segment, 0) });
    assert!(static_at(&fake, &insn).is_none());
}

fn first_compare(obj: &str) -> CallSite {
    let (parsed, reached, blocks) = corpus(&fixtures().join(obj));
    sites(&parsed, &reached, &blocks).into_iter().find(|site| site.name == COMPARE).unwrap()
}

#[test]
fn test_an_absorbed_comparison_leaves_no_value_to_restore() {
    // `site.name is COMPARE` compared identity once, and appended the restore.
    let emitted = absorb(&first_compare("cmpord-v-g3.obj"), Flag::NONE, true).unwrap();
    // push eax / mov eax,[a] / cmp eax,[b] / pop eax
    assert_eq!(emitted.code.len(), 13);
    assert!(!emitted.code.windows(4).any(|window| window == FIXUP[&0]));
}

#[test]
fn test_absorbed_compare_restores_eax() {
    // B$CPI4 changes nothing but the flags, so eax comes back as found.
    let emitted = absorb(&first_compare("cmpord-v-g3.obj"), Flag::NONE, true).unwrap();
    let decoded = decoded(&emitted.code);
    assert!(decoded[0].mnemonic() == Mnemonic::Push && decoded[0].op0_register() == Register::EAX);
    let last = decoded.last().unwrap();
    assert!(last.mnemonic() == Mnemonic::Pop && last.op0_register() == Register::EAX);
    assert!(decoded[1..decoded.len() - 1].iter().any(|insn| insn.op0_register() == Register::EAX));
}

fn static_operand(offset: i64) -> Operand {
    Operand {
        addr: Some(Addr::new(Space::Segment, offset)),
        at: Some(offset as usize),
        length: 1,
        ..Operand::new(Kind::Static)
    }
}

fn constant_operand(value: i64) -> Operand {
    Operand { value, length: 1, ..Operand::new(Kind::Constant) }
}

fn site(name: &str, pushed: Vec<Operand>, consume: Vec<Insn>) -> CallSite {
    CallSite { at: 0, end: 0, start: 0, name: name.to_owned(), pushed, consume }
}

#[test]
fn test_absorbed_compare_against_a_constant_wraps_eax_too() {
    let emitted =
        absorb(&site(COMPARE, vec![static_operand(0x10), constant_operand(70000)], Vec::new()), Flag::NONE, true)
            .unwrap();
    let decoded = decoded(&emitted.code);
    assert!(decoded[0].mnemonic() == Mnemonic::Push && decoded[0].op0_register() == Register::EAX);
    let last = decoded.last().unwrap();
    assert!(last.mnemonic() == Mnemonic::Pop && last.op0_register() == Register::EAX);
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Cmp));
}

#[test]
fn test_squaring_the_same_address_loads_it_once() {
    let x = static_operand(0x76);
    let emitted = absorb(&site(MULTIPLY, vec![x.clone(), x], Vec::new()), Flag::NONE, true).unwrap();
    // mov eax,ds:[x] / imul eax,eax / push eax,pop ax,pop dx -- one load, not two
    assert_eq!(emitted.code, hx("66 A1 00 00  66 0F AF C0  66 50 58 5A"));
    assert_eq!(emitted.relocations, vec![(2, 0x76)], "one fixup, not two, for the one address read");
}

#[test]
fn test_grouped_splits_mixed_shapes_by_byte_count() {
    let (hi, lo, dword) = (one("52"), one("50"), one("66 FF 36 00 00"));
    assert_eq!(grouped(&[hi.clone(), lo.clone(), dword.clone()]), Some(vec![vec![hi, lo], vec![dword]]));
}

#[test]
fn test_grouped_refuses_a_word_pair_split_across_two_arguments() {
    // (word, dword, word) sums to 8 but no 4-byte prefix from the top is one argument.
    assert_eq!(grouped(&[one("52"), one("66 FF 36 00 00"), one("50")]), None);
}

#[test]
fn test_consume_refuses_rather_than_crashes_on_an_ungroupable_frame() {
    let made = site(MULTIPLY, Vec::new(), vec![one("52"), one("66 FF 36 00 00"), one("50")]);
    assert!(consume(&made, Flag::NONE, true).is_err());
}

#[test]
fn test_popped_into_is_a_bare_pop() {
    assert_eq!(popped_into(Register::ECX), Instruction::with1(Code::Pop_r32, Register::ECX).unwrap());
}

fn compare_frame() -> CallSite {
    site(COMPARE, Vec::new(), vec![one("52"), one("50"), one("66 FF 36 00 00")])
}

#[test]
fn test_consume_pops_a_dword_and_a_word_pair_for_compare() {
    let emitted = consume(&compare_frame(), Flag::NONE, true).unwrap();
    assert!(emitted.relocations.is_empty(), "nothing here is a relocated address");
    let decoded = decoded(&emitted.code);
    assert!(decoded[0].mnemonic() == Mnemonic::Push && decoded[0].op0_register() == Register::BP);
    assert!(decoded[1].mnemonic() == Mnemonic::Push && decoded[1].op0_register() == Register::EDX);
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Cmp));
    // the two pushes (-6), the lea's displacement (+12) and the final pop (+2)
    let lea = decoded.iter().find(|insn| insn.mnemonic() == Mnemonic::Lea).unwrap();
    assert_eq!(lea.op0_register(), Register::SP);
    assert_eq!(lea.memory_displacement64(), 12);
    assert_eq!(decoded.iter().map(|insn| insn.stack_pointer_increment()).sum::<i32>() + 12, 8);
    let last = decoded.last().unwrap();
    assert!(last.mnemonic() == Mnemonic::Pop && last.op0_register() == Register::BP);
}

/// Walk a 16-bit instruction stream and fail the moment something reads
/// memory below the current sp, where an interrupt may have written.
fn simulated_reads_never_land_below_sp(code: &[u8]) {
    let mut regs: BTreeMap<Register, i64> =
        BTreeMap::from([(Register::BP, 0x4000), (Register::EDX, 0x1234_5678), (Register::SP, 0x2000)]);
    let mut mem: BTreeMap<i64, i64> = BTreeMap::new();

    let widths = |reg: Register| -> i64 {
        if [Register::EAX, Register::ECX, Register::EDX, Register::EBX].contains(&reg) { 4 } else { 2 }
    };
    let mem_addr = |regs: &BTreeMap<Register, i64>, insn: &Instruction| -> i64 {
        let base = insn.memory_base();
        regs.get(&base).unwrap_or_else(|| panic!("unhandled base register {base:?}")) + insn.memory_displacement64() as i64
    };
    let read_mem = |regs: &BTreeMap<Register, i64>, mem: &BTreeMap<i64, i64>, addr: i64, size: i64| -> i64 {
        assert!(addr >= regs[&Register::SP], "read at {addr:#x} is below sp {:#x}", regs[&Register::SP]);
        mem.get(&addr).copied().unwrap_or(0) & ((1 << (size * 8)) - 1)
    };

    for insn in decoded(code) {
        match insn.mnemonic() {
            Mnemonic::Push => {
                let size = widths(insn.op0_register());
                *regs.get_mut(&Register::SP).unwrap() -= size;
                let value = regs[&insn.op0_register()] & ((1 << (size * 8)) - 1);
                mem.insert(regs[&Register::SP], value);
            }
            Mnemonic::Pop => {
                let size = widths(insn.op0_register());
                let value = read_mem(&regs, &mem, regs[&Register::SP], size);
                regs.insert(insn.op0_register(), value);
                *regs.get_mut(&Register::SP).unwrap() += size;
            }
            Mnemonic::Mov if insn.op1_kind() == OpKind::Memory => {
                let value = read_mem(&regs, &mem, mem_addr(&regs, &insn), widths(insn.op0_register()));
                regs.insert(insn.op0_register(), value);
            }
            Mnemonic::Mov if insn.op0_kind() == OpKind::Memory => {
                let size = widths(insn.op1_register());
                mem.insert(mem_addr(&regs, &insn), regs[&insn.op1_register()] & ((1 << (size * 8)) - 1));
            }
            Mnemonic::Mov => {
                regs.insert(insn.op0_register(), regs[&insn.op1_register()]);
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

#[test]
fn test_consume_compare_never_reads_below_the_stack_pointer() {
    let emitted = consume(&compare_frame(), Flag::NONE, true).unwrap();
    simulated_reads_never_land_below_sp(&emitted.code);
}

#[test]
fn test_consume_pops_every_argument_even_one_shaped_like_a_static() {
    // Reloading one push while leaving it on the stack would leak four bytes per call.
    let made = site(MULTIPLY, Vec::new(), vec![one("66 FF 36 00 00"), one("52"), one("50")]);
    let emitted = consume(&made, Flag::NONE, true).unwrap();
    assert!(emitted.relocations.is_empty(), "nothing here is a relocated address to reuse a fixup for");
    let decoded = decoded(&emitted.code);
    assert!(
        !decoded.iter().any(|insn| insn.is_ip_rel_memory_operand() || insn.memory_base() != Register::None),
        "nothing reads the static-looking push's address -- it was popped, not reloaded"
    );
    assert!(decoded.iter().any(|insn| insn.mnemonic() == Mnemonic::Pop));
    assert_eq!(decoded.iter().map(|insn| insn.stack_pointer_increment()).sum::<i32>(), 8);
}

#[test]
fn test_consume_divide_puts_the_dividend_in_eax_not_the_divisor() {
    // Swapping which one lands in eax is a silent wrong answer, so byte-exact.
    let made = site(DIVIDE, Vec::new(), vec![one("66 FF 36 00 00"), one("66 FF 76 00 00")]);
    let emitted = consume(&made, Flag::NONE, true).unwrap();
    assert_eq!(emitted.code, hx("66 58  66 59  66 99  66 F7 F9  66 8B DA  66 50 58 5A"));
}

#[test]
fn test_absorb_reloads_a_classified_frame_site_rather_than_popping_it() {
    // lngmix-p-g2's B$RMI4 is a frames() site whose pushes still classify;
    // popping it read garbage and stopped the program early under DOSBox.
    let (parsed, reached, blocks) = corpus(&fixtures().join("lngmix-p-g2.obj"));
    let found: Vec<CallSite> =
        sites(&parsed, &reached, &blocks).into_iter().filter(|one| one.name == REMAINDER).collect();
    assert_eq!(found.len(), 1, "expected one B$RMI4 site, found {}", found.len());
    let site = &found[0];
    assert!(!site.consume.is_empty() && !site.pushed.is_empty(), "the shape this bug needs");
    let emitted = absorb(site, Flag::NONE, true).unwrap();
    let first = decoded(&emitted.code)[0];
    assert_eq!(first.mnemonic(), Mnemonic::Mov, "the operand was popped, not reloaded");
}

#[test]
fn test_consume_refuses_a_compare_whose_synthesised_flags_are_read() {
    let dword = one("66 FF 36 00 00");
    assert!(consume(&site(COMPARE, Vec::new(), vec![dword.clone(), dword]), Flag::CF, true).is_err());
}

#[test]
fn test_consume_refuses_a_call_whose_arity_does_not_match_what_was_pushed() {
    assert!(consume(&site(MULTIPLY, Vec::new(), vec![one("66 FF 36 00 00")]), Flag::NONE, true).is_err());
}

/// The one instruction encoded and decoded back.
fn round_trip(insn: Instruction) -> Instruction {
    let mut steps = [insn];
    decoded(&assemble(&mut steps, &[]).unwrap().code)[0]
}

#[test]
fn test_a_constant_multiply_the_386_can_do_without_multiplying() {
    // gcc and clang at -O3 pick these on i386; safe only because absorb()
    // refuses a MULTIPLY whose flags are read.
    for (value, shift) in [(2, 1), (4, 2), (16, 4), (256, 8)] {
        let found = round_trip(_without_multiplying(value).unwrap());
        assert_eq!((found.mnemonic(), found.op0_register(), found.immediate8()), (Mnemonic::Shl, Register::EAX, shift));
    }
    for (value, scale) in [(3, 2), (5, 4), (9, 8)] {
        let found = round_trip(_without_multiplying(value).unwrap());
        assert_eq!(
            (found.mnemonic(), found.op0_register(), found.memory_base(), found.memory_index()),
            (Mnemonic::Lea, Register::EAX, Register::EAX, Register::EAX)
        );
        assert_eq!((found.memory_index_scale(), found.memory_displacement64()), (scale, 0));
    }
}

#[test]
fn test_anything_else_still_multiplies() {
    for value in [0, 1, -1, -4, 7, 10, 100] {
        assert!(_without_multiplying(value).is_none(), "{value}");
    }
}

#[test]
fn test_strength_reduction_only_applies_to_the_multiply() {
    let operand = Operand { value: 3, ..Operand::new(Kind::Constant) };
    assert_eq!(round_trip(apply_to(COMPARE, &operand)).mnemonic(), Mnemonic::Cmp);
}

#[test]
fn test_a_power_of_two_divisor_is_recognised() {
    for value in [2_i64, 16, 512, 262144, 1 << 31] {
        assert_eq!(_power_of_two(value), Some(63 - value.leading_zeros()));
    }
}

#[test]
fn test_anything_else_is_not() {
    for value in [0, 1, -2, -512, 3, 7, 100, 1_i64 << 32] {
        assert_eq!(_power_of_two(value), None, "{value}");
    }
}

/// The sequence dividing_by_a_power_of_two emits, executed.
fn model(name: &str, x: i64, n: u32) -> i64 {
    let (eax, scratch) = (x as u32, x as u32);
    let scratch = ((scratch as i32) >> 31) as u32;
    let scratch = scratch >> (32 - n);
    if name == REMAINDER {
        let scratch = scratch.wrapping_add(eax) & (-(1_i64 << n)) as u32;
        return eax.wrapping_sub(scratch) as i32 as i64;
    }
    ((eax.wrapping_add(scratch) as i32) >> n) as i64
}

fn truncating(a: i64, b: i64) -> i64 {
    let q = a.abs() / b.abs();
    if (a < 0) == (b < 0) { q } else { -q }
}

#[test]
fn test_the_shift_sequence_truncates_towards_zero_like_idiv() {
    // The bias is the whole difference between a shift and idiv; getting it
    // wrong is off-by-one on every negative dividend.
    for n in 1..32 {
        for x in [0, 1, -1, 2, -2, 511, 512, 513, -511, -512, -513, (1 << 31) - 1, -(1 << 31), 123456789, -123456789] {
            let divisor = 1_i64 << n;
            assert_eq!(model(DIVIDE, x, n), truncating(x, divisor), "{x} {n}");
            assert_eq!(model(REMAINDER, x, n), x - truncating(x, divisor) * divisor, "{x} {n}");
        }
    }
}
