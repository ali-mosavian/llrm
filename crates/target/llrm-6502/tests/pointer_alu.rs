//! A value loaded through a pointer and used once by an ALU op is that op's
//! memory operand (bytes, words and the carry chains of wider sums): `x + *p`
//! was `ldy #0; lda (p),y; sta t; clc; txa; adc t` and is `clc; ldy #0; adc
//! (p),y` (the 65C02 with offset 0: `clc; adc (p)`), as llvm-mos does (scmp's
//! final subtract, 2026-11-09).

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n\
    define i8 @addp(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  %r = add i8 %x, %v\n  ret i8 %r\n}\n\
    define i8 @subp(ptr %p, i8 %x) {\n  %q = getelementptr i8, ptr %p, i16 3\n  %v = load i8, ptr %q\n  %r = sub i8 %x, %v\n  ret i8 %r\n}\n\
    define i8 @xorp(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  %r = xor i8 %x, %v\n  ret i8 %r\n}\n\
    define i8 @andp(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  %r = and i8 %x, %v\n  ret i8 %r\n}\n\
    define i8 @orp(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  %r = or i8 %x, %v\n  ret i8 %r\n}\n\
    define i8 @twice(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  %r = add i8 %x, %v\n  %s = add i8 %r, %v\n  ret i8 %s\n}\n\
    define i16 @addw(ptr %p, i16 %x) {\n  %q = getelementptr i16, ptr %p, i16 1\n  %v = load i16, ptr %q, align 1\n  %r = add i16 %x, %v\n  ret i16 %r\n}\n\
    define i16 @subw(ptr %p, i16 %x) {\n  %v = load i16, ptr %p, align 1\n  %r = sub i16 %x, %v\n  ret i16 %r\n}\n\
    define i16 @addz(ptr %p, i16 %x) {\n  %b = load i8, ptr %p\n  %v = zext i8 %b to i16\n  %r = add i16 %x, %v\n  ret i16 %r\n}\n\
    define i16 @subz(ptr %p, i16 %x) {\n  %b = load i8, ptr %p\n  %v = zext i8 %b to i16\n  %r = sub i16 %x, %v\n  ret i16 %r\n}\n\
    define i16 @diffz(ptr %p, ptr %q) {\n  %a = load i8, ptr %p\n  %b = load i8, ptr %q\n  %za = zext i8 %a to i16\n  %zb = zext i8 %b to i16\n  %r = sub i16 %za, %zb\n  ret i16 %r\n}\n\
    define i8 @pick(ptr %p, ptr %q) {\n  %b = load i8, ptr %q\n  %v = zext i8 %b to i16\n  %a = ptrtoint ptr %p to i16\n  %s = add i16 %a, %v\n  %t = inttoptr i16 %s to ptr\n  %r = load i8, ptr %t\n  ret i8 %r\n}\n\
    define i16 @sum32(ptr %p, i16 %unused) {\n  %q = getelementptr i8, ptr %p, i16 4\n  %r = getelementptr i8, ptr %p, i16 8\n  %a = load i32, ptr %q, align 1\n  %b = load i32, ptr %r, align 1\n  %s = add i32 %a, %b\n  %h = lshr i32 %s, 16\n  %t = trunc i32 %h to i16\n  ret i16 %t\n}\n\
    define i8 @addi(ptr %p, i8 %i, i8 %x) {\n  %z = zext i8 %i to i16\n  %q = getelementptr i8, ptr %p, i16 %z\n  %v = load i8, ptr %q\n  %r = add i8 %x, %v\n  ret i8 %r\n}\n\
    @g = global i32 0\n\
    define i32 @addg(i32 %x) {\n  %v = load i32, ptr @g\n  %r = add i32 %x, %v\n  ret i32 %r\n}\n\
    define i8 @and_all(ptr %p) {\n  %v = load i8, ptr %p\n  %r = and i8 %v, 255\n  ret i8 %r\n}\n\
    define i8 @rsub(ptr %p) {\n  %q = getelementptr i8, ptr %p, i16 3\n  %v = load i8, ptr %q\n  %r = sub i8 5, %v\n  ret i8 %r\n}\n\
    define i16 @add_const(ptr %p, i16 %unused) {\n  %v = load i16, ptr %p, align 1\n  %r = add i16 %v, 4660\n  ret i16 %r\n}\n\
    define i8 @after_store(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  store i8 1, ptr %p\n  %r = add i8 %x, %v\n  ret i8 %r\n}";
