//! IR to sdasz80 text through allocation and expansion.

use llrm_z80::compile::{allocated, lines, machine_function, text};
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
    let (test, xor) = (
        out.find("jr nc, ").expect("branches on the top bit (the shift's carry)"),
        out.find("xor a, #33").expect("xors the low byte"),
    );
    assert!(test < xor, "{out}");
}

/// The arguments a function keeps in its frame were stored with `ld -1(ix), a`
/// / `ld -4(ix), l` / `ld -3(ix), h` after a pair of `push af` (crc16): 9 B
/// more than the prologue's `push af` / `push hl`. Hand-built, so the
/// allocation of the kernel does not decide it.
#[test]
fn the_prologue_pushes_the_arguments_a_function_keeps_in_its_frame() {
    use llrm_spike_mf::regalloc::Assignment;
    use llrm_z80::expand::{Line, finish, reads};
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::regalloc::Class;
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr16, Class::Gr8, Class::Gr8];
    let insts = vec![
        Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(0), Operand::Use(Reg::Virt(0))]),
        Inst::new(Kind::Op("SPILL8"), vec![Operand::Frame(1), Operand::Use(Reg::Virt(1))]),
        Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Virt(2)), Operand::Frame(1)]),
        Inst::new(Kind::Op("RET"), vec![]),
    ];
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    let assignment = Assignment(vec![llrm_z80::reg::HL, llrm_z80::reg::A, llrm_z80::reg::B]);
    let lines =
        finish(&function, &assignment, 0, &[(0, 2), (1, 1)], reads(&function)).unwrap_or_else(|why| panic!("{why}"));
    let insts: Vec<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(insts.windows(2).any(|pair| pair == ["push af", "push hl"]), "{insts:?}");
    assert!(!insts.iter().any(|line| line.starts_with("ld -") && line.contains("(ix), ")), "{insts:?}");
}

/// crc16 reloaded its pointer from the slot at SP with `ld c, -3(ix)` / `ld b,
/// -2(ix)`, 6 B and 38 T-states on each of its trips; `pop bc` / `push bc` is 2
/// B and 21.
#[test]
fn the_slot_at_sp_is_reloaded_by_pop_and_push() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    assert!(lines.windows(2).any(|pair| pair == ["pop bc", "push bc"]), "{out}");
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

/// With `has_cmov` left true, gcd's `if (a > b) a -= b; else b -= a;` was
/// turned into a select before the branch and came out 30 B / 462 T at -O2
/// (SDCC 21 B / 295 T). A select is a branch on the Z80: 22 B / 285 T.
#[test]
fn a_select_is_a_branch_and_gcd_is_the_branch_loop() {
    let ir = include_str!("kernels/ir/gcd.ll");
    let out = text(ir, "gcd", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let instructions = out.lines().filter(|l| l.starts_with('\t') && !l.trim_start().starts_with('.')).count();
    assert!(instructions <= 19, "{instructions} instructions\n{out}");
}

/// `slen`'s pointer loop is `strlen` (LoopIdiomRecognize, once the compile
/// states the routine) and the runtime's `cpir` routine, expanded at its sole
/// call, replaces it: a counter loop was 10 B / 482 T-states, this is 12 B /
/// 340 against SDCC's 13 / 437.
#[test]
fn a_loop_over_a_string_is_the_runtimes_strlen() {
    let out = text(include_str!("kernels/ir/slen.ll"), "slen", 0, &Request::new(Level::O2))
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("jp __strlen") && !out.contains("jr"), "{out}");
}

/// A pair copy moved both halves: crc16's `bit 7, h` on a copy of `de` came
/// with `ld l, e` for a `l` nothing reads, a byte and 4 T-states on each of 8
/// trips. A half that is dead after the copy is not moved (`PhysLiveness`).
#[test]
fn a_pair_copy_moves_only_the_halves_that_are_read() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld l, e") && !out.contains("bit 7, h"), "{out}");
}

/// crc16: the pointer and the length fit in `hl` and `a` through the loops,
/// but the allocator gave a short-lived byte to `h`, evicted the pointer and
/// spilled both to the frame: 77 B against SDCC's 59. Spill-free needs no
/// frame access. Waits for greedy's eviction order (greedy-anyisa, 11-08).
#[test]
#[ignore = "greedy evicts the long-lived pair for a short byte that has a free register"]
fn a_function_whose_values_fit_in_registers_makes_no_frame_access() {
    let out = text(include_str!("kernels/ir/crc16.ll"), "crc16", 0, &Request::new(Level::Oz))
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("(ix)"), "{out}");
}

/// bsearch8: the base pointer and a bound spill (`ld l, -2(ix)`, `ld -3(ix),
/// d`) in a loop with registers to spare: 68 B / 790 T against SDCC's 56 / 702.
#[test]
#[ignore = "greedy spills pair values in a loop with registers to spare"]
fn a_search_loop_keeps_its_base_pointer_in_a_register() {
    let out = text(include_str!("kernels/ir/bsearch8.ll"), "bsearch8", 0, &Request::new(Level::O2))
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("(ix)"), "{out}");
}

