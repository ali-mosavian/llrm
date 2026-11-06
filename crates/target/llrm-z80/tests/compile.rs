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

/// -O2 without the full unroll, for the tests that need crc16's spills.
fn unrolled_off() -> Request {
    Request { passes: vec!["-fno-unroll-loops".into()], ..Request::new(Level::O2) }
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
    // the byte is stored and compared from the register it lives in, `c`
    // or, with the induction variable in `b`, `b`
    assert!(out.contains("ld (hl), ") && out.contains("cp a, "), "{out}");
    assert!(!out.contains("ld d, c") && !out.contains("ld d, b"), "{out}");
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

/// crc16 (loops not unrolled, which would free the registers it spilled)
/// reloaded its pointer from the slot at SP with `ld c, -3(ix)` / `ld b,
/// -2(ix)`, 6 B and 38 T-states on each of its trips; `pop bc` / `push bc` is 2
/// B and 21.
#[test]
fn the_slot_at_sp_is_reloaded_by_pop_and_push() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &unrolled_off()).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    assert!(lines.windows(2).any(|pair| pair == ["pop bc", "push bc"]), "{out}");
}

/// A spilled byte (crc16, loops not unrolled) was reloaded into a register
/// just to be compared
/// (`ld c, -1(ix)` / `cp a, c`): a byte more and 4 T-states than `cp a,
/// -1(ix)`.
#[test]
fn a_reloaded_operand_is_read_from_the_frame() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &unrolled_off()).unwrap_or_else(|why| panic!("{why}"));
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
/// the answer is 6. Whichever way it is avoided (the `sub` moved clear, the
/// compare made again), the last flag writer before a `jr` is a `cp`.
#[test]
fn a_flag_reader_after_a_clobber_reads_the_flags_of_its_compare() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %x = select i1 %c, i8 %b, i8 0\n  %d = sub i8 %a, %x\n  %y = select i1 %c, i8 0, i8 %a\n  %e = sub i8 %b, %y\n  %r = xor i8 %d, %e\n  ret i8 %r\n}";
    let out = compiled(ir, "f");
    let writes_flags = |line: &str| {
        let op = line.trim().split_whitespace().next().unwrap_or("");
        matches!(
            op,
            "cp" | "sub" | "sbc" | "add" | "adc" | "inc" | "dec" | "and" | "or" | "xor" | "neg"
        )
    };
    let body: Vec<&str> = out.lines().collect();
    let mut readers = 0;
    for (at, line) in body.iter().enumerate().filter(|(_, line)| line.trim().starts_with("jr ")) {
        if line.trim().starts_with("jr L") {
            continue;
        }
        readers += 1;
        let writer = body[..at].iter().rev().find(|line| writes_flags(line)).expect("a flag writer");
        assert!(writer.trim().starts_with("cp"), "{writer} feeds {line}\n{out}");
    }
    assert!(readers >= 1, "{out}");
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
/// and 4 T-states more per widened load. (A word minus the widened byte: with
/// the byte on both sides it is `SUBZX8`.)
#[test]
fn a_widened_load_goes_to_the_pairs_low_half() {
    let out = compiled(
        "define i16 @w(ptr %p, i16 %x) {\n  %v = load i8, ptr %p\n  %z = zext i8 %v to i16\n  %r = sub i16 %x, %z\n  ret i16 %r\n}",
        "w",
    );
    let direct = ["ld c, (hl)\n\tld b, #0", "ld e, (hl)\n\tld d, #0"].iter().any(|pair| out.contains(pair));
    assert!(direct && !out.contains("ld c, b") && !out.contains("ld e, d"), "{out}");
}

/// `gcd`'s guard and latch both compare `cp a, l`; tail merging did not cross
/// from the guard into the loop at -Oz, so the test was emitted twice (14
/// instructions, 17 B); the pin merges across loops before placement: 13, 16 B.
#[test]
fn a_guard_and_a_latch_with_the_same_test_share_it_where_bytes_come_first() {
    let out = text(include_str!("kernels/ir/gcd.ll"), "gcd", 0, &Request::new(Level::Oz)).expect("compiles");
    let instructions = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert_eq!(instructions, 13, "{out}");
}

/// The merge leaves the loop's test at the bottom behind one entry jump (12 T
/// per call): -3 B for xs8 at -O2 (25 B, 524 T against 28 B, 507 T), a trade
/// speed does not make.
#[test]
fn a_loop_is_not_entered_by_a_jump_to_a_merged_test_at_o2() {
    let out = text(include_str!("kernels/ir/xs8.ll"), "xs8", 0, &Request::new(Level::O2)).expect("compiles");
    let first = out.lines().map(str::trim).find(|line| !line.starts_with('.') && !line.ends_with(':'));
    assert!(first.is_some_and(|line| !line.starts_with("jr ")), "{out}");
}

/// `fib`'s loop counted `a` down with `dec a`, added with `add hl, bc` and then
/// tested `a` again with `or a, a` before `ret z`: `add hl, rr` leaves the zero
/// flag, so `dec a`'s still stood (23 B, 685 T, 112 instructions; 22, 637, 100
/// without the test).
#[test]
fn a_zero_flag_stands_through_a_16_bit_add() {
    let out = text(include_str!("kernels/ir/fib.ll"), "fib", 0, &Request::new(Level::O2)).expect("compiles");
    // the counter is stepped before or after the add; either way the flag
    // stands for `ret z`
    assert!(out.contains("ret z") && !out.contains("add hl, bc\n\tor a, a"), "{out}");
    assert_eq!(out.matches("or a, a").count(), 1, "only the entry test: {out}");
}

/// The adds of a use after the loop were rated every trip, so LSR kept the
/// loop as written and `lz4` -Oz grew a second pointer's setup: 168
/// instructions against 159 with the adds rated as run once.
#[test]
fn an_add_after_the_loop_is_rated_once_not_every_trip() {
    let out = text(include_str!("kernels/ir/lz4.ll"), "lz4", 0, &Request::new(Level::Oz)).expect("compiles");
    let instructions = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(instructions <= 160, "{instructions} instructions\n{out}");
}