const DATA: &str = ".segment \"DATA\"\nbuf: .byte 5, 9, 200, 77, 255, 255, 1, 0, 1, 0, 0, 0\n.segment \"CODE\"\n";

fn text(
    cpu: Cpu,
    name: &str,
) -> String {
    let module = llrm_mir::parse::module(IR).expect("parses");
    function_text(&module, name, &select_function(&module, name, cpu).expect("selects")).expect("emits")
}

/// The tests share work directories by name: one build at a time.
static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(
    cpu: Cpu,
    name: &str,
    x: u8,
) -> i32 {
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    let (target, tag) = if cpu == Cpu::Mos6502 { ("sim6502", "n") } else { ("sim65c02", "c") };
    let main = format!("  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n  lda #{x}\n  jsr {name}\n");
    common::run_on(target, &format!("pointer-alu-{name}-{x}-{tag}"), &main, &format!("{}{DATA}", text(cpu, name))).0
}

/// No copy of the loaded byte survives: the op reads `(rc2)` itself, on both
/// CPUs.
#[test]
fn a_loaded_byte_is_read_by_the_op_that_uses_it() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        for (name, op) in [("addp", "adc"), ("subp", "sbc"), ("xorp", "eor"), ("andp", "and"), ("orp", "ora")] {
            let text = text(cpu, name);
            assert!(text.contains(&format!("  {op} (rc2)")), "{cpu:?} {name}:\n{text}");
            assert!(!text.contains("sta rc"), "{cpu:?} {name} keeps a copy:\n{text}");
        }
    }
}

/// The 65C02 has `(zp)` for offset 0: no `ldy`; the NMOS part needs the index,
/// and the offset 3 needs it on both.
#[test]
fn only_a_zero_offset_on_a_65c02_drops_the_index() {
    assert!(!text(Cpu::W65c02, "addp").contains("ldy"), "{}", text(Cpu::W65c02, "addp"));
    assert!(text(Cpu::Mos6502, "addp").contains("ldy #0"), "{}", text(Cpu::Mos6502, "addp"));
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        assert!(text(cpu, "subp").contains("ldy #3"), "{cpu:?}:\n{}", text(cpu, "subp"));
    }
}

/// A byte read twice stays one load (folding it would read the memory twice); a
/// read that the source orders before a store stays before it.
#[test]
fn a_load_with_two_uses_or_a_store_before_its_user_is_not_folded() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let twice = text(cpu, "twice");
        assert_eq!(twice.matches("(rc2)").count(), 1, "{cpu:?}:\n{twice}");
        let stored = text(cpu, "after_store");
        let (read, store) = (stored.find("adc (rc2)").unwrap_or(usize::MAX), stored.find("sta (rc2)").unwrap_or(0));
        assert!(read < store, "{cpu:?}: the read must come before the store:\n{stored}");
    }
}

/// Every op, on both CPUs, against the bytes in memory: buf[0] = 5, buf[3] =
/// 77.
#[test]
fn the_folded_ops_compute_what_the_loads_did() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        for x in [0u8, 77, 255] {
            assert_eq!(run(cpu, "addp", x), i32::from(x.wrapping_add(5)), "{cpu:?} {x} + 5");
            assert_eq!(run(cpu, "subp", x), i32::from(x.wrapping_sub(77)), "{cpu:?} {x} - 77");
            assert_eq!(run(cpu, "xorp", x), i32::from(x ^ 5), "{cpu:?} {x} ^ 5");
            assert_eq!(run(cpu, "andp", x), i32::from(x & 5), "{cpu:?} {x} & 5");
            assert_eq!(run(cpu, "orp", x), i32::from(x | 5), "{cpu:?} {x} | 5");
            assert_eq!(run(cpu, "twice", x), i32::from(x.wrapping_add(10)), "{cpu:?} {x} + 5 + 5");
            assert_eq!(run(cpu, "after_store", x), i32::from(x.wrapping_add(5)), "{cpu:?} {x} + old 5");
        }
    }
}

/// A word run: `x` in rc4:rc5, the result's low byte in A and its high byte in
/// X.
fn run_word(
    cpu: Cpu,
    name: &str,
    x: u16,
    high: bool,
) -> i32 {
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    let (target, tag) = if cpu == Cpu::Mos6502 { ("sim6502", "n") } else { ("sim65c02", "c") };
    let main = format!(
        "  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n  lda #<{x}\n  sta rc4\n  lda #>{x}\n  sta rc5\n  jsr {name}\n{}",
        if high { "  txa\n" } else { "" }
    );
    common::run_on(
        target,
        &format!("pointer-alu-{name}-{x}-{high}-{tag}"),
        &main,
        &format!("{}{DATA}", text(cpu, name)),
    )
    .0
}

