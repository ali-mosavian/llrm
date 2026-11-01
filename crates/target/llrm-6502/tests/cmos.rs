//! The 65C02 table selects STZ and BRA; the NMOS table must not. Same IR on
//! both CPUs, run on sim65 and sim65c02. Expected values are worked out by
//! hand.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n\
    @g = global i8 7\n\
    define i8 @clr(i8 %x) {\nentry:\n  store i8 0, ptr @g\n  br label %next\nnext:\n  %v = load i8, ptr @g\n  %r = add i8 %v, %x\n  ret i8 %r\n}\n\
    define void @clrp(ptr %p) {\n  store i8 0, ptr %p\n  ret void\n}\n\
    define i8 @getp(ptr %p) {\n  %v = load i8, ptr %p\n  ret i8 %v\n}\n\
    define void @putp(ptr %p, i8 %v) {\n  store i8 %v, ptr %p\n  ret void\n}\n\
    define i8 @zero() {\n  ret i8 0\n}\n\
    define i8 @pick(i8 %x) {\nentry:\n  %c = icmp eq i8 %x, 0\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br label %join\njoin:\n  %r = phi i8 [ 10, %a ], [ 20, %b ]\n  ret i8 %r\n}";
const G_DATA: &str = ".segment \"DATA\"\ng: .byte 7\n.segment \"CODE\"\n";

fn text(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(IR).expect("parses");
    ["clr", "clrp", "zero", "pick", "getp", "putp"]
        .iter()
        .map(|name| {
            function_text(&module, name, &select_function(&module, name, cpu).expect("selects")).expect("emits")
        })
        .collect()
}

/// The tests share work directories by name: one build at a time.
static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(
    target: &str,
    cpu: Cpu,
    function: &str,
) -> (i32, u64) {
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    let main = format!("  lda #5\n  sta rc2\n  jsr {function}\n");
    common::run_on(target, &format!("cmos-{function}"), &main, &format!("{}{G_DATA}", text(cpu)))
}

/// `clr(5)` zeroes `g` and returns 0 + 5; `zero()` returns 0; `pick(5)` takes
/// the `b` side, 20. Both CPUs must agree.
#[test]
fn both_cpus_run_the_same_ir_to_the_same_result() {
    for (function, want) in [("clr", 5), ("zero", 0), ("pick", 20)] {
        assert_eq!(run("sim6502", Cpu::Mos6502, function).0, want);
        assert_eq!(run("sim65c02", Cpu::W65c02, function).0, want);
    }
}

/// The NMOS text has no `stz`, `bra` or `(zp)` store; the 65C02 text has all
/// three, and the binary is smaller. A table that leaked `stz` into NMOS would
/// emit an illegal opcode.
#[test]
fn only_the_65c02_selection_uses_stz_and_bra() {
    let (nmos, cmos) = (text(Cpu::Mos6502), text(Cpu::W65c02));
    for word in ["  stz ", "  bra "] {
        assert!(!nmos.contains(word), "NMOS text has {word}:\n{nmos}");
        assert!(cmos.contains(word), "65C02 text lacks {word}:\n{cmos}");
    }
    assert!(cmos.contains("sta (rc"), "no unindexed (zp) store through a pointer:\n{cmos}");
    let (_, nmos_size) = run("sim6502", Cpu::Mos6502, "clr");
    let (_, cmos_size) = run("sim65c02", Cpu::W65c02, "clr");
    assert!(cmos_size < nmos_size, "65C02 {cmos_size} bytes, NMOS {nmos_size}");
}

/// The 65C02 text must not assemble for an NMOS part: ca65 rejects `stz`.
#[test]
fn the_65c02_text_does_not_assemble_as_nmos() {
    let (code, _) = std::panic::catch_unwind(|| run("sim6502", Cpu::W65c02, "zero")).map_or((-1, 0), |r| r);
    assert_eq!(code, -1);
}

