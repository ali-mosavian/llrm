//! The overlay's encodings against binutils `m68k-linux-gnu-as` (unpacked
//! without root under `$GCC_M68K`, default `~/scratch/gcc-m68k/root`), and its
//! effect on size.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `d1` and so on, as the view of `bits` width the instruction names.
fn register(name: &str) -> Operand {
    let root = match name {
        "d0" => reg::D0,
        "d1" => reg::D1,
        "d2" => reg::D2,
        "d3" => reg::D3,
        "d5" => reg::D5,
        "a1" => reg::A1,
        "a3" => reg::A3,
        "sp" => reg::SP,
        _ => panic!("register {name}"),
    };
    Operand::Reg(root)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Each overlay instruction's bytes, from binutils on the same text:
/// `addq.l #1,%d1` 5281, `addq.l #8,%d1` 5081 (8 is the field 0), `subq.l
/// #4,%sp` 598f, `addq.w #3,%d2` 5642, `addq.b #2,%d3` 5403, `subq.w #1,%d0`
/// 5340, `tst.b %d0` 4a00, `tst.w %d5` 4a45, `tst.l %d1` 4a81, `addq.l #2,%a1`
/// 5489, `subq.l #7,%a3` 5f8b.
#[test]
fn overlay_instructions_encode_as_binutils_does() {
    let q = |opcode, to: &str, amount| Inst { opcode, operands: vec![register(to), Operand::Imm(amount)] };
    let t = |opcode, of: &str| Inst { opcode, operands: vec![register(of)] };
    let cases = [
        (q("ADDQ32d", "d1", 1), "5281"),
        (q("ADDQ32d", "d1", 8), "5081"),
        (q("SUBQ32a", "sp", 4), "598f"),
        (q("ADDQ16d", "d2", 3), "5642"),
        (q("ADDQ8d", "d3", 2), "5403"),
        (q("SUBQ16d", "d0", 1), "5340"),
        (t("TST8d", "d0"), "4a00"),
        (t("TST16d", "d5"), "4a45"),
        (t("TST32d", "d1"), "4a81"),
        (q("ADDQ32a", "a1", 2), "5489"),
        (q("SUBQ32a", "a3", 7), "5f8b"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

fn compiled(
    ir: &str,
    options: Options,
) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    text_with(&module, "f", Cpu::M68000, options).expect("compiles")
}

/// `add.l #1` was 6 bytes and `cmpi.l #0` 6; as `addq.l` and `tst.l` they
/// are 2 each. The pin's text keeps the long forms.
#[test]
fn a_counting_loop_uses_addq_and_tst_where_the_pin_does_not() {
    let ir = "define i32 @f(i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %i1 = add i32 %i, 1\n  %c = icmp ne i32 %i1, 0\n  %d = icmp slt i32 %i1, %n\n  br i1 %d, label %l, label %x\nx:\n  ret i32 %i1\n}";
    let pinned = compiled(ir, Options::PINNED);
    let ours = compiled(ir, Options::default());
    assert!(pinned.contains("add.l\t#1,"), "{pinned}");
    assert!(ours.contains("addq.l\t#1,") && !ours.contains("add.l\t#1,"), "{ours}");
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).unwrap();
    let size = |options| {
        let items = llrm_m68k::compile::items_with(&module, "f", Cpu::M68000, options).unwrap();
        llrm_m68k::object::assemble(&items, 0).unwrap().code.len()
    };
    assert!(size(Options::default()) < size(Options::PINNED));
}