/// Words and a 32-bit sum read the memory through `(rc2),y` lane by lane, with
/// no copy of the loaded bytes, on both CPUs.
#[test]
fn a_loaded_word_is_read_lane_by_lane_by_the_op_that_uses_it() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        for (name, op, lanes) in [("addw", "adc", 2), ("subw", "sbc", 2), ("sum32", "adc", 4)] {
            let text = text(cpu, name);
            let reads = text.matches(&format!("  {op} (rc2)")).count();
            assert!(reads >= lanes, "{cpu:?} {name}: {reads} reads, want {lanes}:\n{text}");
            // `sum32` adds two loaded values: one of them is still loaded, the
            // other is the operand
            assert!(
                name == "sum32" || !text.contains("lda (rc2)"),
                "{cpu:?} {name} still loads a lane into A first:\n{text}"
            );
        }
    }
}

/// The word ops, with carries and borrows across the lanes, against the bytes
/// in memory: the word at buf+2 is 77*256+200 = 19912, the word at buf is
/// 9*256+5 = 2309, the byte at buf is 5, and buf+4 and buf+8 hold 0x0001FFFF
/// and 1.
#[test]
fn the_word_ops_compute_what_the_loads_did() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        for x in [255u16, 0xFFFF, 40000] {
            let sum = x.wrapping_add(19912);
            assert_eq!(run_word(cpu, "addw", x, false), i32::from(sum as u8), "{cpu:?} {x} + 19912, low");
            assert_eq!(run_word(cpu, "addw", x, true), i32::from((sum >> 8) as u8), "{cpu:?} {x} + 19912, high");
            let diff = x.wrapping_sub(2309);
            assert_eq!(run_word(cpu, "subw", x, false), i32::from(diff as u8), "{cpu:?} {x} - 2309, low");
            assert_eq!(run_word(cpu, "subw", x, true), i32::from((diff >> 8) as u8), "{cpu:?} {x} - 2309, high");
            let plus = x.wrapping_add(5);
            assert_eq!(run_word(cpu, "addz", x, false), i32::from(plus as u8), "{cpu:?} {x} + 5, low");
            assert_eq!(run_word(cpu, "addz", x, true), i32::from((plus >> 8) as u8), "{cpu:?} {x} + 5, high");
        }
        assert_eq!(run_word(cpu, "sum32", 0, false), 2, "{cpu:?} 0x0001FFFF + 1 carries into bit 17");
    }
}

/// A byte at `p + i` is read as `adc (p),y` with `i` in `y`: the pointer is not
/// built by an add first (`clc; lda rc2; adc i; sta rc4; ...`), which the
/// offset form did before the indexed one existed (dotx and fletcher,
/// 2026-11-09).
#[test]
fn a_byte_at_a_pointer_plus_an_index_is_read_through_y() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let text = text(cpu, "addi");
        assert!(text.contains("adc (rc2),y"), "{cpu:?}:\n{text}");
        assert!(!text.contains("adc rc"), "{cpu:?} builds the pointer:\n{text}");
    }
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let (target, tag) = if cpu == Cpu::Mos6502 { ("sim6502", "n") } else { ("sim65c02", "c") };
        for (i, x) in [(0u8, 3u8), (1, 250), (3, 77)] {
            let main = format!("  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n  ldx #{x}\n  lda #{i}\n  jsr addi\n");
            let got = common::run_on(
                target,
                &format!("pointer-alu-addi-{i}-{x}-{tag}"),
                &main,
                &format!("{}{DATA}", text(cpu, "addi")),
            )
            .0;
            let buf = [5u8, 9, 200, 77];
            assert_eq!(got, i32::from(x.wrapping_add(buf[i as usize])), "{cpu:?} {x} + buf[{i}]");
        }
    }
}

/// A load from a global stays an absolute `lda g`: its address is not built as
/// a pointer pair to fold the load (`byval_inline` grew 16 B and a push/pop
/// pair of rc10/rc11 when it was).
#[test]
fn a_global_is_not_folded_as_a_pointer() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let text = text(cpu, "addg");
        assert!(!text.contains("),y") && !text.contains("lda #<"), "{cpu:?}:\n{text}");
    }
}

