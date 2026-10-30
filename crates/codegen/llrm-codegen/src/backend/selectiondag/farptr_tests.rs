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
use super::x86isd as own;
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

/// A far global is `build_pair(offset, SEGMENT_OF)`: its offset word and the
/// selector of its segment, each once. A near global cast to far takes the
/// same selector form (DGROUP). Before, both stopped the builder (4466
/// corpus blocks).
#[test]
fn a_far_global_pairs_its_offset_with_its_segment() {
    let dag = built(
        "@far = addrspace(1) global [8 x i16] zeroinitializer\n@near = global i16 0\n\
         define i16 @f() {\n  %a = load i16, ptr addrspace(1) @far\n  %p = addrspacecast ptr @near to ptr addrspace(1)\n  %b = load i16, ptr addrspace(1) %p\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    );
    assert_eq!(count(&dag, own::SEGMENT_OF), 2, "{}", dag.dump());
    assert_eq!(count(&dag, ISD::BuildPair), 2, "{}", dag.dump());
}

/// `llrm.ia16.window` is one `LLRM_WINDOW far -> far`, and a near pointer of
/// no known object is into DGROUP (`SEGMENT_OF` with no global). Before,
/// the window call stopped the builder (34 corpus blocks), and so did the
/// cast (23).
#[test]
fn a_window_is_one_node_and_a_near_pointer_is_in_dgroup() {
    let dag = built(
        "declare ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3))\n\
         define i16 @f(ptr addrspace(3) %h, ptr %n) {\n  %w = call ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3) %h)\n  %p = addrspacecast ptr %n to ptr addrspace(1)\n  %a = load i16, ptr addrspace(1) %w\n  %b = load i16, ptr addrspace(1) %p\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    );
    assert_eq!(count(&dag, own::LLRM_WINDOW), 1, "{}", dag.dump());
    let segment = dag.nodes().iter().find(|node| node.opcode == own::SEGMENT_OF).expect("a segment");
    assert_eq!(segment.data, super::dag::NodeData::None);
}

/// A far pointer a register convention passes in two registers (`ax:dx`,
/// offset then selector) is two parts, as an argument and as a call's
/// operand. Before, the builder counted it one part against two registers
/// and refused ("argument passed in several places": `fsgs_call`).
#[test]
fn a_far_pointer_in_two_registers_is_two_parts() {
    let dag = built(
        "declare watcallcc i16 @w(ptr addrspace(1))\n\
         define watcallcc i16 @f(ptr addrspace(1) %p) {\n  %r = call watcallcc i16 @w(ptr addrspace(1) %p)\n  ret i16 %r\n}\n",
    );
    let dump = dag.dump();
    assert!(dump.contains("CopyFromReg") && dump.contains("$ax") && dump.contains("$dx"), "{dump}");
    assert!(dump.contains("build_pair"), "{dump}");
    let copies: Vec<&str> = dump.lines().filter(|line| line.contains("CopyToReg") && line.contains("$")).collect();
    assert!(copies.iter().any(|line| line.contains("$ax")) && copies.iter().any(|line| line.contains("$dx")), "{dump}");
}

/// A far pointer used in a later block is carried in two `i16` registers
/// (offset, selector), as any expanded type is. One `CopyToReg` of the whole
/// far stopped the type legalizer ("CopyToReg: expand operand", 12 programs
/// of the m16 execution run).
#[test]
fn a_far_pointer_leaves_its_block_in_two_word_registers() {
    let dag = built(
        "define i16 @f(ptr addrspace(1) %p, i1 %c) {\n  br i1 %c, label %a, label %b\na:\n  %v = load i16, ptr addrspace(1) %p\n  ret i16 %v\nb:\n  ret i16 0\n}\n",
    );
    let copies: Vec<_> = dag.nodes().iter().filter(|node| node.opcode == ISD::CopyToReg).collect();
    let far = MVT::Far.into();
    assert!(copies.iter().all(|copy| dag.value_type(copy.ops[2]) != far), "a far copied whole:\n{}", dag.dump());
    let words = copies.iter().filter(|copy| dag.value_type(copy.ops[2]) == super::mvt::EVT::I16).count();
    assert!(words >= 2, "the offset and the selector:\n{}", dag.dump());
}

/// An inline copy between far pointers steps the offset word and reads and
/// writes through `ES` (space 259). It panicked making `Constant: Far` for
/// the step (bitfield_long_long, 2 m16 programs) and, past that, would have
/// addressed the far data through `DS`.
#[test]
fn an_inline_copy_between_far_pointers_moves_the_offset_word() {
    let dag = built(
        "declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)\n\
         define void @f(ptr addrspace(1) %d, ptr addrspace(1) %s) {\n  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) %d, ptr addrspace(1) %s, i16 8, i1 false)\n  ret void\n}\n",
    );
    let spaces: Vec<u32> = dag
        .nodes()
        .iter()
        .filter(|node| matches!(node.opcode, ISD::Load | ISD::Store))
        .filter_map(|node| match node.data {
            super::dag::NodeData::Mem(info) if info.mem_vt == super::mvt::EVT::I32 => Some(info.space),
            _ => None,
        })
        .collect();
    assert!(spaces.len() >= 4, "two dword loads and two stores: {}", dag.dump());
    assert!(spaces.iter().all(|space| *space == 259), "{spaces:?}\n{}", dag.dump());
}

/// `SEGMENT_OF` names its global as an operand: the matcher morphs the node to
/// a pseudo with the operands it has and clears its data, so a global kept in
/// the data was lost and every start of a huge array read as DGROUP's
/// (tests/run/c/huge_array crashed at run time).
#[test]
fn a_segment_names_its_global_as_an_operand() {
    let dag = built(
        "@far = addrspace(1) global [8 x i16] zeroinitializer\n@near = global i16 0\n\
         define i16 @f() {\n  %a = load i16, ptr addrspace(1) @far\n  %p = addrspacecast ptr @near to ptr addrspace(1)\n  %b = load i16, ptr addrspace(1) %p\n  %c = add i16 %a, %b\n  ret i16 %c\n}\n",
    );
    let segments: Vec<_> = dag.nodes().iter().filter(|node| node.opcode == own::SEGMENT_OF).collect();
    assert_eq!(segments.len(), 2, "{}", dag.dump());
    for segment in segments {
        assert_eq!(segment.ops.len(), 1, "{}", dag.dump());
        assert_eq!(dag.opcode(segment.ops[0]), ISD::TargetGlobalAddress);
    }
}