/// scmp: two pointers read through `(hl)` are copied into `hl` on every trip
/// (`ld l, e; ld h, d`, `ld l, c; ld h, b`): 41 B / 218 T against SDCC's
/// 24 / 147. A pointer that is only ever read through `(hl)` lives in `hl`.
#[test]
fn a_pointer_read_through_hl_in_a_loop_is_not_copied_into_hl_each_trip() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2))
        .unwrap_or_else(|why| panic!("{why}"));
    let start = out.find("L0_2:").expect("the loop label");
    let end = out[start..].find("jr nz, L0_2").expect("the back edge") + start;
    assert!(!out[start..end].contains("ld l,"), "{out}");
}

/// scmp copied `bc` into `hl` (`ld l, c; ld h, b`) on every trip only to
/// `ld a, (hl)`: 2 B and 8 T-states a trip that `ld a, (bc)` does not take. A
/// pointer copied into `hl` just to load into or store from `a`, with `hl`
/// dead after, is read through its own pair.
#[test]
fn a_pointer_copied_into_hl_only_to_load_a_is_read_through_its_own_pair() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2))
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ld a, (bc)"), "{out}");
}

/// The carry out of a wide add is `select(sum == b, carry_in, sum < b)`: one
/// 16-bit compare read for its zero flag and its carry. It was a borrow chain
/// (`sub a, e; sbc a, d`), whose zero flag is that of the high byte alone, and
/// the byte made from the carry used `dec`, which sets it again: an `i64` add
/// returned 0 in the high word where 4 was right. Both flags are the compare's.
#[test]
fn a_compare_read_for_equal_and_for_below_sets_both_flags_of_the_word() {
    let ir = "define i8 @f(i16 %s, i16 %b, i8 %cin) {\n\
              \x20 %e = icmp eq i16 %s, %b\n  %u = icmp ult i16 %s, %b\n  %z = zext i1 %u to i8\n\
              \x20 %r = select i1 %e, i8 %cin, i8 %z\n  ret i8 %r\n}\n";
    let out = compiled(ir, "f");
    assert!(out.matches("cp a, ").count() == 2 && !out.contains("sbc a,"), "{out}");
}

/// `reverse` at -Oz compared `a` with the counter without ever loading `a`
/// from it: the byte of the counter pair was given `a` itself, its copy into
/// `a` was dropped as the identity, and the pass that reads a byte from its
/// pair's half saw no use of the byte and deleted the `LO` that made it. The
/// compare read `a` implicitly. The loop exited at once and returned 3, not 6.
#[test]
fn a_byte_made_in_a_register_that_a_later_instruction_reads_is_still_made() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr16, Class::Gr8];
    function
        .blocks
        .push(
            Block {
                id: 0,
                insts: vec![
                    Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(0)), Operand::Imm(0)]),
                    Inst::new(Kind::Op("LO"), vec![Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(0))]),
                    Inst::new(
                        Kind::Op("CP8r"),
                        vec![
                            Operand::Def(Reg::Phys(reg::FLAGS)),
                            Operand::Use(Reg::Phys(reg::A)),
                            Operand::Use(Reg::Phys(reg::B)),
                        ],
                    ),
                ],
                phis: Vec::new(),
            },
        );
    // v0 in hl, its low byte given a
    let assignment = Assignment(vec![reg::HL, reg::A]);
    llrm_z80::compile::use_lanes_in_place(&mut function, &assignment);
    assert!(function.blocks[0].insts.iter().any(|inst| inst.kind == Kind::Op("LO")), "{:?}", function.blocks[0].insts);
}

/// A copy back into the register another copy read from stayed: gcd's argument
/// went `ld b, a` / `ld a, b` before its compare, one instruction more than
/// the entry copy joined into `a`. `ld a, c` after `ld b, a` is no copy back:
/// `b` and `c` are lanes of one pair, not one register.
#[test]
fn a_copy_back_into_the_register_just_copied_from_is_dropped_between_bytes_only() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8; 3];
    function.blocks.push(Block { id: 0, insts: vec![copy(1, 0), copy(0, 1), copy(0, 2)], phis: Vec::new() });
    // v0 a, v1 b, v2 c: the second copy is a copy back, the third is not
    let assignment = Assignment(vec![reg::A, reg::B, reg::C]);
    llrm_z80::compile::remove_copies_back(&mut function, &assignment);
    assert_eq!(function.blocks[0].insts.len(), 2, "{:?}", function.blocks[0].insts);
}

/// fletcher -Oz never finished compiling: two copies that touch neither
/// register each other's, with nothing between them and the branch, took turns
/// moving to the end of the block for ever. Run on a thread, so a hang fails.
#[test]
fn copies_ahead_of_a_branch_with_nothing_between_do_not_trade_places_for_ever() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let jump = Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("JP"), vec![Operand::Block(0)]) };
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8; 4];
    function.blocks.push(Block { id: 0, insts: vec![copy(1, 0), copy(3, 2), jump], phis: Vec::new() });
    let assignment = Assignment(vec![reg::A, reg::B, reg::C, reg::D]);
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        llrm_z80::compile::sink_copies_to_the_branch(&mut function, &assignment);
        let _ = done.send(());
    });
    assert!(
        finished.recv_timeout(std::time::Duration::from_secs(20)).is_ok(),
        "sink_copies_to_the_branch did not return"
    );
}

