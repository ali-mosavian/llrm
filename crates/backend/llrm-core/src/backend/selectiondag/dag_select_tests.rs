use std::collections::HashMap;

use llrm_dagisel::generated::x86_i486::{MACHINE_OPCODES, TABLE};
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_lir::registers::RegId;
use llrm_mir::datalayout::DataLayout;
use llrm_target::Target;

use super::actions::{Features, TargetLowering};
use super::builder::{BuilderTarget, build_block};
use super::dag_select::select_block;
use super::emit::{EmitContext, FrameMap};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::MVT;
use super::opcodes::OpcodeMap;
use super::schedule::SchedInfo;
use crate::backend::isel::Parameter;
use crate::model::ir::{Addr, Mem, Space};

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        llrm_dagisel::instr::desc(opcode as u16).map_or(0, |d| usize::from(d.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        llrm_dagisel::instr::desc(opcode as u16).map_or(&[], |d| d.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        llrm_dagisel::instr::desc(opcode as u16).is_some_and(|d| d.is_call())
    }
}

/// Fixed object `-k` is the k-th stack-passed parameter.
struct Frame(Vec<i64>);

impl FrameMap for Frame {
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem> {
        let disp = *self.0.get(usize::try_from(-index - 1).ok()?)?;
        Some(Mem { through: RegId::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, disp)), width) })
    }
}

fn hooks() -> X86Hooks {
    X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32,
    })
}

/// The DAG path's result for each block of `@f`, or why it refused.
fn through(text: &str) -> Result<Vec<Vec<String>>, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout =
        module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).expect("layout"));
    let target = BuilderTarget::m32();
    let machine = llrm_x86_m32::M32;
    let map = OpcodeMap::new(llrm_instr::parse(&machine.forms_text(), machine.dialect()).expect("x86.instr"));
    let names: HashMap<u16, &str> = MACHINE_OPCODES.iter().copied().collect();
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let selector = Selector::new(TABLE);
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &machine).expect("a convention");
    let cells =
        convention.parameters.iter().filter_map(|p| if let Parameter::Cell(disp) = p { Some(*disp) } else { None });
    let frame = Frame(cells.collect());
    let ctx = EmitContext { frame: &frame, map: &map, names: &names };
    let mut info = FunctionLoweringInfo::new(function);
    let mut out = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(&module, function, &layout, &target, &machine, &convention, &mut info, block)
            .map_err(|r| r.0.to_owned())?;
        let before = built.dag.dump();
        let blocks = select_block(&mut built.dag, &tli, &selector, &mut hooks(), &Info, &ctx)
            .map_err(|r| format!("{r:?}\nbuilt:\n{before}\nafter:\n{}", built.dag.dump()))?;
        for b in blocks {
            out.push(
                b.insns
                    .iter()
                    .map(|i| {
                        let w = i.what.as_ref().unwrap();
                        format!("{:?} := {} {:?}", w.dests, w.name.as_deref().unwrap_or(""), w.sources)
                    })
                    .collect(),
            );
        }
    }
    Ok(out)
}

#[test]
fn a_sum_of_two_arguments_goes_through() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %a, i32 %b) {\nentry:\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";
    let blocks = through(text).unwrap_or_else(|e| panic!("refused: {e}"));
    assert_eq!(blocks.len(), 1, "{blocks:#?}");
    let names: Vec<&str> =
        blocks[0].iter().map(|line| line.split(" := ").nth(1).unwrap().split(' ').next().unwrap()).collect();
    assert_eq!(names, ["mov", "add", "mov"], "{blocks:#?}");
    assert!(blocks[0][1].contains("Mem"), "the second load folds into the add: {blocks:#?}");
    assert!(blocks[0][2].starts_with("[Reg(Reg { register: EAX"), "the sum is returned in EAX: {blocks:#?}");
}