/// `sum4`'s rotated loop was `dec b; inc hl; jr nz`: the pointer step sat
/// between the decrement and the branch, so no `djnz` formed (12 B, 172 T, 22
/// instructions at -O2; 11 B, 160 T, 18 with it). sum4 is unrolled at -O2 now;
/// sum_arr has the same loop.
#[test]
fn a_decrement_behind_a_pointer_step_is_a_djnz() {
    let out = text(include_str!("kernels/ir/sum_arr.ll"), "sum_arr", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(out.contains("djnz") && !out.contains("dec b"), "{out}");
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
    let end = out[start..].find("nz, L0_2").expect("the back edge") + start;
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
    // the carry fusion has since turned the compare and its byte into `adc hl,
    // rr`
    assert!(out.contains("adc hl, ") && !out.contains("SETCC8"), "{out}");
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
/// is 142 bytes below IX, past the byte displacement. It is addressed from IY
/// (byte arguments: words are pushed).
#[test]
fn a_stack_argument_beyond_the_ix_displacement_is_stored_through_iy() {
    let text = compiled(
        "declare void @use(ptr)\ndeclare i32 @g(i32, i32, i8, i8, i8)\ndefine i32 @f(i32 %a, i32 %b) {\n  %buf = alloca [200 x i8]\n  call void @use(ptr %buf)\n  %r = call i32 @g(i32 %a, i32 %b, i8 1, i8 2, i8 3)\n  ret i32 %r\n}",
        "f",
    );
    assert!(text.contains("add iy, sp") && text.contains("(iy)"), "{text}");
}

/// `w >> 8` of a spilled word reloaded both bytes and then overwrote the low
/// one (`ld e, -8(ix)` / `ld d, -7(ix)` / `ld e, d` / `ld d, #0`): 3 B and 19 T
/// a site, 11 sites in div_constants_short. Only the high byte is loaded now.
#[test]
fn a_spilled_word_shifted_right_by_a_byte_reloads_one_byte() {
    let ir = include_str!("kernels/ir/reload_high_lane.ll");
    let out = text(ir, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    let dead = lines
        .windows(4)
        .any(|w| w[0].starts_with("ld e, -") && w[1].starts_with("ld d, -") && w[2] == "ld e, d" && w[3] == "ld d, #0");
    assert!(!dead, "{out}");
    assert!(lines.windows(2).any(|w| w[0].starts_with("ld e, -") && w[1] == "ld d, #0"), "{out}");
}

/// A float argument or result killed the compiler ("legal parts: Type(F32)",
/// 8 programs of tests/run/c); it is softened to an integer now (#6578), so
/// passing one through is register moves and a return, no routine.
#[test]
fn a_float_argument_passes_as_two_words() {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\ndefine float @f(float %x) {{\n  ret float %x\n}}");
    let asm = text(&ir, "f", 0, &Request::new(Level::O2)).expect("a float compiles");
    assert!(asm.trim_end().ends_with("ret") && !asm.contains("call") && !asm.contains("jp "), "{asm}");
}

/// A flag writer scheduled between a compare and its branch overwrites the
/// flags the branch reads (gcd returned 4 where the answer is 6). The emitter
/// makes the compare again before the branch. Fed the split order directly,
/// whatever the scheduler would do.
#[test]
fn a_compare_is_made_again_when_a_flag_writer_sits_between_it_and_its_branch() {
    use llrm_codegen::backend::selectiondag::SDValue;
    use llrm_codegen::backend::selectiondag::isd::ISD;
    use llrm_z80::dagisel::generated::ids::opcode as instr;
    use llrm_z80::driver::selections;
    use llrm_z80::mfunc::{Kind, emit};

    let ir = include_str!("kernels/ir/find.ll");
    let (_, selection) =
        selections(ir, &Request::new(Level::O2)).expect("selects").into_iter().find(|(one, _)| one == "find").unwrap();
    let mut selection = selection.expect("selected");
    let compares = |selection: &llrm_z80::dagisel::function::Selection| {
        let function = emit("find", selection).expect("emitted");
        function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op("CP8m")).count()
    };
    assert_eq!(compares(&selection), 1, "premise: scheduled apart, the compare is made once");

    let block = selection
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .units
                .iter()
                .any(
                    |unit| matches!(
                        block.dag.node(SDValue { node: unit.nodes[0], res: 0 }).opcode,
                        ISD::Machine(op) if op == u32::from(instr::CP8m)
                    ),
                )
        })
        .expect("the block with the compare");
    let at = |block: &llrm_z80::dagisel::function::Selected, wanted: u16| {
        block
            .units
            .iter()
            .position(
                |unit| matches!(
                    block.dag.node(SDValue { node: unit.nodes[0], res: 0 }).opcode,
                    ISD::Machine(op) if op == u32::from(wanted)
                ),
            )
    };
    // the increment and the copy of its result, after the compare
    let (cp, inc) = (at(block, instr::CP8m).unwrap(), at(block, instr::INC8).unwrap());
    assert!(inc < cp, "premise: the increment is scheduled before the compare");
    let moved: Vec<_> = block.units.drain(inc..=inc + 1).collect();
    let cp = cp - 2;
    for (at, unit) in moved.into_iter().enumerate() {
        block.units.insert(cp + 1 + at, unit);
    }
    assert_eq!(compares(&selection), 2, "the compare is made again after the increment");
}

