//! `cmpa.l`: a pointer compared where it lives in an address register was
//! copied to a data register first (`move.l %a0, %d1; cmp.l %a1, %d1`), two
//! bytes and four clocks a trip on the 68000 (`while (p != e) s += *p++`). The
//! pin has no `CMPA`; the rows are built from its `CMP32d*` rows and checked
//! against binutils, and the compare is made after allocation.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::cmpa;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// The bytes binutils assembles `text` to.
fn assembled(text: &str) -> Option<Vec<u8>> {
    let root = support::oracle_root()?;
    let tool = |name: &str| {
        let mut command = std::process::Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    let (object, source, flat) =
        (support::unique_path("cmpa-o"), support::unique_path("cmpa-s"), support::unique_path("cmpa-bin"));
    std::fs::write(&source, format!("{text}\n")).unwrap();
    assert!(tool("as").args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap().success(), "{text}");
    assert!(tool("objcopy").args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap().success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// The operands of a row by its source: a register, an immediate, and the
/// memory forms `(An)`, `(d16,An)` and `(d8,An,Xn)`.
fn operands(form: &str) -> Vec<Operand> {
    let (dest, base, index) = (Operand::Reg(reg::A2), Operand::Reg(reg::A3), Operand::Reg(reg::D1));
    match form {
        "r" => vec![Operand::Reg(reg::D3), dest],
        "i" => vec![Operand::Imm(0x1234_5678), dest],
        "j" => vec![dest, base],
        "p" => vec![dest, Operand::Imm(12), base],
        "f" => vec![dest, Operand::Imm(12), base, index],
        _ => unreachable!("{form}"),
    }
}

/// Every row's bytes are what the assembler makes of its text: the opmode of
/// an address destination is 111, not the pin's 010, and a wrong field would
/// run as another instruction.
#[test]
fn every_cmpa_form_encodes_as_binutils_does() {
    for form in ["r", "i", "j", "p", "f"] {
        let name = format!("CMPA32{form}");
        let name: &'static str = Box::leak(name.into_boxed_str());
        assert!(cmpa::code_of(name).is_some(), "{name} has no code");
        let inst = Inst { opcode: name, operands: operands(form) };
        let text = print(&inst).unwrap();
        assert!(text.starts_with("cmpa.l"), "{text}");
        let Some(want) = assembled(&text.replace('$', "0x")) else { return };
        assert_eq!(encode(&inst).unwrap().bytes, want, "{name}: {text}");
    }
}

/// `cmpa.w #imm, %an` is opmode 011 with a 16-bit immediate.
#[test]
fn the_word_immediate_compare_encodes_as_binutils_does() {
    let inst = Inst { opcode: "CMPA16i", operands: vec![Operand::Imm(-2), Operand::Reg(reg::A2)] };
    let text = print(&inst).unwrap();
    assert!(text.starts_with("cmpa.w"), "{text}");
    let Some(want) = assembled(&text.replace('$', "0x")) else { return };
    assert_eq!(encode(&inst).unwrap().bytes, want, "{text}");
}

const POINTERS: &str = "
define i32 @f(ptr %p, ptr %e) {
entry:
  %done = icmp eq ptr %p, %e
  br i1 %done, label %out, label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %sum, %loop ]
  %v = load i32, ptr %q
  %sum = add i32 %s, %v
  %next = getelementptr inbounds i32, ptr %q, i32 1
  %end = icmp eq ptr %next, %e
  br i1 %end, label %out, label %loop
out:
  %r = phi i32 [ 0, %entry ], [ %sum, %loop ]
  ret i32 %r
}";

/// `while (p != e) s += *p++` compared the stepped pointer through a data
/// register: `add.l (%a0)+, %d0; move.l %a0, %d1; cmp.l %a1, %d1; bne`.
#[test]
fn a_pointer_in_an_address_register_is_compared_there() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(POINTERS), "f", cpu, Options::default()).expect("compiles");
        // the step and the compare are adjacent: no copy between them
        let lines: Vec<&str> = out.lines().collect();
        let step = lines.iter().position(|line| line.contains(")+,")).expect("a post-increment");
        assert!(
            lines[step + 1].trim_start().starts_with("cmpa.l"),
            "{cpu:?}: the pointer is copied before the compare:\n{out}"
        );
    }
}
