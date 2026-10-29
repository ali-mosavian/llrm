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
    define i8 @zero() {\n  ret i8 0\n}\n\
    define i8 @pick(i8 %x) {\nentry:\n  %c = icmp eq i8 %x, 0\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br label %join\njoin:\n  %r = phi i8 [ 10, %a ], [ 20, %b ]\n  ret i8 %r\n}";
const G_DATA: &str = ".segment \"DATA\"\ng: .byte 7\n.segment \"CODE\"\n";

fn text(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(IR).expect("parses");
    ["clr", "zero", "pick"]
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
    assert!(cmos.contains("sta (rc"), "no unindexed (zp) store:\n{cmos}");
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