/// Neither the `Source` nor the `ILP` scheduler wins every function (DEVIATE
/// Z80S1): `ILP` makes xs8 -O2 two bytes smaller, and spills minmax -Oz to the
/// frame (53 B against 29). The cheaper is kept.
#[test]
fn a_function_is_scheduled_by_whichever_of_source_and_ilp_makes_it_cheaper() {
    use llrm_z80::compile::lines;
    use llrm_z80::expand::price;
    let cost = |ir: &str, name: &str, level| {
        price(&lines(ir, name, 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}")))
    };
    let xs8 = cost(include_str!("kernels/ir/xs8.ll"), "xs8", Level::O2);
    let minmax = cost(include_str!("kernels/ir/minmax.ll"), "minmax", Level::Oz);
    assert!(xs8.0 <= 26, "xs8 -O2 is {} B, Source alone gives 28", xs8.0);
    assert!(minmax.0 <= 29, "minmax -Oz is {} B, ILP alone gives 53", minmax.0);
}

/// A frame object out of ix's reach is addressed through `ld iy, #n; add iy,
/// sp`, and the sum sets the carry: a reload between a compare and its `jp c`
/// changed what the branch saw (divshift's `smod(-c, -d)` returned garbage once
/// its frame passed 128 bytes). The flags are kept across the sum, and the
/// stack pointer's two bytes of `push af` are in its offset. Hand-built, in the
/// order the source gives: compare, reload, branch.
#[test]
fn a_far_frame_access_between_a_compare_and_its_branch_keeps_the_carry() {
    use llrm_spike_mf::regalloc::Assignment;
    use llrm_z80::expand::{Line, finish, reads};
    use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
    use llrm_z80::regalloc::Class;
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr8, Class::Gr8, Class::Gr8];
    let compare = Inst {
        sets_flags: true,
        ..Inst::new(
            Kind::Op("CP8r"),
            vec![Operand::Def(Reg::Phys(llrm_z80::reg::FLAGS)), Operand::Use(Reg::Virt(0)), Operand::Use(Reg::Virt(1))],
        )
    };
    let reload = Inst::new(Kind::Op("RELOAD8"), vec![Operand::Def(Reg::Virt(2)), Operand::Frame(100)]);
    let branch = Inst {
        reads_flags: true,
        terminator: true,
        ..Inst::new(Kind::Op("JPC"), vec![Operand::Block(1), Operand::Use(Reg::Phys(llrm_z80::reg::FLAGS))])
    };
    function.blocks.push(Block { id: 0, insts: vec![compare, reload, branch], phis: Vec::new() });
    function.blocks.push(Block { id: 1, insts: vec![Inst::new(Kind::Op("RET"), vec![])], phis: Vec::new() });
    let assignment = Assignment(vec![llrm_z80::reg::A, llrm_z80::reg::B, llrm_z80::reg::C]);
    // object 100 sits below 100 words of other objects: out of ix's reach
    let slots: Vec<(i32, usize)> = (0..101).map(|at| (at, 2)).collect();
    let lines = finish(&function, &assignment, 0, &slots, reads(&function)).unwrap_or_else(|why| panic!("{why}"));
    let insts: Vec<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let cp = insts.iter().position(|text| *text == "cp a, b").expect("the compare");
    let sum = cp
        + insts[cp..]
            .iter()
            .position(|text| *text == "add iy, sp")
            .unwrap_or_else(|| panic!("premise: via iy {insts:?}"));
    assert_eq!(insts[sum - 2], "push af", "{insts:?}");
    assert_eq!(insts[sum + 1], "pop af", "{insts:?}");
    assert!(insts[sum - 1].starts_with("ld iy, #"), "{insts:?}");
}

/// `br i1 %c, label %r, label %r` (left by -O0 and by x_switch's mid-end)
/// made a `jp c` and a `jp` to one `ret` block: the block was copied into
/// the predecessor in place of the `jp` and deleted, leaving the `jp c`
/// without a target, and the compiler panicked in tail merging.
#[test]
fn a_conditional_branch_to_the_same_return_block_as_the_jump_keeps_that_block() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\ndefine i16 @f(i16 %x) {{\n  %c = icmp sgt i16 %x, 0\n  br i1 %c, label %r, label %r\nr:\n  ret i16 0\n}}"
    );
    let out = text(&ir, "f", 0, &Request::new(Level::O0)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ret"), "{out}");
}

/// A word that joins a byte and a constant (`*a`, or 0 where the string
/// ended) was not known to have a zero high byte, so strcmp's `*a - *b` was a
/// 16-bit `sbc hl, de` with both bytes widened by hand: scmp -O2 took 36 B and
/// 183 clocks. The difference is `sub a, b` and the borrow as the high byte.
#[test]
fn a_difference_of_a_phi_of_bytes_and_a_byte_is_a_byte_subtract_with_the_borrow() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(out.contains("sbc a, a") && !out.contains("sbc hl, de"), "{out}");
}

/// switch.ll's two functions took 8 full allocations each (the return block
/// copied or kept, constants hoisted or not, the argument copies joined or
/// not): 16 greedy runs, 51% of a 1.3 G-instruction compile. Copied and kept
/// gave the same code on all 60 kernel cells; only the copied one is made.
/// lz4 -O2 took 16 runs. The choice of a byte loaded through `a` with the
/// argument copies unjoined won on no kernel cell and none of the 105 corpus
/// files: 6 per scheduler (`Source`, `ILP`), 12.
#[test]
fn a_function_is_allocated_at_most_six_times_per_scheduler() {
    let ir = include_str!("kernels/ir/lz4.ll");
    llrm_z80::compile::take_allocations();
    text(ir, "lz4", 0, &Request::new(Level::O2)).expect("compiles");
    let runs = llrm_z80::compile::take_allocations();
    assert!(runs <= 12, "lz4 -O2 was allocated {runs} times");
}

fn many(
    count: usize,
    constant: u32,
) -> String {
    let mut ir = format!("target datalayout = \"{DATALAYOUT}\"\n");
    for i in 0..count {
        ir.push_str(&format!(
            "define i16 @f{i}(i16 %a) {{\n  %r = add i16 %a, {}\n  ret i16 %r\n}}\n",
            constant + i as u32
        ));
    }
    ir
}

/// Every function of a module ran the whole mid-end and the selection of every
/// function again, twice (the return block copied and kept): a module of N
/// functions cost N squared (switch.ll 1.29 G instructions, 4.75x the x86
/// reference). The module is selected once however many functions it has.
#[test]
fn a_module_is_run_through_the_mid_end_once_however_many_functions_it_has() {
    for count in [3, 30] {
        let ir = many(count, 1);
        llrm_z80::compile::take_selection_runs();
        for i in 0..count {
            text(&ir, &format!("f{i}"), i, &Request::new(Level::O2)).expect("compiles");
        }
        assert_eq!(llrm_z80::compile::take_selection_runs(), 1, "{count} functions");
    }
}

/// The held module's text was compared with the module's, whole, each time a
/// function was built (four times a function): O(module text) per function,
/// the +6% per function left from 150 to 1200 functions. A prepared module is
/// not compared at all, and `text` compares once per function.
#[test]
fn the_text_of_a_module_is_not_compared_once_per_function_built() {
    for count in [10, 40] {
        let ir = many(count, 1);
        let request = Request::new(Level::O2);
        llrm_z80::compile::take_text_compares();
        let prepared = llrm_z80::compile::prepare(&ir, &request).expect("prepares");
        for (number, name) in prepared.names().enumerate() {
            llrm_z80::compile::text_in(&prepared, name, number, &request).expect("compiles");
        }
        assert_eq!(llrm_z80::compile::take_text_compares(), 0, "{count} prepared functions");
        for i in 0..count {
            text(&ir, &format!("f{i}"), i, &request).expect("compiles");
        }
        assert_eq!(llrm_z80::compile::take_text_compares(), count, "{count} functions through `text`");
    }
}

