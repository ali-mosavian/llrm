//! The 68020's `(d8,An,Xn.l*S)`: every scaled row's text and bytes against
//! binutils, for each scale. The rows are built from the pin's `(d8,An,Xn)`
//! rows with the scale field of the extension word set, so a wrong field would
//! run as another instruction or another scale.

use std::process::Command;

use llrm_m68k::dagisel::local;
use llrm_m68k::dagisel::scaled;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

mod support;

fn binutils(tool: &str) -> Option<Command> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty());
    let Some(root) = root else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return None;
    };
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn assembled(text: &str) -> Option<Vec<u8>> {
    let object = support::unique_path("scaled-o");
    let source = support::unique_path("scaled-s");
    let flat = support::unique_path("scaled-bin");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// The operands of a row, by its text: a memory operand is `(12,%a2,%d1)`; a
/// register is `%d3`, or `%a3` for the address-register mnemonics; an
/// immediate is 5.
fn operands(name: &str) -> Vec<Operand> {
    let &(_, text, shape) = local::text(name).expect("a row");
    let mnemonic = text.split(['\t', ' ']).next().unwrap();
    // the pin prints `movea` as `move`: its rows are named for the class,
    // `...af`
    let base = scaled::base_of(name).unwrap();
    let address = ["adda", "suba", "movea", "cmpa", "lea"].iter().any(|one| mnemonic.starts_with(one))
        || base.ends_with("af")
        || base.ends_with('a');
    let width = |method: &str| match () {
        _ if method.contains("ARII") => 3,
        _ if method.contains("ARID") || method.contains("PCI") => 2,
        _ => 1,
    };
    let count = shape.iter().map(|one| usize::from(one.at) + width(one.method)).max();
    let mut out = vec![Operand::Imm(100); count.unwrap_or(0)];
    for one in shape {
        let at = usize::from(one.at);
        if one.method.contains("ARII") {
            out[at] = Operand::Imm(12);
            out[at + 1] = Operand::Reg(reg::A2);
            out[at + 2] = Operand::Reg(reg::D1);
        } else if one.method.contains("PCI") {
            out[at] = Operand::Imm(8);
            out[at + 1] = Operand::Reg(reg::D1);
        } else if one.method.contains("ARID") {
            out[at] = Operand::Imm(12);
            out[at + 1] = Operand::Reg(reg::A2);
        } else if one.method.contains("ARIPI") || one.method.contains("ARIPD") || one.method.contains("ARI") {
            out[at] = Operand::Reg(reg::A2);
        } else if one.method.contains("AL") {
            // an absolute long: a small address would be assembled as a word
            out[at] = Operand::Imm(0x1234_5678);
        } else if one.method == "printOperand" {
            out[at] = Operand::Reg(if address { reg::A3 } else { reg::D3 });
        }
    }
    out
}

#[test]
fn every_scaled_row_encodes_as_binutils_does() {
    let mut checked = 0;
    for name in scaled::names() {
        let inst = Inst { opcode: name, operands: operands(name) };
        let text = print(&inst).unwrap_or_else(|why| panic!("{name}: {why}"));
        assert!(text.contains(".l*"), "{name}: no scale in `{text}`");
        let Some(want) = assembled(&text.replace('$', "0x")) else { return };
        assert_eq!(encode(&inst).unwrap().bytes, want, "{name}: {text}");
        checked += 1;
    }
    assert_eq!(checked, scaled::names().count());
    assert!(checked > 100, "{checked} rows");
}

#[test]
fn a_scaled_row_keeps_its_pin_rows_description() {
    for name in scaled::names() {
        let base = scaled::base_of(name).unwrap();
        let (scaled_code, base_code) = (
            local::code_of(name).unwrap_or_else(|| panic!("{name}: no code")),
            local::code_of(base).unwrap_or_else(|| panic!("{base}: the pin row has no code")),
        );
        let (a, b) = (local::desc(scaled_code).unwrap(), local::desc(base_code).unwrap());
        assert_eq!((a.num_operands, a.num_defs), (b.num_operands, b.num_defs), "{name}");
        assert_eq!(local::name_of(u32::from(scaled_code)), Some(name));
    }
}
