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
        (out.find("bit 7, ").expect("tests the top bit"), out.find("xor a, #33").expect("xors the low byte"));
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
    assert!(out.contains("ld h, d") && !out.contains("ld l, e"), "{out}");
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
#[ignore = "greedy has no hint to keep the pointers read through (hl) in hl"]
fn a_pointer_read_through_hl_in_a_loop_is_not_copied_into_hl_each_trip() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2))
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld l, e") && !out.contains("ld l, c"), "{out}");
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
/// tests/run/c killed the compiler instead of being named as refused.
#[test]
fn a_result_the_convention_has_no_registers_for_is_refused_not_a_panic() {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\ndefine i64 @f() {{\n  ret i64 5\n}}");
    let why = text(&ir, "f", 0, &Request::new(Level::O2)).expect_err("the compiler refuses an eight-byte result");
    assert!(why.contains("no registers"), "{why}");
}
