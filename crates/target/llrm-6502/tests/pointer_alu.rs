//! A byte loaded through a pointer and used once by an ALU op is that op's memory operand: `x + *p` was
//! `ldy #0; lda (p),y; sta t; clc; txa; adc t` and is `clc; ldy #0; adc (p),y` (the 65C02 with offset 0: `clc; adc (p)`),
//! as llvm-mos does (scmp's final subtract, 2026-11-09).

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
    define i8 @after_store(ptr %p, i8 %x) {\n  %v = load i8, ptr %p\n  store i8 1, ptr %p\n  %r = add i8 %x, %v\n  ret i8 %r\n}";
const DATA: &str = ".segment \"DATA\"\nbuf: .byte 5, 9, 200, 77\n.segment \"CODE\"\n";

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

/// No copy of the loaded byte survives: the op reads `(rc2)` itself, on both CPUs.
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

/// The 65C02 has `(zp)` for offset 0: no `ldy`; the NMOS part needs the index, and the offset 3 needs it on both.
#[test]
fn only_a_zero_offset_on_a_65c02_drops_the_index() {
    assert!(!text(Cpu::W65c02, "addp").contains("ldy"), "{}", text(Cpu::W65c02, "addp"));
    assert!(text(Cpu::Mos6502, "addp").contains("ldy #0"), "{}", text(Cpu::Mos6502, "addp"));
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        assert!(text(cpu, "subp").contains("ldy #3"), "{cpu:?}:\n{}", text(cpu, "subp"));
    }
}

/// A byte read twice stays one load (folding it would read the memory twice); a read that the source orders before a store
/// stays before it.
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

/// Every op, on both CPUs, against the bytes in memory: buf[0] = 5, buf[3] = 77.
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