/// xs8's `x << 3` was `sla a` x3 (2 B / 8 T a bit) and its `x >> 5` `srl a` x5:
/// 35 B / 737 T against SDCC's 26 B / 546 T. A byte shifted left in `a` is
/// `add a, a`; a logical right shift by two or more, with the flags dead, is
/// `rlca`/`rrca` the short way round and an `and` of the surviving bits.
#[test]
fn a_byte_shifted_in_a_is_doublings_and_a_rotate_with_a_mask() {
    let ir = include_str!("kernels/ir/xs8.ll");
    let out = text(ir, "xs8", 0, &Request::new(Level::Oz)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.matches("add a, a").count() >= 4 && out.matches("rlca").count() == 3, "{out}");
    assert!(out.contains("and a, #7") && !out.contains("sla a") && !out.contains("srl a"), "{out}");
}

/// minmax's `lo`, merged by a branch, was copied in ahead of the compare that
/// decides the branch, so it lived beside the value it replaces and needed a
/// register of its own: both extremes spilled to the frame, 66 B / 2675 T
/// against SDCC's 56 B / 2351 T. Sunk below the compare, `lo` and `hi` stay in
/// registers.
#[test]
fn a_merged_value_is_not_copied_in_ahead_of_the_compare_that_decides_it() {
    let ir = include_str!("kernels/ir/minmax.ll");
    for level in [Level::O2, Level::Oz] {
        let out = text(ir, "minmax", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(!out.contains("(ix)"), "{level:?}\n{out}");
    }
}

/// fletcher -Oz made its zero in the loop (`ld e, #0; ld c, e`, 11 T a trip)
/// where the other allocation of the argument copies keeps it in a register
/// of its own: 1811 T against 1706. The cheaper by the loop's weighted clocks
/// stays.
#[test]
fn the_allocation_of_a_loop_with_the_cheaper_weighted_clocks_is_kept() {
    let ir = include_str!("kernels/ir/fletcher.ll");
    let out = text(ir, "fletcher", 0, &Request::new(Level::Oz)).unwrap_or_else(|why| panic!("{why}"));
    let (zero, head) = (out.find("ld e, #0"), out.find("L0_1:"));
    assert!(zero.is_some() && zero < head, "{out}");
}

/// popcount -Oz copied its loop value into `a` at the loop head and again at
/// the head of the body the head alone enters: `ld a, c` twice, 1 B and 4 T a
/// trip. A block two blocks enter starts with nothing known.
#[test]
fn a_copy_repeated_in_the_only_block_that_follows_is_dropped() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy = || Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(0)), Operand::Use(Reg::Virt(1))]);
    let jump =
        |to: u32| Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("JP"), vec![Operand::Block(to)]) };
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8; 2];
    let block = |id, insts| Block { id, insts, phis: Vec::new() };
    // 0 enters 1, which has no other entrance; 1 and 2 enter 3
    function.blocks.push(block(0, vec![copy(), jump(1)]));
    function.blocks.push(block(1, vec![copy(), jump(3)]));
    function.blocks.push(block(2, vec![copy(), jump(3)]));
    function.blocks.push(block(3, vec![copy(), jump(3)]));
    let assignment = Assignment(vec![reg::A, reg::C]);
    llrm_z80::compile::remove_copies_back(&mut function, &assignment);
    let copies: Vec<usize> =
        function.blocks.iter().map(|b| b.insts.iter().filter(|i| i.kind == Kind::Copy).count()).collect();
    assert_eq!(copies, [1, 0, 1, 1], "{:?}", function.blocks);
}

/// sum_arr kept `ld b, a` ahead of `or a, a`. The `ld a, #0` both successors
/// begin with could not be hoisted past a compare that reads `a`, so the
/// function lost its `ret z`: 16 B, not 14. A copy the compare reads stays.
#[test]
fn a_copy_nothing_after_it_touches_moves_down_to_the_branch() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let compare = |of: u32| Inst {
        sets_flags: true,
        ..Inst::new(
            Kind::Op("CP8i"),
            vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(Reg::Virt(of)), Operand::Imm(0)],
        )
    };
    let branch = Inst {
        terminator: true,
        reads_flags: true,
        ..Inst::new(Kind::Op("JPNZ"), vec![Operand::Block(0), Operand::Use(Reg::Phys(reg::FLAGS))])
    };
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8; 3];
    // v0 a, v1 b, v2 c: `ld b, a` can follow the compare of a; `ld c, b` cannot
    // follow the compare of c
    function.blocks.push(Block { id: 0, insts: vec![copy(1, 0), compare(0), branch.clone()], phis: Vec::new() });
    function.blocks.push(Block { id: 1, insts: vec![copy(2, 1), compare(2), branch.clone()], phis: Vec::new() });
    let assignment = Assignment(vec![reg::A, reg::B, reg::C]);
    llrm_z80::compile::tidy_copies(&mut function, &assignment);
    // v3 is a again: the copy back into a is dropped first, then the copy ahead
    // of it follows the compare
    let mut back = MFunction::new("g");
    back.classes = vec![Class::Gr8; 4];
    back.blocks.push(Block { id: 0, insts: vec![copy(1, 0), copy(3, 1), compare(3), branch], phis: Vec::new() });
    llrm_z80::compile::tidy_copies(&mut back, &Assignment(vec![reg::A, reg::B, reg::C, reg::A]));
    function.blocks.extend(back.blocks);
    let kinds: Vec<Vec<Kind>> =
        function.blocks.iter().map(|b| b.insts.iter().map(|inst| inst.kind).collect()).collect();
    let (cp, jp) = (Kind::Op("CP8i"), Kind::Op("JPNZ"));
    assert_eq!(kinds, [vec![cp, Kind::Copy, jp], vec![Kind::Copy, cp, jp], vec![cp, Kind::Copy, jp]]);
}

