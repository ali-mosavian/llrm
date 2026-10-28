//! `VNCoercion`: a load a write contains, rebuilt from the written value, reads
//! what the load read. Each case runs the interpreter on the function before
//! and after the load is replaced.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, Operand};
use llrm_mir::opcode::Opcode;

use super::vncoercion::*;
use crate::testing::{parsed, printed, results};

const LITTLE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";
const BIG: &str = "target datalayout = \"E-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n";
const MEMSET: &str = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n";
const MEMCPY: &str = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\n";

fn layout(module: &Module) -> DataLayout {
    DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("parses")
}

/// The one instruction of @f that `is` accepts.
fn find(
    module: &Module,
    is: impl Fn(&Opcode) -> bool,
) -> InstId {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let found: Vec<InstId> =
        function.walk().map(|(_, inst)| inst).filter(|&inst| is(&function.instruction(inst).opcode)).collect();
    assert_eq!(found.len(), 1, "one such instruction");
    found[0]
}

fn is_load(opcode: &Opcode) -> bool {
    matches!(opcode, Opcode::Load { .. })
}

fn is_store(opcode: &Opcode) -> bool {
    matches!(opcode, Opcode::Store { .. })
}

fn is_call(opcode: &Opcode) -> bool {
    matches!(opcode, Opcode::Call(_))
}

/// What the analysis says of @f's one load against its one `dep`, and the
/// module after the load is replaced by what `get` builds; the interpreter
/// finds the same results for `inputs` before and after.
fn replaced(
    text: &str,
    dep: fn(&Opcode) -> bool,
    inputs: &[&[i128]],
) -> (Option<u64>, String) {
    let before = parsed(text);
    let layout = layout(&before);
    let load = find(&before, is_load);
    let dep_inst = find(&before, dep);
    let (_, _, function) = before.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let (load_ty, load_ptr) = (function.instruction(load).ty, function.instruction(load).operands[0]);
    let offset = match function.instruction(dep_inst).opcode {
        Opcode::Store { .. } => {
            analyze_load_from_clobbering_store(&before.context, &layout, function, load_ty, load_ptr, dep_inst)
        }
        Opcode::Load { .. } => {
            analyze_load_from_clobbering_load(&before.context, &layout, function, load_ty, load_ptr, dep_inst)
        }
        _ => {
            let mem = MemIntrinsic::of(&before, function, dep_inst).expect("a mem intrinsic");
            analyze_load_from_clobbering_mem_inst(&before.context, &layout, function, load_ty, load_ptr, mem)
        }
    };
    let Some(offset) = offset else { return (None, printed(&before)) };

    let mem = MemIntrinsic::of(&before, function, dep_inst);
    let mut after = before.clone();
    let (context, function) = after.function_mut("f").expect("@f");
    let dep = function.instruction(dep_inst);
    let source = match dep.opcode {
        Opcode::Store { .. } => Some(dep.operands[0]),
        Opcode::Load { .. } => dep.result.map(Operand::Value),
        _ => None,
    };
    let mut builder = Inserter { context, function, layout: &layout, before: load };
    let value = match (mem, source) {
        (None, Some(source)) => get_value_for_load(&mut builder, source, offset, load_ty),
        (Some(mem), _) => get_mem_inst_value_for_load(&mut builder, mem, load_ty).expect("a memset"),
        (None, None) => unreachable!("a store or a load"),
    };
    let result = function.instruction(load).result.expect("a result");
    function.replace_all_uses_with(result, value);
    function.erase(load).expect("unused");
    let text = printed(&after);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{text}");
    (Some(offset), text)
}

fn module(
    layout: &str,
    body: &str,
) -> String {
    format!("{layout}{MEMSET}{MEMCPY}@c = constant [8 x i8] c\"abcdefgh\"\n\n{body}")
}

const INPUTS: &[&[i128]] = &[&[0x1234_5678], &[-1], &[0x8000_0001]];

fn after_store(
    layout: &str,
    store_ty: &str,
    load_ty: &str,
    byte: i16,
) -> (Option<u64>, String) {
    let body = format!(
        "define {load_ty} @f({store_ty} %x) {{
b0:
  %p = alloca [8 x i8]
  store {store_ty} %x, ptr %p
  %q = getelementptr i8, ptr %p, i16 {byte}
  %v = load {load_ty}, ptr %q
  ret {load_ty} %v
}}
"
    );
    replaced(&module(layout, &body), is_store, INPUTS)
}

#[test]
fn test_a_half_of_a_word_is_shifted_and_truncated() {
    let (offset, text) = after_store(LITTLE, "i32", "i16", 2);
    assert_eq!(offset, Some(2));
    assert!(text.contains("lshr i32 %x, 16") && text.contains("trunc i32"), "{text}");
}

#[test]
fn test_the_low_half_of_a_word_needs_no_shift() {
    let (offset, text) = after_store(LITTLE, "i32", "i16", 0);
    assert_eq!(offset, Some(0));
    assert!(!text.contains("lshr") && text.contains("trunc i32"), "{text}");
}

