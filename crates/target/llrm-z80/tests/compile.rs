//! IR to sdasz80 text through allocation and expansion.

use llrm_z80::compile::{allocated, machine_function, text};
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

fn compiled(
    body: &str,
    name: &str,
) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    text(&ir, name, 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn dump() {
    for (name, ir) in [
        ("zero", "define i8 @zero() {\n  ret i8 0\n}"),
        ("add8", "define i8 @add8(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}"),
        ("add16", "define i16 @add16(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}"),
        (
            "max8",
            "define i8 @max8(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}",
        ),
    ] {
        let request = Request::new(Level::O2);
        let text_ir = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
        match machine_function(&text_ir, name, &request) {
            Ok(f) => {
                for b in &f.blocks {
                    eprintln!("{name} block {}: phis {:?}", b.id, b.phis);
                    for i in &b.insts {
                        eprintln!("   {:?} {:?}", i.kind, i.operands);
                    }
                }
            }
            Err(why) => eprintln!("{name}: machine function: {why}"),
        }
        if let Ok((f, a, _)) = allocated(&text_ir, name, &request) {
            for b in &f.blocks {
                eprintln!("{name} allocated block {}:", b.id);
                for i in &b.insts {
                    eprintln!("   {:?} {:?}", i.kind, i.operands);
                }
            }
            eprintln!("assignment {:?}", a.0.iter().map(|r| llrm_z80::REGISTER_INFO.name(*r)).collect::<Vec<_>>());
        }
        eprintln!(
            "{name}:\n{}",
            match text(&text_ir, name, 0, &request) {
                Ok(t) => t,
                Err(why) => format!("ERR {why}"),
            }
        );
        let _ = compiled;
    }
}

/// `ld b, c` was dropped as an identity because `b` and `c` share the root
/// `bc`; a select then returned the wrong byte.
#[test]
fn a_copy_between_the_two_bytes_of_a_pair_is_kept() {
    use llrm_spike_mf::regalloc::Assignment;
    use llrm_z80::compile::remove_identity_copies;
    use llrm_z80::mfunc::{Block, Inst, MFunction, Operand, Reg};
    use llrm_z80::regalloc::Class;
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8, Class::Gr8];
    let copy = Inst::copy(Reg::Virt(1), Reg::Virt(0));
    function.blocks.push(Block { id: 0, insts: vec![copy], phis: Vec::new() });
    remove_identity_copies(&mut function, &Assignment(vec![llrm_z80::reg::C, llrm_z80::reg::B]));
    assert_eq!(function.blocks[0].insts.len(), 1);
    remove_identity_copies(&mut function, &Assignment(vec![llrm_z80::reg::C, llrm_z80::reg::C]));
    assert!(function.blocks[0].insts.is_empty());
    let _ = Operand::Imm(0);
}

/// A byte taken from a pair was copied to a spare register first
/// (`ld d, c` / `ld (hl), d` / `cp a, d`): three bytes and 12 T-states a loop
/// that read `c` directly.
#[test]
fn a_byte_of_a_pair_is_read_in_place() {
    let ir = include_str!("kernels/ir/fill.ll");
    let out = text(ir, "fill", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ld (hl), c") && out.contains("cp a, c"), "{out}");
    assert!(!out.contains("ld d, c"), "{out}");
}

/// The loaded byte went to a spare pair and the running sum to `hl`, so the
/// sum was spilled to the frame every iteration: 48 bytes and 2519 T-states for
/// a loop that fits 21 bytes with no frame access.
#[test]
fn a_value_made_for_a_fixed_register_is_made_there() {
    let ir = include_str!("kernels/ir/sum16.ll");
    let out = text(ir, "sum16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("(ix)"), "{out}");
}

/// Both halves of `x << 1 ^ 0x1021` were computed before the test of the top
/// bit, kept in `bc`, and copied to the frame: crc16 took 122 bytes and 21373
/// T-states where the xor belongs on the one path that needs it.
#[test]
fn a_value_only_one_side_of_a_select_needs_is_made_on_that_side() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let (test, xor) =
        (out.find("bit 7, d").expect("tests the top bit"), out.find("xor a, #33").expect("xors the low byte"));
    assert!(test < xor, "{out}");
}

/// A spilled byte was reloaded into a register just to be compared
/// (`ld c, -1(ix)` / `cp a, c`): a byte more and 4 T-states than `cp a,
/// -1(ix)`.
#[test]
fn a_reloaded_operand_is_read_from_the_frame() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.lines().any(|line| line.trim().starts_with("cp a, ") && line.ends_with("(ix)")), "{out}");
}

/// A spilled byte stepped by one was reloaded, stepped and stored back
/// (`ld b, -1(ix)` / `inc b` / `ld -1(ix), b`): 7 bytes and 42 T-states for
/// `inc -1(ix)`'s 3 and 23.
#[test]
fn a_reloaded_byte_stepped_and_spilled_back_is_stepped_in_the_frame() {
    use llrm_spike_mf::regalloc::Assignment;
    use llrm_z80::expand::{Line, finish, reads};
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::regalloc::Class;
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8, Class::Gr8];
    let (loaded, stepped) = (Reg::Virt(0), Reg::Virt(1));
    let insts = vec![
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(loaded), Operand::Frame(0)]),
        Inst::new(Kind::Op("INC8"), vec![Operand::Def(stepped), Operand::Use(loaded)]),
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(0), Operand::Use(stepped)]),
        Inst::new(Kind::Op("RET"), vec![]),
    ];
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    let assignment = Assignment(vec![llrm_z80::reg::B, llrm_z80::reg::B]);
    let lines = finish(&function, &assignment, 0, &[(0, 1)], reads(&function)).unwrap_or_else(|why| panic!("{why}"));
    let insts: Vec<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(insts.iter().any(|text| text.starts_with("inc ") && text.ends_with("(ix)")), "{insts:?}");
    assert!(!insts.iter().any(|text| text.starts_with("ld b")), "{insts:?}");
}