/// With `-g` every function parsed the module's text again for its source
/// file and start line: O(module) per function. A prepared module is parsed
/// once.
#[test]
fn a_module_is_parsed_for_its_sources_once_however_many_functions_it_has() {
    for count in [10, 40] {
        let ir = many(count, 1);
        let request = Request { debug: true, ..Request::new(Level::O2) };
        llrm_z80::driver::take_source_parses();
        let prepared = llrm_z80::compile::prepare(&ir, &request).expect("prepares");
        for (number, name) in prepared.names().enumerate() {
            llrm_z80::compile::text_in(&prepared, name, number, &request).expect("compiles");
        }
        assert_eq!(llrm_z80::driver::take_source_parses(), 1, "{count} functions");
    }
}

/// The module kept for the next function is the module asked for: two modules
/// that name a function alike, compiled one after the other, give their own
/// code.
#[test]
fn two_modules_compiled_in_turn_do_not_share_the_cache() {
    let (one, other) = (many(2, 1000), many(2, 2000));
    let first = text(&one, "f0", 0, &Request::new(Level::O2)).expect("compiles");
    let second = text(&other, "f0", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(first.contains("#1000") && second.contains("#2000"), "{first}\n{second}");
    let again = text(&one, "f0", 0, &Request::new(Level::O2)).expect("compiles");
    assert_eq!(first, again);
}

/// gcd -O2 ended its subtract arm with `jr` (12 T taken, always taken): 275
/// clocks against gcc's `jp` at 274. Where speed comes first an always-taken
/// jump stays a `jp`; at -Oz it is a `jr`, a byte less.
#[test]
fn an_always_taken_jump_is_jp_when_speed_comes_first_and_jr_when_size_does() {
    let ir = include_str!("kernels/ir/gcd.ll");
    let fast = text(ir, "gcd", 0, &Request::new(Level::O2)).expect("compiles");
    let small = text(ir, "gcd", 0, &Request::new(Level::Oz)).expect("compiles");
    let unconditional = |asm: &str, op: &str| asm.lines().any(|line| line.trim_start().starts_with(&format!("{op} L")));
    assert!(unconditional(&fast, "jp") && !unconditional(&fast, "jr"), "{fast}");
    assert!(unconditional(&small, "jr") && !unconditional(&small, "jp"), "{small}");
}

/// An always-taken jump outside every loop ran once: scmp -O2's empty-string
/// exit was a `jp` for 1 B and no clocks saved on any input. Only a jump inside
/// a loop stays a `jp` where speed comes first.
#[test]
fn a_jump_outside_every_loop_is_jr_even_when_speed_comes_first() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!out.lines().any(|line| line.trim_start().starts_with("jp ")), "{out}");
}

/// The driver turned the unroller off: sum4's four-trip loop ran 160 clocks at
/// -O2, 104 unrolled. A loop of four trips is copied out where speed comes
/// first, and stays a loop at -Oz.
#[test]
fn a_four_trip_loop_is_unrolled_where_speed_comes_first_and_not_at_oz() {
    let ir = include_str!("kernels/ir/sum4.ll");
    let fast = text(ir, "sum4", 0, &Request::new(Level::O2)).expect("compiles");
    let small = text(ir, "sum4", 0, &Request::new(Level::Oz)).expect("compiles");
    assert!(!fast.contains("djnz") && !fast.contains(" L0_"), "{fast}");
    assert!(small.contains("djnz"), "{small}");
}

/// lz4's match copy kept the source as `dst + offset` (a base add, `pop bc;
/// push bc; add hl,bc`) and its pointers in ix slots: 102905 clocks at -O2,
/// gcc's end-pointer loop 35781. Both copies, the literals and the match, are
/// byte loops of registers, and an `ldir` each: 19721.
#[test]
fn the_literal_and_match_copies_are_an_ldir_each() {
    let out = text(include_str!("kernels/ir/lz4.ll"), "lz4", 0, &Request::new(Level::O2)).expect("compiles");
    assert_eq!(out.matches("\tldir").count(), 2, "{out}");
}

/// reverse -Oz kept `p + i` and `p + j` as base adds to spare a register, and
/// spilled the rest to the frame: 105 B and 1900 T against SDCC's 68 B and
/// 1270 T. Base adds are rated before registers; 68 B, 1222 T.
#[test]
fn reverse_at_oz_does_not_live_in_the_frame() {
    let out = text(include_str!("kernels/ir/reverse.ll"), "reverse", 0, &Request::new(Level::Oz)).expect("compiles");
    let frame = out.lines().filter(|line| line.contains("(ix)")).count();
    assert!(frame <= 10, "{frame} frame accesses\n{out}");
}

/// crc16's inner loop of eight trips kept its counter and branch: `dec h; jr
/// nz` is 16 T a bit, 7135 T for 12 bytes against gcc's 5134. Where speed
/// comes first it is copied out (the pin's threshold: +67 B, -2490 T, past 1 B
/// = 3 T); at -Oz it stays a loop.
#[test]
fn an_eight_trip_loop_is_unrolled_where_speed_comes_first_and_not_at_oz() {
    let ir = include_str!("kernels/ir/crc16.ll");
    let fast = text(ir, "crc16", 0, &Request::new(Level::O2)).expect("compiles");
    let small = text(ir, "crc16", 0, &Request::new(Level::Oz)).expect("compiles");
    assert!(!fast.contains("dec h"), "{fast}");
    assert!(small.contains("dec h"), "{small}");
}

const MEMCPY: &str = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\n\
    define void @mc(ptr %d, ptr %s, i16 %n) {\n  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)\n  ret void\n}";

/// A `memcpy` of a length known only at run time called `__memcpy`: the call, a
/// push, a swap, the same test of the count, the same `ldir`, a pop and a
/// return, about 60 T more than the copy. Where speed comes first it is the
/// test and `ldir` inline; tuned for size it stays the call, 4 B less.
#[test]
fn a_memcpy_of_unknown_length_is_an_ldir_behind_a_test_of_the_count_where_speed_comes_first() {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{MEMCPY}");
    let fast = text(&ir, "mc", 0, &Request::new(Level::O2)).expect("compiles");
    let small = text(&ir, "mc", 0, &Request::new(Level::Oz)).expect("compiles");
    assert!(fast.contains("or a, c\n\tjr z, ") && fast.contains("ldir") && !fast.contains("__memcpy"), "{fast}");
    assert!(small.contains("__memcpy") && !small.contains("ldir"), "{small}");
}