/// A constant first operand keeps its own forms: `x & 255` is the load alone
/// (no `and`), `5 - *p` and `*p + 0x1234` run as the ordinary ops on a loaded
/// cell. Folding them put the constant in `a` first and grew compiler-rt's
/// `__unorddf2` (2026-11-09).
#[test]
fn a_constant_first_operand_keeps_the_ordinary_forms() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let asm = text(cpu, "and_all");
        assert!(!asm.contains("  and"), "{cpu:?}:\n{asm}");
        for name in ["rsub", "add_const"] {
            let asm = text(cpu, name);
            assert!(!asm.contains("sbc (rc") && !asm.contains("adc (rc"), "{cpu:?} {name}:\n{asm}");
        }
    }
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let (target, tag) = if cpu == Cpu::Mos6502 { ("sim6502", "n") } else { ("sim65c02", "c") };
        for (name, want) in
            [("and_all", 5), ("rsub", 5u8.wrapping_sub(77).into()), ("add_const", (2309u16 + 4660) as i32 & 0xFF)]
        {
            let main = "  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n";
            let got = common::run_on(
                target,
                &format!("pointer-alu-{name}-{tag}"),
                &format!("{main}  jsr {name}\n"),
                &format!("{}{DATA}", text(cpu, name)),
            )
            .0;
            assert_eq!(got, want, "{cpu:?} {name}");
        }
    }
}

/// `wide_shift_by_one`: `(u64)a << 32 | v[j]` folded the load into `ora (p),y`
/// with `a` holding a zero (`ldx #0; txa; ora (rc6),y`), where the load was
/// `lda (rc6),y`; one instruction more a trip, +196 instructions (2026-11-10).
#[test]
fn an_or_or_xor_of_a_zero_accumulator_through_a_pointer_is_the_load() {
    use llrm_6502::asm::Asm;
    use llrm_6502::peephole::drop_known_loads;
    for (operand, want) in [("(rc6),y", "(rc6),y"), ("(rc6)", "(rc6)")] {
        for op in ["ora", "eor"] {
            let mut asm = Asm::parse(&format!("f:\n  ldx #0\n  txa\n  ldy #2\n  {op} {operand}\n  sta rc14\n  rts\n"));
            drop_known_loads(&mut asm);
            let text = asm.text();
            assert!(text.contains(&format!("  lda {want}\n")) && !text.contains(op), "{op} {operand}:\n{text}");
        }
    }
    let live = "f:\n  lda rc2\n  ldy #2\n  ora (rc6),y\n  sta rc14\n  rts\n";
    let mut asm = Asm::parse(live);
    drop_known_loads(&mut asm);
    assert_eq!(asm.text(), live, "a is not zero");
}

/// A word op whose second operand is a zero-extended loaded byte runs on the low
/// bytes with the load as the operand and `#0` for the high byte: `a - *p` was
/// `lda (p); sta t; sec; lda a; sbc t; ...` (17 B), `*p - *q` (scmp's final
/// subtract) 19 B, 13 B now (2026-11-10).
#[test]
fn a_zero_extended_loaded_byte_is_the_operand_of_a_word_subtract() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let sub = text(cpu, "subz");
        assert!(sub.contains("  sbc (rc2)"), "{cpu:?} subz:\n{sub}");
        assert!(!sub.contains("lda (rc2)"), "{cpu:?} subz loads the byte first:\n{sub}");
        let diff = text(cpu, "diffz");
        assert!(diff.contains("  sbc (rc4)"), "{cpu:?} diffz:\n{diff}");
        assert!(diff.matches("sta rc").count() == 0, "{cpu:?} diffz keeps a copy of a byte:\n{diff}");
        // `buf[0]` is 5: a word minus it borrows through both bytes
        for x in [0u16, 5, 255, 256, 0xFFFF] {
            let want = x.wrapping_sub(5);
            assert_eq!(run_word(cpu, "subz", x, false), i32::from(want as u8), "{cpu:?} {x} - 5, low");
            assert_eq!(run_word(cpu, "subz", x, true), i32::from((want >> 8) as u8), "{cpu:?} {x} - 5, high");
        }
    }
}

/// The word is an address: `p + zext(*q)` read by a load stays the word add that
/// `(p),y` takes, not a byte add and a carry.
#[test]
fn a_zero_extended_loaded_byte_added_to_a_pointer_stays_an_index() {
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        let text = text(cpu, "pick");
        assert!(!text.contains("adc #0"), "{cpu:?} pick splits the pointer add:\n{text}");
    }
}
