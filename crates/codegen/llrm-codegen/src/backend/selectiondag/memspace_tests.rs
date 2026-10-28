//! Far and override accesses carry their address space (see `farptr_tests`
//! for the pointer forms).

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;
use super::isd::ISD;
use crate::testing::m16_ir;

const LAYOUT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n";

fn built(body: &str) -> super::dag::SelectionDAG {
    let module = llrm_mir::parse::module(&format!("{layout}{body}", layout = m16_ir(LAYOUT)))
        .unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .expect("f");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m16(),
        &llrm_x86_m16::M16,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .unwrap_or_else(|why| panic!("{}", why.0))
    .dag
}

/// A load through a far pointer reads through `ES` (space 259) and one through
/// `fs` its own space, so the matcher picks the segment. Without the space
/// the DAG path addressed far data through DS: wrong output.
#[test]
fn far_and_override_accesses_carry_their_address_space() {
    let dag = built(
        "define i16 @f(ptr addrspace(1) %p, ptr addrspace(257) %q) {\n  %a = load i16, ptr addrspace(1) %p\n  %b = load i16, ptr addrspace(257) %q\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    );
    let mut spaces: Vec<u32> = dag
        .nodes()
        .iter()
        .filter(|node| node.opcode == ISD::Load && dag.value_type(node.ops[1]) != super::mvt::EVT::I32)
        .filter_map(|node| match node.data {
            super::dag::NodeData::Mem(info) if info.mem_vt == super::mvt::EVT::I16 => Some(info.space),
            _ => None,
        })
        .collect();
    spaces.sort();
    assert_eq!(spaces, [0, 257, 259], "the near argument load, then the two accesses:\n{}", dag.dump());
}