/// `ldir` copies forward, so a `memmove` with the destination above the source
/// would overwrite its own source: the runtime's routine picks the direction,
/// and the inline form is only for `memcpy`.
#[test]
fn a_memmove_is_still_a_call() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\ndeclare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)\n\
         define void @mm(ptr %d, ptr %s, i16 %n) {{\n  call void @llvm.memmove.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)\n  ret void\n}}"
    );
    let out = text(&ir, "mm", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(out.contains("__memmove") && !out.contains("ldir"), "{out}");
}

/// The last block of `asm` that holds `add a, (hl)` and the loop's count step.
fn checksum_loop(asm: &str) -> String {
    asm.split("\nL0_")
        .filter(|block| block.contains("add a, (hl)") && block.contains("dec bc"))
        .last()
        .unwrap_or_else(|| panic!("no checksum loop in\n{asm}"))
        .to_owned()
}

/// A loop over `c[0..n]` after a loop that used up the registers: the pointer
/// was copied into `hl` for `add a,(hl)` and stepped after it, so it overlapped
/// `hl`'s range and could not be `hl`; the pointer, the count and both sums
/// were in the frame, five `(ix)` accesses a trip against none.
#[test]
fn a_pointer_read_through_hl_and_stepped_after_stays_in_hl() {
    let out = compiled(
        "define i16 @rs1(ptr %a, ptr %b, ptr %c, i16 %n) {\n  %z = icmp eq i16 %n, 0\n  br i1 %z, label %out, label %l1\n\
         l1:\n  %i = phi i16 [ 0, %0 ], [ %i1, %l1 ]\n  %pa = getelementptr inbounds nuw i8, ptr %a, i16 %i\n  \
         %pb = getelementptr inbounds nuw i8, ptr %b, i16 %i\n  %pc = getelementptr inbounds nuw i8, ptr %c, i16 %i\n  \
         %x = load i8, ptr %pa\n  %y = load i8, ptr %pb\n  %s = add i8 %x, %y\n  store i8 %s, ptr %pc\n  \
         %i1 = add nuw i16 %i, 1\n  %c1 = icmp ult i16 %i1, %n\n  br i1 %c1, label %l1, label %l2\n\
         l2:\n  %j = phi i16 [ 0, %l1 ], [ %j1, %l2 ]\n  %u = phi i8 [ 0, %l1 ], [ %u1, %l2 ]\n  \
         %v = phi i8 [ 0, %l1 ], [ %v1, %l2 ]\n  %pj = getelementptr inbounds nuw i8, ptr %c, i16 %j\n  \
         %w = load i8, ptr %pj\n  %u1 = add i8 %u, %w\n  %v1 = add i8 %v, %u1\n  %j1 = add nuw i16 %j, 1\n  \
         %c2 = icmp ult i16 %j1, %n\n  br i1 %c2, label %l2, label %done\n\
         done:\n  %hi = zext i8 %v1 to i16\n  %sh = shl i16 %hi, 8\n  %lo = zext i8 %u1 to i16\n  %r = or i16 %sh, %lo\n  \
         ret i16 %r\nout:\n  ret i16 0\n}",
        "rs1",
    );
    let body = checksum_loop(&out);
    assert!(!body.contains("(ix)") && !body.contains("ld l, c"), "{out}");
}

/// lz4c's checksum loop over `out[i]` kept `out + i` as a base add with the
/// bound and `out` read from the frame every trip, 150 T a byte against gcc's
/// 106: the exit count `umax(1, n) - 1` cost 6 against a budget of 4 basic
/// instructions. lz4c -O2 111095 T against gcc's 103309; 102805 stepping a
/// pointer and counting down in `bc`, which still loaded and stored the
/// pointer's slot every trip; 54659 with the pointer in `hl` across the back
/// edge.
#[test]
fn lz4cs_checksum_loop_steps_a_pointer_in_hl_and_counts_down() {
    let out = text(include_str!("kernels/ir/lz4c.ll"), "lz4c", 0, &Request::new(Level::O2)).expect("compiles");
    let body = checksum_loop(&out);
    assert!(!body.contains("add hl, bc") && !body.contains("(ix)"), "{out}");
}

/// scmp's exit merged `zext c` and `0` into a pair and subtracted its low
/// byte: `ld c, a` / `ld b, #0` made a high byte nothing read, and `ld bc, #0`
/// another, 33 B and 165 T against SDCC's 24 B and 147 T. The pair is a byte.
#[test]
fn a_pair_only_its_low_byte_is_read_from_is_a_byte() {
    let out = text(include_str!("kernels/ir/scmp.ll"), "scmp", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!out.contains("ld b, #0") && !out.contains("ld bc, #0"), "{out}");
}

/// A size build copied a callee bigger than its call into every caller: the
/// pin's `OptSizeThreshold` allows growth, and `step`'s six word updates are
/// 36 B where the call is 3. `step` is called from three places and stays one
/// body at -Os; at -O2 the pin's threshold alone decides.
#[test]
fn a_size_build_does_not_copy_a_callee_bigger_than_its_call() {
    let globals: String = (0..6).map(|i| format!("@g{i} = global i16 0\n")).collect();
    let updates: String = (0..6)
        .map(|i| {
            format!("  %v{i} = load i16, ptr @g{i}\n  %w{i} = add i16 %v{i}, {}\n  store i16 %w{i}, ptr @g{i}\n", 7 + i)
        })
        .collect();
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n{globals}define void @step() {{\n{updates}  ret void\n}}\n\
         define void @one() {{\n  call void @step()\n  ret void\n}}\n\
         define void @two() {{\n  call void @step()\n  call void @step()\n  ret void\n}}"
    );
    let calls: usize = ["one", "two"]
        .iter()
        .map(|name| {
            let out = text(&ir, name, 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
            out.matches("_step").count()
        })
        .sum();
    assert_eq!(calls, 3, "step was copied into its callers");
}

/// A callee with a frame pays its entry and exit at every call: `push ix`,
/// `ld ix`, `add ix,sp`, the stack adjustment and their undoing, 12 to 20 B,
/// which an inline removes. Priced as the bare call, -Os kept `bench` (two
/// calls, a 140 B frame) a call: stack_arrays_copy was 155 B, 122 B inlined.
#[test]
fn a_size_build_inlines_a_callee_whose_frame_costs_more_than_its_body() {
    let ir = include_str!("kernels/ir/stack_arrays_copy.ll");
    let out = text(ir, "main", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("_bench"), "{out}");
}

