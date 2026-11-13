//! `InstrEmitter::EmitCopyFromReg` uses a virtual register directly. The emitter
//! copied a parameter loaded in the entry into a new register at the top of the
//! loop, so `xor %a, 77777` read a register defined inside the loop and
//! MachineLICM (which the pin runs on `f`: the xor goes to the preheader) found
//! nothing invariant.

use llrm_m68k::dagisel::function::select_function;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{Shape, emit_with};
use llrm_spike_mf::mfunc::{Kind, Operand, Reg};

#[test]
fn a_value_of_an_earlier_block_is_read_where_it_was_defined() {
    let mut module = llrm_mir::parse::module(include_str!("fixtures/licm_invariant.ll")).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let blocks = select_function(&module, "f", Cpu::M68000).expect("selects");
    let function = emit_with("f", &blocks, Shape::PIN).expect("emits");
    let block_of = |register: u32| {
        function.blocks.iter().position(|block| {
            block.insts.iter().any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(v)) if *v == register)))
        })
    };
    let (at, xor) = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(at, block)| block.insts.iter().map(move |inst| (at, inst)))
        .find(|(_, inst)| inst.kind == Kind::Op("XOR32di"))
        .expect("the xor");
    let Some(Operand::Use(Reg::Virt(read))) = xor.operands.iter().find(|o| matches!(o, Operand::Use(_))) else {
        panic!("the xor reads no register");
    };
    assert_ne!(block_of(*read), Some(at), "the xor reads v{read}, which block {at} defines");
}