/// The divisor 10 was kept in `de` across the call, so it was spilled to the
/// frame and loaded back for the compare (`ld -3(ix), e` / `ld c, -3(ix)`); a
/// constant is loaded again where it is wanted.
#[test]
fn a_constant_is_loaded_again_rather_than_spilled() {
    let ir = include_str!("kernels/ir/digits.ll");
    let out = text(ir, "digits", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld -3(ix)") && !out.contains("ld -2(ix)"), "{out}");
}

/// A word compared with a constant loaded the constant into a pair first
/// (`ld bc, #10` / `sub a, c` / `sbc a, b`): the bytes of the constant are
/// operands (`sub a, #10` / `sbc a, #0`).
#[test]
fn a_word_compared_with_a_constant_takes_its_bytes_as_operands() {
    let ir = include_str!("kernels/ir/digits.ll");
    let out = text(ir, "digits", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("sub a, #10") && out.contains("sbc a, #0"), "{out}");
}

/// `x ^ (b << 8)` moved the byte to the high half over a zero and xored both
/// halves (`ld h, l` / `ld l, #0` / `ld a, l` / `xor a, e`): the Pat for it
/// was dropped by tblgen without a word because its nested results were
/// untyped. The low byte is left alone and the high one xored.
#[test]
fn a_xor_with_a_byte_in_the_high_half_changes_only_the_high_half() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld l, #0") && !out.contains("xor a, e"), "{out}");
}

/// Two selects on one compare with a `sub` between them: the second `jr c`
/// read the carry of that `sub`, not of the compare, so gcd returned 4 where
/// the answer is 6. The compare is made again before the second reader.
#[test]
fn a_flag_reader_after_a_clobber_compares_again() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %x = select i1 %c, i8 %b, i8 0\n  %d = sub i8 %a, %x\n  %y = select i1 %c, i8 0, i8 %a\n  %e = sub i8 %b, %y\n  %r = xor i8 %d, %e\n  ret i8 %r\n}";
    let out = compiled(ir, "f");
    assert_eq!(out.lines().filter(|line| line.trim().starts_with("cp a,")).count(), 2, "{out}");
}

/// A byte loaded into `a` and stored through a pair other than `hl` went
/// through `hl` anyway (`ld a, (bc)` / `ex de, hl` / `ld (hl), a`): the check
/// for a value made in `a` read the pointer operand.
#[test]
fn a_byte_in_a_is_stored_through_any_pair() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
        define void @cp(ptr %a, ptr %b, ptr %c) {\n  %v = load i8, ptr %c\n  store i8 %v, ptr %b\n  %w = load i8, ptr %a\n  store i8 %w, ptr %c\n  ret void\n}\n";
    let out = text(ir, "cp", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ld (de), a") && out.contains("ld (bc), a") && !out.contains("ex de, hl"), "{out}");
}

/// `find` made `ld hl, #buf` on every trip of its loop: 415 T-states against
/// SDCC's 387. Made once before the loop it is 365.
#[test]
fn a_constant_added_in_a_loop_is_made_before_the_loop() {
    let ir = include_str!("kernels/ir/find.ll");
    let out = text(ir, "find", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let (made, looped) = (out.find("#_buf").expect("loads the address"), out.find("L0_1:").expect("has the loop"));
    assert!(made < looped && out.matches("#_buf").count() == 1, "{out}");
}

/// A byte loaded through `hl` and then compared or added was loaded into a
/// register first (`ld a, (hl)` / `cp a, c`): `find` was 18 B and `sum_arr` 18
/// B. `cp a, (hl)` and `add a, (hl)` read the memory themselves.
#[test]
fn a_byte_read_by_an_alu_op_is_read_from_memory() {
    let find = text(include_str!("kernels/ir/find.ll"), "find", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(find.contains("cp a, (hl)") && !find.contains("ld a, (hl)"), "{find}");
    let sum = text(include_str!("kernels/ir/sum_arr.ll"), "sum_arr", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(sum.contains("add a, (hl)"), "{sum}");
}

/// A loaded byte that is widened went to a byte register and was copied to the
/// pair's low half (`ld d, (hl)` / `ld e, d` / `ld d, #0`): scmp took a byte
/// and 4 T-states more per widened load.
#[test]
fn a_widened_load_goes_to_the_pairs_low_half() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(out.contains("ld e, (hl)\n\tld d, #0") && !out.contains("ld e, d"), "{out}");
}

/// `digits` kept its counter across the divide call in a frame slot (`ld
/// -1(ix), b` / `ld b, -1(ix)`) with 13 bytes of frame setup and teardown: 89 B
/// at -Oz against SDCC's 76. As `push bc` / `pop bc` it needs no frame.
#[test]
fn a_spill_slot_that_is_a_stack_is_push_and_pop() {
    let out = text(include_str!("kernels/ir/digits.ll"), "digits", 0, &Request::new(Level::Oz)).expect("compiles");
    assert!(
        out.contains("push bc") && out.contains("pop bc") && !out.contains("(ix)") && !out.contains("push ix"),
        "{out}"
    );
}