#[test]
fn test_on_a_big_endian_target_the_low_bytes_are_the_last_ones() {
    let (offset, text) = after_store(BIG, "i32", "i16", 2);
    assert_eq!(offset, Some(2));
    assert!(!text.contains("lshr") && text.contains("trunc i32"), "{text}");
    let (_, text) = after_store(BIG, "i32", "i16", 0);
    assert!(text.contains("lshr i32 %x, 16"), "{text}");
}

#[test]
fn test_a_float_read_back_as_an_integer_is_a_bitcast() {
    let body = "define i32 @f(i32 %y) {
b0:
  %x = bitcast i32 %y to float
  %p = alloca [8 x i8]
  store float %x, ptr %p
  %v = load i32, ptr %p
  ret i32 %v
}
";
    let (offset, text) = replaced(&module(LITTLE, body), is_store, INPUTS);
    assert_eq!(offset, Some(0));
    assert!(text.contains("ret i32 %y") || text.contains("bitcast float %x to i32"), "{text}");
}

#[test]
fn test_an_integer_read_back_as_a_float_is_a_bitcast() {
    let (offset, text) = after_store(LITTLE, "i32", "float", 0);
    assert_eq!(offset, Some(0));
    assert!(text.contains("bitcast i32 %x to float"), "{text}");
}

#[test]
fn test_a_pointer_read_back_as_an_integer_is_a_ptrtoint() {
    let body = "define i16 @f(i16 %y) {
b0:
  %x = getelementptr i8, ptr @c, i16 %y
  %p = alloca [8 x i8]
  store ptr %x, ptr %p
  %v = load i16, ptr %p
  ret i16 %v
}
";
    let (offset, text) = replaced(&module(LITTLE, body), is_store, &[&[1], &[3]]);
    assert_eq!(offset, Some(0));
    assert!(text.contains("ptrtoint ptr %x to i16"), "{text}");
}

#[test]
fn test_an_integer_read_back_as_a_pointer_is_an_inttoptr() {
    let body = "define i16 @f(i16 %x) {
b0:
  %p = alloca [8 x i8]
  store i16 %x, ptr %p
  %v = load ptr, ptr %p
  %i = ptrtoint ptr %v to i16
  ret i16 %i
}
";
    let (offset, text) = replaced(&module(LITTLE, body), is_store, &[&[7], &[-1]]);
    assert_eq!(offset, Some(0));
    assert!(text.contains("inttoptr i16 %x to ptr"), "{text}");
}

#[test]
fn test_a_stored_constant_folds_into_a_constant() {
    let body = "define i16 @f() {
b0:
  %p = alloca [8 x i8]
  store i32 305419896, ptr %p
  %q = getelementptr i8, ptr %p, i16 2
  %v = load i16, ptr %q
  ret i16 %v
}
";
    let (offset, text) = replaced(&module(LITTLE, body), is_store, &[&[]]);
    assert_eq!(offset, Some(2));
    assert!(text.contains("ret i16 4660") && !text.contains("lshr") && !text.contains("trunc"), "{text}");
}

#[test]
fn test_an_earlier_load_serves_a_narrower_one() {
    let body = "define i16 @f(ptr %p) {
b0:
  %w = load i32, ptr %p
  %q = getelementptr i8, ptr %p, i16 2
  %v = load i16, ptr %q
  %t = trunc i32 %w to i16
  %s = add i16 %v, %t
  ret i16 %s
}
";
    // The dependency is the wide load, the narrow one is replaced.
    let before = parsed(&module(LITTLE, body));
    let layout = layout(&before);
    let (_, _, function) = before.functions().find(|(_, g, _)| g.name.as_deref() == Some("f")).expect("@f");
    let loads: Vec<InstId> =
        function.walk().map(|(_, i)| i).filter(|&i| is_load(&function.instruction(i).opcode)).collect();
    let (wide, narrow) = (loads[0], loads[1]);
    let load_ty = function.instruction(narrow).ty;
    let load_ptr = function.instruction(narrow).operands[0];
    assert_eq!(analyze_load_from_clobbering_load(&before.context, &layout, function, load_ty, load_ptr, wide), Some(2));
}

#[test]
fn test_a_load_the_store_does_not_cover_is_not_served() {
    // 2 bytes stored, 4 read.
    let (offset, _) = after_store(LITTLE, "i16", "i32", 0);
    assert_eq!(offset, None);
    // 4 bytes stored, the last 2 and 2 more read.
    let (offset, _) = after_store(LITTLE, "i32", "i32", 2);
    assert_eq!(offset, None);
}

#[test]
fn test_a_store_of_an_aggregate_is_not_served() {
    let body = "define i16 @f({ i16, i16 } %x) {
b0:
  %p = alloca [8 x i8]
  store { i16, i16 } %x, ptr %p
  %v = load i16, ptr %p
  ret i16 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_store, &[]).0, None);
}