/// A `bra` over 60 instructions is out of range (60 x 3 > 127). Unrelaxed, ca65
/// refuses it; relaxed it is a `jmp`, and a near `bra` stays.
#[test]
fn a_far_bra_becomes_jmp_and_a_near_one_stays() {
    let far = format!("  bra far\n{}far:\n  rts\n", "  lda $1234\n".repeat(60));
    let near = "  bra near\n  nop\nnear:\n  rts\n";
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cmos-bra");
    std::fs::create_dir_all(&dir).unwrap();
    let assembles = |text: &str| {
        std::fs::write(dir.join("p.s"), text).unwrap();
        std::process::Command::new("ca65")
            .args(["--cpu", "65C02", "p.s", "-o", "p.o"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success()
    };
    assert!(!assembles(&far), "the unrelaxed far bra assembled; the range check measures nothing");
    let relaxed = llrm_6502::emit::relax_bra(&far);
    assert!(relaxed.starts_with("  jmp far\n") && assembles(&relaxed));
    assert!(llrm_6502::emit::relax_bra(near).starts_with("  bra near\n"));
}

/// `a` jumps over a block `b` of 38 ops (t0 = x + 1, t1 = t0 + x, t_n = t_(n-1)
/// + (t_(n-2) ^ x), which the combiner cannot reduce): far more than 127 bytes.
/// Unrelaxed, the `bra` fails to assemble; relaxed it is a `jmp`. `far(0)`
/// takes `a` (10); `far(5)` is t19 = 159, worked out above in 8-bit arithmetic.
#[test]
fn a_far_bra_in_a_function_becomes_jmp_and_runs() {
    let mut ir = String::from(
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n\
         define i8 @far(i8 %x) {\nentry:\n  %c = icmp eq i8 %x, 0\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  %t0 = add i8 %x, 1\n",
    );
    ir.push_str("  %t1 = add i8 %t0, %x\n");
    for n in 2..20 {
        ir.push_str(&format!("  %u{n} = xor i8 %t{}, %x\n  %t{n} = add i8 %t{}, %u{n}\n", n - 2, n - 1));
    }
    ir.push_str("  br label %join\njoin:\n  %r = phi i8 [ 10, %a ], [ %t19, %b ]\n  ret i8 %r\n}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let text =
        function_text(&module, "far", &select_function(&module, "far", Cpu::W65c02).expect("selects")).expect("emits");
    assert!(!text.contains("  bra far_b3"), "the far bra survived:\n{text}");
    assert!(text.contains("  jmp far_b3"), "no jmp to the join:\n{text}");
    for (x, want) in [(0, 10), (5, 159)] {
        let main = format!("  lda #{x}\n  sta rc2\n  jsr far\n");
        assert_eq!(common::run_on("sim65c02", &format!("cmos-far-{x}"), &main, &text).0, want);
    }
}

/// A zero constant selected as `stz` was pinned to a zero-page cell: `zero()`
/// became `stz rc2; lda rc2` (5 bytes against NMOS's 3), and scmp's pointers
/// spilled for want of cells (175 bytes against 92). A zero stays a constant;
/// only one that lands in a cell is stored with `stz`.
#[test]
fn a_zero_constant_is_not_pinned_to_a_cell() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let text = |cpu| function_text(&module, "zero", &select_function(&module, "zero", cpu).expect("selects")).unwrap();
    assert_eq!(text(Cpu::W65c02), text(Cpu::Mos6502));
}

/// A byte through a pointer at offset 0 used `ldy #0; lda (p),y` on both CPUs.
/// The 65C02 has `lda (p)` and `sta (p)`: no `ldy`, two bytes and two cycles
/// less each. `g` holds 7; `putp` stores 9 into it.
#[test]
fn a_byte_through_a_pointer_uses_the_unindexed_mode_on_the_65c02() {
    let (nmos, cmos) = (text(Cpu::Mos6502), text(Cpu::W65c02));
    assert!(nmos.contains("  ldy #0\n  lda (rc2),y\n") && nmos.contains("  sta (rc2),y\n"), "{nmos}");
    assert!(cmos.contains("  lda (rc2)\n") && cmos.contains("  sta (rc2)\n"), "{cmos}");
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    let point = "  lda #<g\n  sta rc2\n  lda #>g\n  sta rc3\n";
    let get = format!("{point}  jsr getp\n");
    let put = format!("{point}  lda #9\n  sta rc4\n  jsr putp\n  lda g\n");
    for (name, main, want) in [("cmos-getp", get, 7), ("cmos-putp", put, 9)] {
        for (target, cpu) in [("sim6502", Cpu::Mos6502), ("sim65c02", Cpu::W65c02)] {
            let got = common::run_on(target, name, &main, &format!("{}{G_DATA}", text(cpu))).0;
            assert_eq!(got, want, "{name} on {target}");
        }
    }
}

const STEPS: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n\
    define i8 @countdown(i8 %n) {\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %done, label %loop\n\
    loop:\n  %i = phi i8 [ %n, %entry ], [ %j, %loop ]\n  %steps = phi i8 [ 0, %entry ], [ %s2, %loop ]\n\
      %j = sub i8 %i, 1\n  %s2 = add i8 %steps, 3\n  %c = icmp ne i8 %j, 0\n  br i1 %c, label %loop, label %done\n\
    done:\n  %r = phi i8 [ 0, %entry ], [ %s2, %loop ]\n  ret i8 %r\n}\n\
    define i8 @wrap(i8 %x) {\n  %a = add i8 %x, 1\n  %b = and i8 %a, 15\n  ret i8 %b\n}\n\
    define i8 @drop(i8 %x) {\n  %a = add i8 %x, -1\n  %b = or i8 %a, 64\n  ret i8 %b\n}\n\
    define void @bumpp(ptr %p) {\n  %v = load i8, ptr %p\n  %r = add i8 %v, 1\n  store i8 %r, ptr %p\n  ret void\n}\n\
    define void @dropp(ptr %p) {\n  %v = load i8, ptr %p\n  %r = sub i8 %v, 1\n  store i8 %r, ptr %p\n  ret void\n}";

fn steps(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(STEPS).expect("parses");
    ["countdown", "wrap", "drop", "bumpp", "dropp"]
        .iter()
        .map(|name| {
            function_text(&module, name, &select_function(&module, name, cpu).expect("selects")).expect("emits")
        })
        .collect()
}

/// `x + 1` and `x - 1` went through the accumulator, `clc; adc #1` and
/// `sec; sbc #1`, wherever the value lived: a counter in `x` was moved to `a`
/// and back. They are `inx`/`dex`, `iny`/`dey`, `inc cell`/`dec cell` in
/// place, and on the 65C02 `inc a`/`dec a` for a value that is in `a`. Both
/// the `sub 1` and the `add -1` forms, and a wrap at 255 and at 0, run on
/// sim65 and sim65c02. countdown(5) = 5 * 3 = 15; wrap(15) = 16 & 15 = 0;
/// wrap(255) = 0 & 15 = 0; wrap(6) = 7; drop(0) = 255 | 64 = 255; drop(10) = 9
/// | 64 = 73; a byte loaded through a pointer, stepped and stored back (g = 7):
/// 8 and 6.
#[test]
fn a_step_of_one_is_done_in_place() {
    for (cpu, target) in [(Cpu::Mos6502, "sim6502"), (Cpu::W65c02, "sim65c02")] {
        let text = steps(cpu);
        // in a register or a cell a step is one instruction; a value in `a` on
        // NMOS has no better than adc/sbc
        let registers = text.split(".export bumpp").next().unwrap();
        assert!(!registers.contains("adc #1") && !registers.contains("sbc #1"), "{target}:\n{text}");
        let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
        for (function, arg, want) in [
            ("countdown", 5, 15),
            ("countdown", 1, 3),
            ("wrap", 15, 0),
            ("wrap", 255, 0),
            ("wrap", 6, 7),
            ("drop", 0, 255),
            ("drop", 10, 73),
        ] {
            let main = format!("  lda #{arg}\n  sta rc2\n  jsr {function}\n");
            let got = common::run_on(target, &format!("step-{function}-{arg}"), &main, &text).0;
            assert_eq!(got, want, "{function}({arg}) on {target}");
        }
        for (function, want) in [("bumpp", 8), ("dropp", 6)] {
            let main = format!("  lda #<g\n  sta rc2\n  lda #>g\n  sta rc3\n  jsr {function}\n  lda g\n");
            let got = common::run_on(target, &format!("step-{function}"), &main, &format!("{text}{G_DATA}")).0;
            assert_eq!(got, want, "{function} on {target}");
        }
    }
    let (nmos, cmos) = (steps(Cpu::Mos6502), steps(Cpu::W65c02));
    assert!(nmos.contains("adc #1") && !nmos.contains("inc a") && !nmos.contains("dec a"), "{nmos}");
    assert!(cmos.contains("  inc a\n") && cmos.contains("  dec a\n"), "{cmos}");
    assert!(!cmos.contains("adc #1") && !cmos.contains("sbc #1"), "{cmos}");
}

const THROUGH: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n\
    define i8 @twice(i8 %x) {\n  %r = add i8 %x, %x\n  ret i8 %r\n}\n\
    define i8 @thrice(ptr %f, i8 %x) {\n  %a = call i8 %f(i8 %x)\n  %b = call i8 %f(i8 %a)\n  %c = call i8 %f(i8 %b)\n  ret i8 %c\n}";

/// An indirect call pushes the callee's address minus one low byte last: the
/// low byte went through `a` (`txa; pha`), and is `phx` on the 65C02. Three
/// calls in a row, so three return labels: `thrice(&twice, 3)` is 3 * 8 = 24.
#[test]
fn an_indirect_call_pushes_x_directly_on_the_65c02() {
    let module = llrm_mir::parse::module(THROUGH).expect("parses");
    let text = |cpu| -> String {
        ["twice", "thrice"]
            .iter()
            .map(|name| {
                function_text(&module, name, &select_function(&module, name, cpu).expect("selects")).expect("emits")
            })
            .collect()
    };
    let (nmos, cmos) = (text(Cpu::Mos6502), text(Cpu::W65c02));
    assert!(nmos.contains("  txa\n  pha\n  rts") && !nmos.contains("phx"), "{nmos}");
    assert!(cmos.contains("  phx\n  rts") && !cmos.contains("  txa\n  pha\n  rts"), "{cmos}");
    let _one = BUILD.lock().unwrap_or_else(|e| e.into_inner());
    let main = "  lda #<twice\n  sta rc2\n  lda #>twice\n  sta rc3\n  lda #3\n  sta rc4\n  jsr thrice\n";
    for (target, text) in [("sim6502", nmos), ("sim65c02", cmos)] {
        assert_eq!(common::run_on(target, "cmos-thrice", main, &text).0, 24, "{target}");
    }
}
