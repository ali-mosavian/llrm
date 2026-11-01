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
        eprintln!("{name}:\n{}", match text(&text_ir, name, 0, &request) {
            Ok(t) => t,
            Err(why) => format!("ERR {why}"),
        });
        let _ = compiled;
    }
}

/// `ld b, c` was dropped as an identity because `b` and `c` share the root
/// `bc`; a select then returned the wrong byte.
#[test]
fn a_copy_between_the_two_bytes_of_a_pair_is_kept() {
    use llrm_z80::compile::remove_identity_copies;
    use llrm_z80::mfunc::{Block, Inst, MFunction, Operand, Reg};
    use llrm_z80::regalloc::Class;
    use llrm_spike_mf::regalloc::Assignment;
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