#[test]
fn test_two_bases_are_not_one() {
    let body = "define i16 @f(i32 %x) {
b0:
  %p = alloca [8 x i8]
  %o = alloca [8 x i8]
  store i32 %x, ptr %p
  %v = load i16, ptr %o
  ret i16 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_store, &[]).0, None);
}

/// `underlying` goes on through a GEP with a variable index; the base
/// `GetPointerBaseWithConstantOffset` names is that GEP, so a load a constant
/// offset past it is contained in the store at it. Seen to fail with the walk
/// going on past the variable index: the offsets were unknown and nothing was
/// served.
#[test]
fn test_a_base_with_a_variable_index_is_the_gep_itself() {
    let body = "define i16 @f(i32 %x, i16 %i) {
b0:
  %p = alloca [64 x i8]
  %a = getelementptr i8, ptr %p, i16 %i
  store i32 %x, ptr %a
  %b = getelementptr i8, ptr %a, i16 2
  %v = load i16, ptr %b
  ret i16 %v
}
";
    let (offset, _) = replaced(&module(LITTLE, body), is_store, &[&[0x1234_5678, 0], &[-1, 8]]);
    assert_eq!(offset, Some(2));
}

#[test]
fn test_two_variable_indices_are_two_bases() {
    let body = "define i16 @f(i32 %x, i16 %i, i16 %j) {
b0:
  %p = alloca [64 x i8]
  %a = getelementptr i8, ptr %p, i16 %i
  %c = getelementptr i8, ptr %p, i16 %j
  store i32 %x, ptr %a
  %v = load i16, ptr %c
  ret i16 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_store, &[]).0, None);
}

#[test]
fn test_a_memset_fills_a_word_with_its_byte() {
    let body = "define i32 @f(i8 %b) {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %p, i8 %b, i16 8, i1 false)
  %q = getelementptr i8, ptr %p, i16 2
  %v = load i32, ptr %q
  ret i32 %v
}
";
    let (offset, text) = replaced(&module(LITTLE, body), is_call, &[&[0x5a], &[-1], &[0]]);
    assert_eq!(offset, Some(2));
    assert!(text.contains("zext i8 %b to i32") && text.contains("shl i32"), "{text}");
}

#[test]
fn test_a_memset_of_a_byte_fills_a_wide_load_to_the_last_byte() {
    // 3 bytes: the doubling stops at 2, one byte at a time for the third.
    let body = "define i24 @f(i8 %b) {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %p, i8 %b, i16 8, i1 false)
  %v = load i24, ptr %p
  ret i24 %v
}
";
    let (offset, _) = replaced(&module(LITTLE, body), is_call, &[&[0x5a], &[0xff], &[1]]);
    assert_eq!(offset, Some(0));
}

#[test]
fn test_a_memset_shorter_than_the_load_is_not_served() {
    let body = "define i32 @f(i8 %b) {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %p, i8 %b, i16 2, i1 false)
  %v = load i32, ptr %p
  ret i32 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_call, &[]).0, None);
}

#[test]
fn test_a_memset_of_a_variable_length_is_not_served() {
    let body = "define i32 @f(i8 %b, i16 %n) {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %p, i8 %b, i16 %n, i1 false)
  %v = load i32, ptr %p
  ret i32 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_call, &[]).0, None);
}

#[test]
fn test_a_constant_memset_byte_is_a_splat_constant() {
    let body = "define i32 @f() {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %p, i8 171, i16 8, i1 false)
  %v = load i32, ptr %p
  ret i32 %v
}
";
    let mut m = parsed(&module(LITTLE, body));
    let layout = layout(&m);
    let call = find(&m, is_call);
    let mem = {
        let (_, _, function) = m.functions().find(|(_, g, _)| g.name.as_deref() == Some("f")).expect("@f");
        MemIntrinsic::of(&m, function, call).expect("a memset")
    };
    let ty = m.context.types.int(32);
    let Some(Operand::Constant(c)) = get_constant_mem_inst_value_for_load(&mut m.context, &layout, mem, ty) else {
        panic!("a constant")
    };
    assert_eq!(m.context.get(c).kind, llrm_mir::context::ConstantKind::Int(0xabab_abab));
    // A float: until fold-poc's load folder lands, nothing folds a load of it
    // from a constant.
    let float = m.context.types.intern(llrm_mir::types::Type::Float(llrm_mir::types::FloatKind::Float));
    assert_eq!(get_constant_mem_inst_value_for_load(&mut m.context, &layout, mem, float), None);
}

#[test]
fn test_a_copy_from_constant_memory_is_not_served_until_loads_fold() {
    // LLVM serves this load from @c; until fold-poc's
    // `ConstantFoldLoadFromConst` lands the load stays.
    let body = "define i16 @f() {
b0:
  %p = alloca [8 x i8]
  call void @llvm.memcpy.p0.p0.i16(ptr %p, ptr @c, i16 8, i1 false)
  %v = load i16, ptr %p
  ret i16 %v
}
";
    assert_eq!(replaced(&module(LITTLE, body), is_call, &[]).0, None);
}
