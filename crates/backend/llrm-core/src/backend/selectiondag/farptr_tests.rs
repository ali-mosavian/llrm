//! Far and huge pointers at m16 (docs/working/selectiondag-far-builder.md): a
//! far is `build_pair(offset, selector)`, a far GEP moves the offset word
//! alone, a huge one is `HUGE_ADD`, casts take the halves apart. Before,
//! every one stopped the builder at "pointer in another address space" (7919
//! corpus blocks). The pin has no far pointers; these test the shape.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;
use super::isd::ISD;
use super::mvt::MVT;
use super::x86_lowering::x86isd;

const LAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"\n";

fn built(body: &str) -> super::dag::SelectionDAG {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{body}")).unwrap_or_else(|error| panic!("{error}"));
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

fn count(
    dag: &super::dag::SelectionDAG,
    opcode: ISD,
) -> usize {
    dag.nodes().iter().filter(|node| node.opcode == opcode).count()
}

/// A far GEP adds to the offset word and pairs it with the same selector; the
/// load reads through the far value.
#[test]
fn a_far_gep_moves_the_offset_word() {
    let dag = built(
        "define i16 @f(ptr addrspace(1) %p, i16 %i) {\n  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i\n  %v = load i16, ptr addrspace(1) %q\n  ret i16 %v\n}\n",
    );
    let pair = dag.nodes().iter().position(|node| node.opcode == ISD::BuildPair).expect("a build_pair") as u32;
    assert_eq!(dag.nodes()[pair as usize].values[0], MVT::Far.into());
    assert_eq!(count(&dag, x86isd::HUGE_ADD), 0);
    assert!(
        dag.nodes().iter().any(|node| node.opcode == ISD::Load && dag.value_type(node.ops[1]) == MVT::Far.into()),
        "a load through the far value:\n{}",
        dag.dump()
    );
}

/// A huge GEP is the whole byte displacement added by `HUGE_ADD`, not
/// arithmetic on the offset word.
#[test]
fn a_huge_gep_is_huge_add() {
    let dag = built(
        "define i8 @f(ptr addrspace(3) %p, i32 %n) {\n  %q = getelementptr i8, ptr addrspace(3) %p, i32 %n\n  %v = load i8, ptr addrspace(3) %q\n  ret i8 %v\n}\n",
    );
    assert_eq!(count(&dag, x86isd::HUGE_ADD), 1, "{}", dag.dump());
}

/// Far to near is the offset, far to huge is nothing, and a frame object's
/// near pointer becomes far with `SS` for its selector.
#[test]
fn casts_take_the_halves_apart_and_put_them_together() {
    let dag = built(
        "define i16 @f(ptr addrspace(1) %p) {\n  %a = alloca i16\n  %n = addrspacecast ptr addrspace(1) %p to ptr\n  %h = addrspacecast ptr addrspace(1) %p to ptr addrspace(3)\n  %f = addrspacecast ptr %a to ptr addrspace(1)\n  store i16 1, ptr addrspace(3) %h\n  store i16 2, ptr addrspace(1) %f\n  %v = load i16, ptr %n\n  ret i16 %v\n}\n",
    );
    let dump = dag.dump();
    assert!(dump.contains("extract_element") && dump.contains("$ss"), "{dump}");
}