/// `icmp ult i64 %a, 4294967297` leaves a word compare whose flags nothing
/// reads (the next compare sets them again). It was refused, "CP16 read by
/// None", which refused every 64-bit divide by a constant in the C corpus.
#[test]
fn a_word_compare_nothing_reads_is_not_emitted() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              @g = external global i64\n\
              define i1 @f() {\n  %a = load i64, ptr @g\n  %c = icmp ult i64 %a, 4294967297\n  ret i1 %c\n}\n";
    let out = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ret"), "{out}");
}

/// `c ^= p[i] << 8` changes the high byte of a pair and keeps the low one. The
/// high byte's copy had been given `l`, the half the low byte is kept in, so
/// the low byte was never read in place: `ld c, l` ... `ld l, c` stayed, two
/// instructions of crc16's outer loop. Once the high byte is read in place the
/// low byte can be too.
#[test]
fn a_pair_with_its_high_byte_replaced_keeps_its_low_byte_where_it_is() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr16, Class::Gr8, Class::Gr8, Class::Gr16];
    let reg_of = Reg::Virt;
    function.blocks.push(Block {
        id: 0,
        insts: vec![
            Inst::new(Kind::Op("LO"), vec![Operand::Def(reg_of(1)), Operand::Use(reg_of(0))]),
            Inst::new(Kind::Op("HI"), vec![Operand::Def(reg_of(2)), Operand::Use(reg_of(0))]),
            Inst {
                sets_flags: true,
                ..Inst::new(
                    Kind::Op("XOR8r"),
                    vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(Reg::Phys(reg::A)), Operand::Use(reg_of(2))],
                )
            },
            Inst::new(
                Kind::Op("PACK"),
                vec![Operand::Def(reg_of(3)), Operand::Use(reg_of(1)), Operand::Use(Reg::Phys(reg::A))],
            ),
        ],
        phis: Vec::new(),
    });
    // v0 and v3 are hl; the low byte v1 got c, the high byte v2 got l
    let assignment = Assignment(vec![reg::HL, reg::C, reg::L, reg::HL]);
    llrm_z80::compile::use_lanes_in_place(&mut function, &assignment);
    let kinds: Vec<_> = function.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds, [Kind::Op("XOR8r"), Kind::Op("PACK")], "{kinds:?}");
}

/// `*a - *b` of two bytes widened to int went `ld l, a; ld h, #0; ld c, d;
/// ld b, #0; or a, a; sbc hl, bc; ex de, hl`: 11 B and 45 T for what is a byte
/// subtraction and its borrow (`sub a, d; ld e, a; sbc a, a; ld d, a`). scmp
/// lost to SDCC on bytes, clocks and instructions with it.
#[test]
fn the_difference_of_two_zero_extended_bytes_is_a_byte_subtraction_and_its_borrow() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i16 @f(i8 %a, i8 %b) {\n  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  %d = sub i16 %x, %y\n  ret i16 %d\n}\n";
    let out = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("sbc a, a") && !out.contains("sbc hl"), "{out}");
}

/// `c & 0x8000 ? .. : ..` beside `c << 1` tested the sign of a copy of the high
/// byte after the shift (`ld h, d` ... `bit 7, h`): the carry of the shift has
/// that bit. 3 B and 12 T on each of crc16's 8 trips. A test of some other
/// value stays.
#[test]
fn the_sign_tested_beside_a_shift_left_is_the_shifts_carry() {
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::reg;
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let shift = |v: u32| Inst {
        sets_flags: true,
        ..Inst::new(Kind::Op("SHL16"), vec![Operand::Def(Reg::Virt(v)), Operand::Use(Reg::Virt(v)), Operand::Imm(1)])
    };
    let bit = |of: u32| Inst {
        sets_flags: true,
        ..Inst::new(Kind::Op("BIT7_16"), vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(Reg::Virt(of))])
    };
    let branch = Inst {
        terminator: true,
        reads_flags: true,
        ..Inst::new(Kind::Op("JPNZ"), vec![Operand::Block(0), Operand::Use(Reg::Phys(reg::FLAGS))])
    };
    let kinds = |function: &MFunction| -> Vec<Kind> { function.blocks[0].insts.iter().map(|inst| inst.kind).collect() };
    let build = |tested: u32| {
        let mut function = MFunction::new("f");
        function.classes = vec![llrm_z80::regalloc::Class::Gr16; 3];
        function
            .blocks
            .push(
                Block {
                    id: 0,
                    insts: vec![copy(1, 0), shift(1), copy(2, 1), bit(tested), branch.clone()],
                    phis: Vec::new(),
                },
            );
        function
    };
    let mut same = build(0);
    llrm_z80::compile::use_the_shift_carry(&mut same);
    assert_eq!(kinds(&same), [Kind::Copy, Kind::Op("SHL16"), Kind::Copy, Kind::Op("JPC")], "{:?}", kinds(&same));
    let mut other = build(2);
    llrm_z80::compile::use_the_shift_carry(&mut other);
    assert_eq!(kinds(&other), [Kind::Copy, Kind::Op("SHL16"), Kind::Copy, Kind::Op("BIT7_16"), Kind::Op("JPNZ")]);
}