/// A callee's frame is built once, and an inlined copy builds it in the caller:
/// credited per site, as if every copy shed one, -Os inlined `dirty` (a 96 B
/// frame, two sites) into a `main` with no frame of its own and
/// long_double_constant_store grew from 64 B to 72 B. `dirty` stays a call.
#[test]
fn a_size_build_keeps_a_callee_whose_frame_would_move_into_a_frameless_caller() {
    let ir = include_str!("kernels/ir/long_double_constant_store.ll");
    let out = text(ir, "main", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(out.matches("call _dirty").count(), 2, "{out}");
}

/// A counted byte copy kept its count in a word: `ld de, #200` and `dec de;
/// ld a, d; or a, e` a trip, with the pointer stepped beside it. HardwareLoops
/// counts the trips in a byte (`ld b, #200`) and the word is gone: rep_count
/// was 9% fewer clocks, mul_constants 12% fewer bytes.
#[test]
fn a_counted_loop_counts_its_trips_in_a_byte() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n@a = global [200 x i8] zeroinitializer\n\
         @b = global [200 x i8] zeroinitializer\n\
         define void @copy() {{\nentry:\n  br label %loop\nloop:\n\
         \x20 %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]\n\
         \x20 %p = getelementptr i8, ptr @a, i16 %i\n  %v = load i8, ptr %p\n  %w = add i8 %v, 1\n\
         \x20 %q = getelementptr i8, ptr @b, i16 %i\n  store i8 %w, ptr %q\n\
         \x20 %i.next = add nuw i16 %i, 1\n  %c = icmp ne i16 %i.next, 200\n\
         \x20 br i1 %c, label %loop, label %exit\nexit:\n  ret void\n}}"
    );
    let out = text(&ir, "copy", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("ld b, #200"), "{out}");
    assert!(!out.contains("or a, e"), "the word count is tested no more: {out}");
}

/// A function tuned for size called the long long divide made for speed: the
/// routine is 179 B more to link (data_int64 and int64_division, -Os). It
/// calls the size routine; any other function the speed one.
#[test]
fn a_function_tuned_for_size_calls_the_small_long_long_divide() {
    let body = |attr: &str| {
        format!(
            "define i64 @f(i64 %a, i64 %b) {attr} {{\n  %q = udiv i64 %a, %b\n  %r = srem i64 %a, %b\n  %s = add i64 %q, %r\n  ret i64 %s\n}}"
        )
    };
    for attr in ["optsize", "minsize"] {
        let out = compiled(&body(attr), "f");
        assert!(out.contains("call __s_udivdi3") && out.contains("call __s_moddi3"), "{attr}: {out}");
    }
    let out = compiled(&body(""), "f");
    assert!(out.contains("call __udivdi3") && !out.contains("_s\n"), "{out}");
}

/// A function tuned for size called the long divide made for speed: 391 B to
/// link where the register-only routine is 76 (div_constants_ulong, -Os: +278 B
/// against SDCC). It calls the size routine, for the signed and the unsigned
/// divide and remainder; any other function the speed one.
#[test]
fn a_function_tuned_for_size_calls_the_small_long_divide() {
    let body = |attr: &str, ops: [&str; 2]| {
        format!(
            "define i32 @f(i32 %a, i32 %b) {attr} {{\n  %q = {} i32 %a, %b\n  %r = {} i32 %b, %a\n  %s = add i32 %q, %r\n  ret i32 %s\n}}",
            ops[0], ops[1]
        )
    };
    for (ops, small, fast) in [
        (["udiv", "urem"], ["call __s_udivsi3", "call __s_umodsi3"], ["call __udivsi3", "call __umodsi3"]),
        (["sdiv", "srem"], ["call __s_divsi3", "call __s_modsi3"], ["call __divsi3", "call __modsi3"]),
    ] {
        for attr in ["optsize", "minsize"] {
            let out = compiled(&body(attr, ops), "f");
            assert!(small.iter().all(|call| out.contains(call)), "{attr} {ops:?}: {out}");
        }
        let out = compiled(&body("", ops), "f");
        assert!(fast.iter().all(|call| out.contains(call)) && !out.contains("__s_"), "{ops:?}: {out}");
    }
}

/// A long divided and taken the remainder of by the same operands was two calls
/// (`x / d` and `x % d` of div_constants: two 388 B-routine runs of the same
/// loop). They are one call to the routine that stores the remainder through an
/// address (the pin's `useDivRem`); a divide alone, or a remainder of other
/// operands, keeps its own call.
#[test]
fn a_long_divide_and_remainder_of_the_same_operands_are_one_call() {
    let body = |attr: &str, ops: [&str; 2]| {
        format!(
            "define i32 @f(i32 %a, i32 %b) {attr} {{\n  %q = {} i32 %a, %b\n  %r = {} i32 %a, %b\n  %s = add i32 %q, %r\n  ret i32 %s\n}}",
            ops[0], ops[1]
        )
    };
    for (ops, fast, small, others) in [
        (["udiv", "urem"], "call __udivremsi4", "call __s_udivremsi4", ["__udivsi3", "__umodsi3"]),
        (["sdiv", "srem"], "call __divremsi4", "call __s_divremsi4", ["__divsi3", "__modsi3"]),
    ] {
        let out = compiled(&body("", ops), "f");
        assert_eq!(out.matches("call").count(), 1, "{ops:?}: {out}");
        assert!(out.contains(fast), "{ops:?}: {out}");
        let out = compiled(&body("optsize", ops), "f");
        assert_eq!(out.matches("call").count(), 1, "{ops:?} optsize: {out}");
        assert!(out.contains(small), "{ops:?} optsize: {out}");
        let alone = compiled(
            &format!("define i32 @f(i32 %a, i32 %b) {{\n  %q = {} i32 %a, %b\n  ret i32 %q\n}}", ops[0]),
            "f",
        );
        assert!(alone.contains(&format!("call {}", others[0])) && !alone.contains("rem"), "{ops:?} alone: {alone}");
    }
}

/// A long multiplied by a constant was split into its halves, each half
/// shifted and subtracted, in a function tuned for size: `x * 31` was 99 B and
/// `x * 1000` 185 B, where a call is 14 (div_constants_short, mul_constants,
/// -Os). A function tuned for size calls `__mulsi3`; the others keep the
/// halves.
#[test]
fn a_function_tuned_for_size_calls_the_long_multiply() {
    let body = |attr: &str| {
        format!(
            "define i32 @f(i32 %a, i32 %b, i32 %c) {attr} {{\n  %r = mul i32 %a, %b\n  %s = mul i32 %r, %c\n  ret i32 %s\n}}"
        )
    };
    let out = compiled(&body("optsize"), "f");
    assert!(out.contains("call __mulsi3"), "{out}");
    let out = compiled(&body(""), "f");
    assert!(!out.contains("__mulsi3"), "{out}");
}

/// The routine is 57 B: one small multiply in a module called it at -Os and
/// the program grew (copy_runs +10 B, switch_jump_table +31 B). A module
/// needs two long multiplies for the call.
#[test]
fn one_long_multiply_in_a_module_stays_inline_at_size() {
    let ir = "define i32 @f(i32 %a) optsize {\n  %r = mul i32 %a, 31\n  ret i32 %r\n}";
    let out = compiled(ir, "f");
    assert!(!out.contains("__mulsi3"), "{out}");
}

/// A long shifted by a constant was unrolled in a function tuned for size:
/// `x << 5` 60 B, `x >> 13` 49 B, where the shift routine is a 14 B loop
/// (-Os). The bytes still shifted times the bits beyond whole bytes are the
/// cost: 5 and 13 take the routine; whole bytes, one or two bits and the last
/// byte (29, 31) stay unrolled, as a 64-bit shift's halves did (data_int64
/// +199 B when 29 and 31 called); speed levels never take it.
#[test]
fn a_function_tuned_for_size_shifts_a_long_by_a_constant_in_the_routine() {
    let shifted = |op: &str, count: u32, attr: &str| {
        compiled(&format!("define i32 @f(i32 %a) {attr} {{\n  %r = {op} i32 %a, {count}\n  ret i32 %r\n}}"), "f")
    };
    for (op, routine) in [("shl", "__ashlsi3"), ("lshr", "__lshrsi3"), ("ashr", "__ashrsi3")] {
        for count in [5, 13] {
            assert!(shifted(op, count, "optsize").contains(routine), "{op} {count}");
        }
        for count in [1, 2, 8, 16, 29, 31] {
            assert!(!shifted(op, count, "optsize").contains("__"), "{op} {count} stays unrolled");
        }
        assert!(!shifted(op, 5, "").contains("__"), "{op} 5 at speed levels");
    }
}

/// A long compared with another was two word compares joined by a select: each
/// made a 0/1 byte, the high words' equality picked one (`lcmp` 53 B at -Os,
/// SDCC 27). The words are one chain of `sub`, `sbc`, `sbc`, `sbc` instead, the
/// order read from the carry; a signed order corrects the sign for overflow.
#[test]
fn a_long_order_is_one_borrow_chain() {
    let order = |cc: &str| {
        compiled(
            &format!(
                "define i16 @f(i32 %x, i32 %y) optsize {{\n  %c = icmp {cc} i32 %x, %y\n  %r = zext i1 %c to i16\n  ret i16 %r\n}}"
            ),
            "f",
        )
    };
    for cc in ["ult", "uge", "slt", "sge", "ugt", "ule", "sgt", "sle"] {
        let text = order(cc);
        assert_eq!(text.matches("sbc a,").count(), 3, "{cc}:\n{text}");
        assert!(!text.contains("jr nz"), "{cc} still joins the words by equality:\n{text}");
        assert_eq!(text.contains("jp po"), cc.starts_with('s'), "{cc}:\n{text}");
    }
}

/// The borrow of a 64-bit subtract's low long is the order chain of its words,
/// and the compare of the low word is shared with the subtract's own borrow:
/// the sharing kept the word subtract from becoming `sub hl` / `sbc hl`, and
/// int64_division grew 8 B over the select form. The subtract keeps its fused
/// words (at most 135 instructions; 145 with the compare gone from the fusion).
#[test]
fn a_long_subtract_borrow_shared_with_the_order_chain_still_fuses() {
    let text = compiled("define i64 @f(i64 %a, i64 %b) optsize {\n  %r = sub i64 %a, %b\n  ret i64 %r\n}", "f");
    let instructions =
        text.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(instructions <= 135, "{instructions} instructions:\n{text}");
}

/// `line` passed the address of its local to `kernel` and ended in `jp
/// _kernel` with the frame already popped: `kernel` ran over the local it was
/// handed (frame_address_stack_args -Oz printed 13599, not 13346).
#[test]
fn a_call_given_the_address_of_a_local_is_not_a_tail_jump() {
    let out = compiled(
        "declare void @use(ptr)\ndefine void @f() {\n  %a = alloca [4 x i8]\n  call void @use(ptr %a)\n  ret void\n}",
        "f",
    );
    assert!(out.contains("call _use") && !out.contains("jp _use"), "{out}");
}

/// The same call without a local to reach is still a tail jump.
#[test]
fn a_call_with_no_local_to_reach_is_a_tail_jump() {
    let out =
        compiled("declare void @use(i16)\ndefine void @f(i16 %x) {\n  call void @use(i16 %x)\n  ret void\n}", "f");
    assert!(out.contains("jp _use"), "{out}");
}