/// sum_arr -Oz kept `ld a, b` at the head of a loop, where the entry
/// has just made it: it ran on every trip and once more at the exit, +4 T.
/// Moved into the back edge, the only entrance without it, it runs once a trip.
/// A join with two entrances lacking it keeps it (the code would grow).
#[test]
fn a_join_s_copy_moves_into_the_one_entrance_that_lacks_it() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let jump =
        |to: u32| Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("JP"), vec![Operand::Block(to)]) };
    let write = || {
        Inst::new(
            Kind::Op("ADD8i"),
            vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(Reg::Phys(reg::A)), Operand::Imm(1)],
        )
    };
    let leave = Inst {
        terminator: true,
        reads_flags: true,
        ..Inst::new(Kind::Op("JPZ"), vec![Operand::Block(3), Operand::Use(Reg::Phys(reg::FLAGS))])
    };
    let block = |id, insts| Block { id, insts, phis: Vec::new() };
    let build = |back: Vec<Inst>| {
        let mut function = MFunction::new("f");
        function.classes = vec![Class::Gr8; 2];
        function.blocks.push(block(0, vec![copy(1, 0)]));
        function.blocks.push(block(1, vec![copy(0, 1), write(), leave.clone()]));
        function.blocks.push(block(2, back));
        function
            .blocks
            .push(block(3, vec![Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("RET"), vec![]) }]));
        function
    };
    // v0 a, v1 b
    let assignment = Assignment(vec![reg::A, reg::B]);
    let mut function = build(vec![write(), jump(1)]);
    llrm_z80::compile::place_copies_where_needed(&mut function, &assignment);
    let counts: Vec<usize> =
        function.blocks.iter().map(|b| b.insts.iter().filter(|i| i.kind == Kind::Copy).count()).collect();
    assert_eq!(counts, [1, 0, 1, 0], "{:?}", function.blocks);
    // a second block also enters the head without the copy: it stays
    let mut two = build(vec![write(), jump(1)]);
    two.blocks.push(block(4, vec![write(), jump(1)]));
    llrm_z80::compile::place_copies_where_needed(&mut two, &assignment);
    let counts: Vec<usize> =
        two.blocks.iter().map(|b| b.insts.iter().filter(|i| i.kind == Kind::Copy).count()).collect();
    assert_eq!(counts, [1, 1, 0, 0, 0], "{:?}", two.blocks);
}

/// gcd's `ld a, b` after the diamond's join ran on both sides, though one
/// side (`ld b, a`) had just made it: the other side ends with the copy, and
/// the join starts without it. The block that falls into the join and ends in
/// nothing that reads either register takes it.
#[test]
fn a_diamonds_join_copy_moves_into_the_side_that_lacks_it() {
    use llrm_spike_mf::Assignment;
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::{reg, regalloc::Class};
    let copy =
        |to: u32, from: u32| Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]);
    let jump =
        |to: u32| Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("JP"), vec![Operand::Block(to)]) };
    let write = Inst::new(
        Kind::Op("ADD8i"),
        vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(Reg::Phys(reg::A)), Operand::Imm(1)],
    );
    let branch = Inst {
        terminator: true,
        reads_flags: true,
        ..Inst::new(Kind::Op("JPNC"), vec![Operand::Block(2), Operand::Use(Reg::Phys(reg::FLAGS))])
    };
    let block = |id, insts| Block { id, insts, phis: Vec::new() };
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8; 2];
    function.blocks.push(block(0, vec![branch]));
    function.blocks.push(block(1, vec![copy(1, 0), jump(3)]));
    function.blocks.push(block(2, vec![write]));
    function
        .blocks
        .push(
            block(
                3,
                vec![copy(0, 1), Inst { terminator: true, barrier: true, ..Inst::new(Kind::Op("RET"), vec![]) }],
            ),
        );
    // v0 a, v1 b
    let assignment = Assignment(vec![reg::A, reg::B]);
    llrm_z80::compile::place_copies_where_needed(&mut function, &assignment);
    let counts: Vec<usize> =
        function.blocks.iter().map(|b| b.insts.iter().filter(|i| i.kind == Kind::Copy).count()).collect();
    assert_eq!(counts, [0, 1, 1, 0], "{:?}", function.blocks);
}