/// `x + 1` and `x - 1` on a long rebuilt the low word's carry by a compare, a
/// `ld h, #1` / `jr` / `dec h` byte and an add: 19 B and 20 B. The carry is a
/// zero test of the low word and the high word steps behind a `jr` (12 B).
#[test]
fn a_long_step_of_one_is_a_zero_test_and_a_conditional_step() {
    let ir = "define i32 @inc(i32 %x) {\n  %r = add i32 %x, 1\n  ret i32 %r\n}\n\
              define i32 @dec(i32 %x) {\n  %r = add i32 %x, -1\n  ret i32 %r\n}";
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    for (name, step) in [("inc", "inc"), ("dec", "dec")] {
        let out = text(&ir, name, 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
        let instructions = out.lines().filter(|l| l.starts_with('\t') && !l.trim_start().starts_with('.')).count();
        assert!(out.contains("jr nz") && !out.contains("sbc") && !out.contains("sub a"), "{name}\n{out}");
        assert!(out.contains(&format!("\t{step} ")) && instructions <= 12, "{name}: {instructions}\n{out}");
    }
}

/// The divisor's high word of a long divide went on the stack as `push bc`
/// (a reserved slot) and two stores into it from `ix`: 10 B for the word, and
/// 60 B in each `div_constants_long` function. It is `ld rr, #0; push rr`.
#[test]
fn the_stack_word_of_a_long_divide_is_pushed_not_stored() {
    let ir = "define i32 @q(i32 %x) {\n  %r = sdiv i32 %x, 3\n  ret i32 %r\n}";
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let out = text(&ir, "q", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    let call =
        lines.iter().position(|l| l.contains("call __divsi3")).unwrap_or_else(|| panic!("no divide call\n{out}"));
    let push = lines[..call].iter().rposition(|l| l.starts_with("push")).unwrap_or_else(|| panic!("no push\n{out}"));
    let pair = lines[push].trim_start_matches("push ");
    assert_eq!(lines[push - 1], format!("ld {pair}, #0"), "{out}");
    assert!(
        !lines[push..call].iter().any(|l| l.starts_with("ld -") && l.contains("(ix)")),
        "stored into the slot\n{out}"
    );
}

/// The shift routines of the z80 (`__ashlsi3` and the others) read their count
/// from `a`, one byte; the pin makes a library shift's amount the C `int`, so
/// the shared legalizer's default would hand them a word in `bc` and every
/// shift of a long by a constant would be wrong. z80 keeps the byte until its
/// helpers take a word (`with_narrow_library_shift`).
#[test]
fn a_long_shift_routine_takes_its_count_in_a_as_a_byte() {
    let shifted = compiled("define i32 @f(i32 %a) optsize {\n  %r = shl i32 %a, 5\n  ret i32 %r\n}", "f");
    assert!(shifted.contains("ld a, #5"), "{shifted}");
    assert!(!shifted.contains("ld bc, #5") && !shifted.contains("ld hl, #5"), "{shifted}");
}

/// Positive control for the stack-slot refusal counter: `x` is spilled before
/// `y` and reloaded first, so its slot is not on top. The counter reading
/// "nothing refused" here would be the instrument being wrong, not the
/// allocator (the reasons are measured on div_constants_short).
#[test]
fn a_slot_reloaded_under_another_is_refused_and_named() {
    let ir = include_str!("kernels/ir/stack_slots_crossed.ll");
    llrm_z80::compile::take_stack_refusals();
    let out = text(ir, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let refused = llrm_z80::compile::take_stack_refusals();
    assert!(refused.iter().any(|(_, why)| *why == llrm_z80::compile::StackRefusal::LoadNotOnTop), "{refused:?}\n{out}");
}

/// The other half of the control: one value kept across one call is a plain
/// `push` and `pop`, and the counter names nothing. A counter that refused
/// every slot would pass the crossed-order test alone. The word is spilled
/// from `de` and reloaded into `bc`: a word slot may come back in any pair (a
/// rule of "the same register" refused it and lost the whole saving).
#[test]
fn a_slot_that_is_a_stack_is_not_refused() {
    let ir = include_str!("kernels/ir/stack_slots_single.ll");
    llrm_z80::compile::take_stack_refusals();
    let out = text(ir, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let refused = llrm_z80::compile::take_stack_refusals();
    assert!(refused.is_empty(), "{refused:?}\n{out}");
    assert!(out.contains("push") && !out.contains("(ix)"), "{out}");
}

/// copy_runs and loop_versioning at -Os gave wrong sums (164226 for 929463):
/// `sum` kept its loop count in a byte slot stored from `e` and reloaded into
/// `d`, and `push de` / `pop de` restores `e` into `e`. A byte slot whose
/// stores and loads use different halves of a pair stays in the frame; a word
/// may come back in another pair.
#[test]
fn a_byte_stored_from_one_half_of_a_pair_and_loaded_into_the_other_stays_in_the_frame() {
    let ir = include_str!("kernels/ir/stack_slot_other_register.ll");
    llrm_z80::compile::take_stack_refusals();
    let out = text(ir, "sum", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let refused = llrm_z80::compile::take_stack_refusals();
    assert!(
        refused.iter().any(|(_, why)| *why == llrm_z80::compile::StackRefusal::DifferentLane),
        "{refused:?}\n{out}"
    );
    assert!(!out.contains("pop de\n\tdec d"), "{out}");
}

/// Spill slots were push and pop only at -Os and -Oz: at -O2 and -O3 the
/// slot read again lost `pop bc; push bc` to two IX loads, 4,000 B and 12.8M T
/// over the corpus (div_constants_short -O2 1.36M T). A slot read again is now
/// peeked at every level.
#[test]
fn spill_slots_are_push_and_pop_at_every_level() {
    let ir = include_str!("kernels/ir/stack_slots_framed.ll");
    for level in [Level::O2, Level::O3, Level::Os] {
        let out = text(ir, "f", 0, &Request::new(level)).unwrap_or_else(|why| panic!("{why}"));
        assert!(out.contains("call _g\n\tpush de") && !out.contains("ld -4(ix)"), "{level:?}\n{out}");
    }
}

/// The base pass at -O2: a function whose frame holds only spills loses the
/// frame to `push` and `pop`, at every level.
#[test]
fn a_function_with_only_spills_loses_its_frame_at_o2() {
    let ir = include_str!("kernels/ir/stack_slots_single.ll");
    let out = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(out.contains("push de") && !out.contains("(ix)"), "{out}");
}

/// Peeking every slot that is read again made logical_value -Os 152 lines
/// instead of 148 (+12 B): the frame stayed for the other slots and each moved
/// slot added its own `ld`. The function is priced with and without the peeks,
/// and the cheaper stays.
#[test]
fn peeking_slots_is_kept_only_where_it_is_smaller() {
    let out = text(include_str!("kernels/ir/stack_peek_not_always_cheaper.ll"), "main", 0, &Request::new(Level::Os))
        .expect("compiles");
    let lines = out.lines().count();
    assert!(lines <= 148, "{lines} lines\n{out}");
}

/// An `internal` function was labelled `name::`, a global: two modules that
/// each define the same `static inline` helper failed to link (`_sf_clz64` in
/// the soft-float f64 mul and div: float_bits, float_in_place_update).
#[test]
fn an_internal_function_is_a_local_label_and_an_external_one_is_global() {
    let ir = include_str!("kernels/ir/stack_slot_other_register.ll");
    let internal = text(ir, "sum", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert!(internal.starts_with("_sum:\n"), "{internal}");
    let external = text(include_str!("kernels/ir/digits.ll"), "digits", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(external.starts_with("_digits::\n"), "{external}");
}