/// A `long long` compared and returned held its four argument registers for
/// the whole of its range, and the allocator found no room to split it:
/// "greedy could not allocate", the function refused. It is allocated again
/// with the argument copies left unjoined.
#[test]
fn a_long_long_in_the_argument_registers_is_allocated() {
    let out = compiled(
        "define i64 @f(i64 %a) {\n  %c = icmp ult i64 %a, 4294967297\n  %z = zext i1 %c to i64\n  ret i64 %z\n}",
        "f",
    );
    assert!(out.contains("ret"), "{out}");
}

/// digits moved the quotient from `de` to `hl` for the next trip with
/// `ld l, e` / `ld h, d` at the end of a block: the copy's source was dead (the
/// loop head loads `de` first), but the block ended before anything said so.
/// `ex de, hl` is one byte for two.
#[test]
fn a_pair_copy_at_a_blocks_end_is_an_exchange_when_the_source_is_dead_past_it() {
    let ir = include_str!("kernels/ir/digits.ll");
    let out = text(ir, "digits", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ex de, hl") && !out.contains("ld l, e"), "{out}");
}

/// `&str[6]` of an external global was `ld hl, #_str` / `ld bc, #6` / `add hl,
/// bc`: the offset folds into the symbol only for a symbol that is `dso_local`,
/// and clang's IR for a static program leaves the flag off an `extern` array
/// the program only declares. Addressing is absolute here, so every symbol
/// folds: `ld de, #_str+6`. The scmp driver paid 4 instructions for it.
#[test]
fn a_constant_offset_from_an_external_global_is_folded_into_the_symbol() {
    let out = compiled(
        "@str = external global i8\ndefine ptr @f() {\n  %p = getelementptr i8, ptr @str, i16 6\n  ret ptr %p\n}",
        "f",
    );
    assert!(out.contains("#_str+6") && !out.contains("add hl"), "{out}");
}

/// At -Oz slen was the 10 B loop: 482 T against best SDCC's 437 T and 58
/// instructions (65). The pin forms the `strlen` call at every level: 12 B,
/// 340 T, 25 instructions, ahead of SDCC on all three.
#[test]
fn a_string_loop_is_the_runtime_strlen_at_every_level() {
    let ir = include_str!("kernels/ir/slen.ll");
    for level in [Level::O2, Level::Os, Level::Oz] {
        let out = text(ir, "slen", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(out.contains("_strlen"), "{level:?}\n{out}");
    }
}

/// A branch to a block that is only a `ret` prints as `ret z`; the block was
/// printed anyway, a dead `ret` behind the live one: gcd -O2 was 20 B with
/// two, 19 B with one.
#[test]
fn a_return_block_only_conditional_returns_reach_is_not_printed() {
    let ir = include_str!("kernels/ir/gcd.ll");
    let out = text(ir, "gcd", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let rets = out.lines().filter(|line| line.trim() == "ret").count();
    assert_eq!(rets, 1, "{out}");
    assert!(out.contains("ret z"), "{out}");
}

/// lz4's `tmp == 255` was widened to a word compare of the zero-extended load
/// (`ExtendSetCCUses`, as the pin does where a word compare costs the same as a
/// byte one): `ld a, l; sub a, #255; jr nz; ld a, h; sub a, #0`, and the extra
/// pressure spilled more. The compare stays on the byte: `cp a, #255`.
#[test]
fn a_byte_compared_with_a_constant_is_not_widened_to_a_word_compare() {
    let ir = include_str!("kernels/ir/lz4.ll");
    let out = text(ir, "lz4", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("cp a, #255") && !out.contains("sub a, #255"), "{out}");
}

/// fletcher -O2 made its zero in the loop (`ld e, #0` then `ld c, e`, every
/// trip) once the plain allocation was chosen over the hoisted one by static
/// size alone: 35 B / 1456 T became 35 B / 1561 T. The two are chosen by the
/// loop's weighted clocks.
#[test]
fn a_zero_a_loop_needs_is_made_before_the_loop_when_that_is_faster() {
    let ir = include_str!("kernels/ir/fletcher.ll");
    let out = text(ir, "fletcher", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let body = out.split("L0_2:").nth(1).and_then(|rest| rest.split("L0_12:").next()).expect("the loop");
    assert!(!body.contains("#0"), "{out}");
}

/// minmax -Oz loaded a byte into `a` that its max compare also reads from a
/// register, so the byte was copied out of `a` again and the pointer lost its
/// pair: spilled and reloaded by `pop de; push de` every trip, 60 B / 2938 T
/// became 64 B / 3557 T. A byte a non-copy reads as a register is not loaded
/// through `a`.
#[test]
fn a_byte_also_read_as_a_register_is_not_loaded_through_a() {
    let ir = include_str!("kernels/ir/minmax.ll");
    let out = text(ir, "minmax", 0, &Request::new(Level::Oz)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("pop de") && out.contains("ld d, (hl)"), "{out}");
}

/// A byte wanted in `a` and in a register of its own was loaded through `hl`
/// by a fixed rule, which lz4c -O2 paid for in pressure on `hl`: 443 B against
/// 428 B when it is loaded through its pair. Both are allocated and the
/// cheaper by weighted clocks stays.
#[test]
fn a_byte_wanted_in_a_and_in_a_register_is_loaded_the_cheaper_way() {
    let ir = include_str!("kernels/ir/lz4c.ll");
    let out = lines(ir, "lz4c", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let (bytes, _) = llrm_z80::expand::price(&out);
    assert!(bytes <= 435, "{bytes} B");
}

/// lz4's copy loops made `p + 1` before the store through `p` (IR order), so
/// the source and destination pointers, their increments and the count were
/// live together: five pairs on three, the destination reloaded from the frame
/// every trip. The pressure-aware order the pin gives a target without a
/// machine scheduler stores first: the loop-weighted clocks of lz4 -O2 fell
/// from 548952 to 245094 (170569 T to 103942 T in the simulator).
#[test]
fn the_copy_loops_store_through_a_pointer_before_stepping_it() {
    let ir = include_str!("kernels/ir/lz4.ll");
    let out = lines(ir, "lz4", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let (_, clocks) = llrm_z80::expand::price_in_loops(&out);
    assert!(clocks < 300_000, "{clocks}");
}

/// `h * 33` (InstCombine folds `(h << 5) + h` into it) was a `__mulhi3` call:
/// hash16 89 B / 6644 T against SDCC's 40 B / 1686 T. A constant `2^n +- 1` is
/// shifts and an add, as the pin's `decomposeMulByConstant` makes it.
#[test]
fn a_multiply_by_a_constant_one_off_a_power_of_two_is_shifts_and_an_add() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i16 @f(i16 %x) {\n  %r = mul i16 %x, 33\n  ret i16 %r\n}\n";
    let out = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("mulhi3"), "{out}");
}

/// A word shifted left by a constant in `hl` was `sla l; rl h` a bit (4 B / 16
/// T) where `add hl, hl` is 1 B / 11 T; hash16's `<< 5` paid 10 instructions.
#[test]
fn a_word_shifted_left_in_hl_is_doublings() {
    let ir = include_str!("kernels/ir/hash16.ll");
    let out = text(ir, "hash16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.matches("add hl, hl").count() >= 5 && !out.contains("rl h"), "{out}");
}

/// A word returned in `de` and stored to a global went through `hl`:
/// `ex de, hl; ld (_r), hl`, two instructions where `ld (_r), de` is one of
/// the same 4 B / 20 T. Every add16-shaped row paid an instruction for it.
#[test]
fn a_word_in_de_is_stored_to_a_global_without_moving_it() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              @r = external global i16\n\
              declare i16 @g()\n\
              define void @f() {\n  %v = call i16 @g()\n  store volatile i16 %v, ptr @r\n  ret void\n}\n";
    let out = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ld (_r), de") && !out.contains("ex de, hl"), "{out}");
}

/// The carry out of a 32-bit add is a compare used as a value (`SETCC8` after
/// `CP16`). It was refused with "CP16 read by SETCC8": the compare did not know
/// the byte reader and nothing expanded it. 30 functions of the C corpus beyond
/// the 23 rows hit it.
#[test]
fn a_comparison_used_as_a_value_is_a_byte_that_is_one_or_zero() {
    let ir = include_str!("kernels/ir/carry32.ll");
    let out = text(ir, "carry32", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("sbc a, ") && out.contains("dec "), "{out}");
}

/// A call with more arguments than registers was refused: "stack arguments are
/// not expanded yet". 8 functions of the C corpus beyond the 23 rows hit it.
/// The caller reserves the area, stores through IX and pops it after the call.
#[test]
fn a_call_with_stack_arguments_reserves_and_frees_the_area() {
    let ir = include_str!("kernels/ir/stackargs.ll");
    let out = text(ir, "stackargs", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("push bc") && out.contains("pop bc") && out.contains("(ix), "), "{out}");
}

/// A frame object out of the byte displacement of IX was refused: "frame object
/// N is -306 from ix, out of the byte displacement". 7 functions of the C
/// corpus beyond the 23 rows hit it. IY, when nothing holds it, is the base.
#[test]
fn a_frame_object_out_of_ixs_reach_is_addressed_through_iy() {
    let ir = include_str!("kernels/ir/bigframe.ll");
    let out = text(ir, "bigframe", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("(iy)") && out.contains("add iy, sp"), "{out}");
}

/// A call through a pointer was refused: `CannotSelect(Target(CALL))`, nothing
/// selects a call whose callee is a register. 4 functions of the C corpus
/// beyond the 23 rows hit it. The address goes in iy and the call is the
/// runtime's `jp (iy)`, as SDCC's `___sdcc_call_iy`.
#[test]
fn a_call_through_a_pointer_puts_the_address_in_iy() {
    let ir = include_str!("kernels/ir/indirect.ll");
    let out = text(ir, "indirect", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("pop iy") && out.contains("call __call_iy"), "{out}");
}

/// A word compared with zero for equality took a `sub a, #0` ladder: 6
/// instructions and two branches for `dec hl; jr nz`. The two bytes or-ed set Z
/// when both are zero: `ld a, h; or a, l`.
#[test]
fn a_word_tested_for_zero_is_its_two_bytes_or_ed() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              @r = external global i8\n\
              define void @count16(i16 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i16 [ %n, %entry ], [ %i1, %loop ]\n  store volatile i8 1, ptr @r\n  %i1 = add i16 %i, -1\n  %c = icmp eq i16 %i1, 0\n  br i1 %c, label %done, label %loop\ndone:\n  ret void\n}\n";
    let out = text(ir, "count16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("or a, ") && !out.contains("sub a, #0"), "{out}");
}

/// A `freeze` is selected to a machine `COPY`, which the emitter had no arm
/// for: "a machine node with no name: 20". 2 functions of the C corpus beyond
/// the 23 rows hit it.
#[test]
fn a_freeze_is_a_copy() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i16 @f(i16 %x, i8 %b) {\n  %c = icmp eq i8 %b, 0\n  %n = sub nsw i16 0, %x\n  %s = select i1 %c, i16 %x, i16 %n\n  %f = freeze i16 %s\n  %d = sdiv i16 %f, 3\n  %m = mul i16 %d, 3\n  %r = sub i16 %f, %m\n  ret i16 %r\n}\n";
    text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
}

/// A load of an `i1` was refused (`CannotSelect`): the bit is a byte in
/// memory, 0 or 1.
#[test]
fn a_bit_is_loaded_as_a_byte() {
    let text = compiled(
        "@flag = global i1 false\ndefine i8 @f() {\n  %v = load i1, ptr @flag\n  %w = zext i1 %v to i8\n  ret i8 %w\n}",
        "f",
    );
    assert!(text.contains("ld a, (_flag)"), "{text}");
}

/// The store of an `i1` writes the byte 0 or 1.
#[test]
fn a_bit_is_stored_as_a_byte_of_zero_or_one() {
    let text = compiled(
        "@flag = global i1 false\ndefine void @f(i8 %x) {\n  %c = icmp ne i8 %x, 0\n  store i1 %c, ptr @flag\n  ret void\n}",
        "f",
    );
    assert!(text.contains("(_flag)"), "{text}");
}

/// `huffman_decode`'s `static int bad` is an `i1` global that a branch, a
/// `select` and a `zext` all read. Selected as it was, the `select` became a
/// conditional move on a compare of its own result: a cycle in the DAG that
/// `calc_sethi_ullman` walked until the memory ran out (no end in 5 minutes).
#[test]
fn a_loaded_bit_read_by_a_branch_a_select_and_a_zext_compiles() {
    let text = compiled(
        "@bad = global i1 false\ndefine i8 @f(i8 %x) {\n  %v = load i1, ptr @bad\n  br i1 %v, label %t, label %e\nt:\n  ret i8 7\ne:\n  %s = select i1 %v, i8 %x, i8 1\n  %z = zext i1 %v to i8\n  %r = add i8 %s, %z\n  ret i8 %r\n}",
        "f",
    );
    assert!(text.contains("#_bad"), "{text}");
}

/// A result with no row in the convention (`i64`, eight bytes) panicked in
/// `result_registers` ("calling.toml has a result row"): 7 programs of
/// tests/run/c killed the compiler. It is written through the pointer the
/// caller passes in `hl`.
#[test]
fn a_result_the_convention_has_no_registers_for_is_written_through_a_pointer() {
    let out = compiled("define i64 @f() {\n  ret i64 5\n}", "f");
    assert!(out.contains("ld (hl), e"), "{out}");
}

/// `abi::z80()` parsed calling.toml on every call lowering: 12% of a debug
/// compile of tests/run/c/memory_expansion.c. The convention is parsed once.
#[test]
fn the_z80_convention_is_parsed_once() {
    assert!(std::ptr::eq(llrm_z80::abi::z80(), llrm_z80::abi::z80()));
}

/// A call with a stack argument from a frame of 200 bytes was refused ("a
/// stack argument -142 from ix"; `div_constants_long`): the argument area
/// is 142 bytes below IX, past the byte displacement. It is addressed from IY.
#[test]
fn a_stack_argument_beyond_the_ix_displacement_is_stored_through_iy() {
    let text = compiled(
        "declare void @use(ptr)\ndeclare i32 @g(i32, i32)\ndefine i32 @f(i32 %a, i32 %b) {\n  %buf = alloca [200 x i8]\n  call void @use(ptr %buf)\n  %r = call i32 @g(i32 %a, i32 %b)\n  ret i32 %r\n}",
        "f",
    );
    assert!(text.contains("add iy, sp") && text.contains("(iy)"), "{text}");
}

/// A float argument or result killed the compiler ("legal parts: Type(F32)",
/// 8 programs of tests/run/c): it is refused by name.
#[test]
fn a_float_argument_is_refused_not_a_panic() {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\ndefine float @f(float %x) {{\n  ret float %x\n}}");
    let why = text(&ir, "f", 0, &Request::new(Level::O2)).expect_err("the compiler refuses a float");
    assert!(why.contains("not a byte or a word"), "{why}");
}
