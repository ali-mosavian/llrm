use crate::mir::emit;
use crate::model::{Block, Dialect, Function, FunctionLinkage, Instruction, Module, Op, Operand, Program, RuntimeProfile, Terminator, TerminatorKind, Type, TypeKind, Value};

fn program(function: Function) -> Program {
    let mut integer = Type::new(1, "integer", TypeKind::Integer, 2);
    integer.signed = Some(true);
    let types = vec![Type::new(0, "void", TypeKind::Void, 0), integer];
    Program::new(Dialect::Qb45, RuntimeProfile::Qb45, vec![Module::new(1, "m", types, vec![function])])
}

/// `SUB`'s shape: `r = a - b`, returned.
fn difference() -> Function {
    let values = vec![Value { id: 1, r#type: 1 }, Value { id: 2, r#type: 1 }, Value { id: 3, r#type: 1 }];
    let sub = Instruction::new(1, Op::Sub, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2)]);
    let block = Block::new(1, vec![sub], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(3)], Vec::new()));
    let mut function = Function::new(1, "DIFF%", 1, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1, 2];
    function
}

#[test]
fn a_function_becomes_its_llvm_ir() {
    let emitted = emit(&program(difference())).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    // With no ABI of its own, a procedure is far and C's.
    assert!(text.contains("define i16 @\"DIFF%\"(i16 %0, i16 %1) addrspace(1) {\nb1:\n  %2 = sub i16 %0, %1\n  ret i16 %2\n}\n"), "{text}");
}

/// A call carries its callee's calling convention, and a far callee lives
/// in code address space 1: BASIC's pops its own arguments, pushed left to
/// right; a near C one is in address space 0.
#[test]
fn a_call_repeats_its_callees_convention() {
    use crate::model::{CallAbi, CallDistance, FloatReturn, ProcedureAbi, StackCleanup};
    let mut function = difference();
    function.abi = Some(ProcedureAbi { cleanup: StackCleanup::Callee, distance: CallDistance::Far, parameter_bytes: 4, float_return: FloatReturn::Pointer, variadic: false });
    let mut call = Instruction::new(2, Op::Call, vec![4], vec![Operand::value_ref(3), Operand::value_ref(1)]);
    call.callee = Some("B$NEAR".to_owned());
    function.values.push(Value { id: 4, r#type: 1 });
    function.blocks[0].instructions.push(call);
    function.blocks[0].terminator.operands = vec![Operand::value_ref(4)];
    let site = |order| CallAbi { instruction: 2, order, cleanup: StackCleanup::Caller, distance: CallDistance::Near, callee: None, float_return: FloatReturn::Register };
    function.calls = vec![site(vec![1, 0])];
    let emitted = emit(&program(function.clone())).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("define cc1000 i16 @\"DIFF%\"(i16 %0, i16 %1) addrspace(1) {"), "{text}");
    assert!(text.contains("call i16 @llrm.qb.B$NEAR(i16 %2, i16 %0)"), "{text}");
    assert!(text.contains("declare i16 @llrm.qb.B$NEAR(i16, i16)\n"), "{text}");

    // Pushed in another order, its arguments are passed so the convention
    // pushes them as the site does: PDS's B$HARY pushes its subscripts in
    // record order and was refused.
    function.calls = vec![site(vec![0, 1])];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("call i16 @llrm.qb.B$NEAR(i16 %0, i16 %2)"), "{text}");
}

/// A call answering in two registers returns one aggregate, each result a
/// field of it: B$HARY's offset and selector, of which the second was lost.
#[test]
fn a_call_with_two_results_returns_an_aggregate() {
    let mut function = difference();
    let mut call = Instruction::new(2, Op::Call, vec![4, 5], vec![Operand::value_ref(3)]);
    call.callee = Some("B$PAIR".to_owned());
    function.values.extend([Value { id: 4, r#type: 1 }, Value { id: 5, r#type: 1 }]);
    let add = Instruction::new(3, Op::Add, vec![6], vec![Operand::value_ref(4), Operand::value_ref(5)]);
    function.values.push(Value { id: 6, r#type: 1 });
    function.blocks[0].instructions.extend([call, add]);
    function.blocks[0].terminator.operands = vec![Operand::value_ref(6)];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("= call addrspace(1) { i16, i16 } @llrm.qb.B$PAIR(i16 %2)"), "{text}");
    assert!(text.contains("extractvalue { i16, i16 } %3, 0") && text.contains("extractvalue { i16, i16 } %3, 1"), "{text}");
}

/// A refused internal function was left `declare internal`, which LLVM
/// rejects; as LLVM's `deleteBody` does, it becomes external.
#[test]
fn a_refused_function_is_an_external_declaration() {
    let mut function = difference();
    function.linkage = FunctionLinkage::Internal;
    function.external_entries = vec![function.entry + 1];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, [("DIFF%".to_owned(), "an alternate entry".to_owned())]);
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
}

/// BC's string layout: a far payload whose word 1 is its own near offset,
/// a segment word, a near descriptor with the payload's offset and the
/// segment word's address, and a far pointer to the descriptor.
#[test]
fn relocated_data_becomes_pointers_in_its_initializer() {
    use crate::model::{AddressKind, DataObject, DataRelocation};
    let relocation = |at, target, addend, address| DataRelocation { at, target, addend, address, code: false };
    let mut payload = DataObject::new(3, "payload", vec![0, 0, 0, 0, 2, 0, 72, 73]);
    (payload.address, payload.readonly, payload.relocations) = (AddressKind::Far, true, vec![relocation(2, 3, 4, AddressKind::Near)]);
    let mut segment = DataObject::new(4, "segment", vec![0, 0]);
    segment.relocations = vec![relocation(0, 3, 0, AddressKind::Segment)];
    let mut descriptor = DataObject::new(5, "descriptor", vec![0, 0, 0, 0]);
    descriptor.relocations = vec![relocation(0, 3, 2, AddressKind::Near), relocation(2, 4, 0, AddressKind::Near)];
    let mut far = DataObject::new(6, "far", vec![0, 0, 0, 0]);
    far.relocations = vec![relocation(0, 5, 0, AddressKind::Far)];
    let mut program = program(difference());
    program.modules[0].data = vec![payload, segment, descriptor, far];

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let globals: Vec<&str> = text.lines().filter(|line| line.starts_with('@')).collect();
    assert_eq!(
        globals,
        [
            "@payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @payload, i16 4) to i16), [4 x i8] c\"\\02\\00HI\" }>",
            "@segment = internal global ptr addrspace(2) addrspacecast (ptr addrspace(1) @payload to ptr addrspace(2))",
            "@descriptor = internal global <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @payload, i16 2) to i16), ptr @segment }>",
            "@far = internal global ptr addrspace(1) addrspacecast (ptr @descriptor to ptr addrspace(1))",
        ]
    );
}

/// `DIM a(1 TO 3, 0 TO 4)`, column-major: `a(i, j)` is element
/// `j * 3 + (i - 1)`.
#[test]
fn an_array_element_is_its_linear_index_into_the_array() {
    use crate::model::{ArrayElement, Place, Storage};
    let values = vec![Value { id: 1, r#type: 1 }, Value { id: 2, r#type: 1 }, Value { id: 3, r#type: 1 }];
    let element = Operand::ArrayElement(ArrayElement { place: 1, indices: vec![Operand::value_ref(1), Operand::value_ref(2)] });
    let load = Instruction::new(1, Op::Load, vec![3], vec![element]);
    let block = Block::new(1, vec![load], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(3)], Vec::new()));
    let places = vec![Place::new(1, "A", 2, Storage::Local, -30)];
    let mut function = Function::new(1, "AT%", 1, values, places, vec![block], 1);
    function.parameters = vec![1, 2];
    let mut program = program(function);
    let mut array = Type::new(2, "array", TypeKind::Array, 30);
    (array.element, array.rank, array.bounds) = (Some(1), 2, vec![(1, 3), (0, 4)]);
    program.modules[0].types.push(array);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = "  %2 = alloca [30 x i8]\n  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 30, i1 false)\n  %3 = sub i16 %1, 0\n  %4 = sub i16 %0, 1\n  %5 = mul i16 %3, 3\n  %6 = add i16 %5, %4\n  %7 = getelementptr inbounds i16, ptr %2, i16 %6\n  %8 = load i16, ptr %7, !tbaa !2\n  ret i16 %8\n";
    assert!(text.contains(body), "{text}");
}

/// A far pointer taken apart and put back together.
#[test]
fn a_far_pointer_is_a_segment_and_an_offset() {
    use crate::model::AddressKind;
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 1 }, Value { id: 3, r#type: 1 }, Value { id: 4, r#type: 2 }];
    let instructions = vec![
        Instruction::new(1, Op::PointerSegment, vec![2], vec![Operand::value_ref(1)]),
        Instruction::new(2, Op::PointerOffset, vec![3], vec![Operand::value_ref(1)]),
        Instruction::new(3, Op::Concat, vec![4], vec![Operand::value_ref(2), Operand::value_ref(3)]),
    ];
    let block = Block::new(1, instructions, Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(4)], Vec::new()));
    let mut function = Function::new(1, "JOIN", 2, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    let mut far = Type::new(2, "far", TypeKind::Pointer, 4);
    far.address = AddressKind::Far;
    program.modules[0].types.push(far);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = "  %1 = addrspacecast ptr addrspace(1) %0 to ptr addrspace(2)\n  %2 = ptrtoint ptr addrspace(2) %1 to i16\n  %3 = ptrtoint ptr addrspace(1) %0 to i16\n  %4 = inttoptr i16 %2 to ptr addrspace(2)\n  %5 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)\n  %6 = getelementptr i8, ptr addrspace(1) %5, i16 %3\n  ret ptr addrspace(1) %6\n";
    assert!(text.contains(body), "{text}");
}

/// A runtime's variable comes with no bytes; `b$seg` was `[0 x i8]`, so no
/// load of it was safe to hoist. A place over it says how far it reaches.
#[test]
fn an_external_object_is_as_large_as_its_places() {
    use crate::model::{DataLinkage, DataObject, Place, Storage};
    let values = vec![Value { id: 1, r#type: 1 }];
    let load = Instruction::new(1, Op::Load, vec![1], vec![Operand::place_ref(1)]);
    let block = Block::new(1, vec![load], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(1)], Vec::new()));
    let mut place = Place::new(1, "b$seg", 1, Storage::External, 0);
    place.symbol = 3;
    let function = Function::new(1, "SEG%", 1, values, vec![place], vec![block], 1);
    let mut program = program(function);
    let mut segment = DataObject::new(3, "b$seg", Vec::new());
    segment.linkage = DataLinkage::External;
    program.modules[0].data = vec![segment];

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("@b$seg = external global [2 x i8]"), "{text}");
}

/// plasma reloaded each array's segment word every iteration: HIR's
/// promise that a far allocation is disjoint from every place was dropped.
/// An allocation's element and a word read through a place's own address
/// carry the `!tbaa` tags that say so.
#[test]
fn an_allocation_and_a_place_are_tagged_apart() {
    use crate::model::{AddressKind, IndirectPlace, Place, Storage};
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 3 }, Value { id: 3, r#type: 1 }];
    let indirect = |base, offset, allocation| Operand::IndirectPlace(IndirectPlace { base, offset, r#type: 1, volatile: false, origin: None, allocation });
    let instructions = vec![
        Instruction::new(1, Op::Address, vec![2], vec![Operand::place_ref(1)]),
        Instruction::new(2, Op::Load, vec![3], vec![indirect(2, 2, None)]),
        Instruction::new(3, Op::Store, vec![], vec![indirect(1, 0, Some(1)), Operand::value_ref(3)]),
    ];
    let block = Block::new(1, instructions, Terminator::new(TerminatorKind::Return, Vec::new(), Vec::new()));
    let places = vec![Place::new(1, "D", 4, Storage::Local, -4)];
    let mut function = Function::new(1, "FILL", 0, values, places, vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    let mut far = Type::new(2, "far", TypeKind::Pointer, 4);
    far.address = AddressKind::Far;
    let mut descriptor = Type::new(4, "descriptor", TypeKind::Array, 4);
    (descriptor.element, descriptor.rank, descriptor.bounds) = (Some(1), 1, vec![(0, 1)]);
    program.modules[0].types.extend([far, Type::new(3, "near", TypeKind::Pointer, 2), descriptor]);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %3 = load i16, ptr %2, !tbaa !2\n  store i16 %3, ptr addrspace(1) %0, !tbaa !4\n"), "{text}");
    assert!(text.contains("!1 = !{!\"place\", !0, i64 0}\n") && text.contains("!3 = !{!\"allocation\", !0, i64 0}\n"), "{text}");
}

/// A far pointer advanced by a displacement moves its offset alone: a GEP
/// at the far space's 16-bit index width.
#[test]
fn a_far_pointer_offset_is_a_gep_at_the_index_width() {
    use crate::model::AddressKind;
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 2 }];
    let advance = Instruction::new(1, Op::PtrOffset, vec![2], vec![Operand::value_ref(1), Operand::constant(1, 6)]);
    let block = Block::new(1, vec![advance], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let mut function = Function::new(1, "NEXT", 2, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    let mut far = Type::new(2, "far", TypeKind::Pointer, 4);
    far.address = AddressKind::Far;
    program.modules[0].types.push(far);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %1 = getelementptr i8, ptr addrspace(1) %0, i16 6\n  ret ptr addrspace(1) %1\n"), "{text}");
}

/// `CINT(x)`: BASIC rounds to nearest, ties to even, as `llvm.lrint` does.
#[test]
fn a_float_converted_to_an_integer_is_rounded() {
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 1 }];
    let convert = Instruction::new(1, Op::Convert, vec![2], vec![Operand::value_ref(1)]);
    let block = Block::new(1, vec![convert], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let mut function = Function::new(1, "ROUND%", 1, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    program.modules[0].types.push(Type::new(2, "double", TypeKind::Float, 8));

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %1 = call i16 @llvm.lrint.i16.f64(double %0)\n  ret i16 %1\n"), "{text}");
}

/// `FIX(x)`, Nib's `i16(x)`: TRUNCATE rounds toward zero, as `fptosi`
/// does, and an unsigned result by `fptoui`; it was `lrint`, which made
/// `i16(-7.9)` -8.
#[test]
fn a_float_truncated_to_an_integer_rounds_toward_zero() {
    for (signed, cast) in [(Some(true), "fptosi"), (Some(false), "fptoui")] {
        let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 3 }];
        let truncate = Instruction::new(1, Op::Truncate, vec![2], vec![Operand::value_ref(1)]);
        let block = Block::new(1, vec![truncate], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
        let mut function = Function::new(1, "FIX%", 3, values, Vec::new(), vec![block], 1);
        function.parameters = vec![1];
        let mut program = program(function);
        program.modules[0].types.push(Type::new(2, "double", TypeKind::Float, 8));
        let mut word = Type::new(3, "word", TypeKind::Integer, 2);
        word.signed = signed;
        program.modules[0].types.push(word);

        let emitted = emit(&program).remove(0);
        assert_eq!(emitted.refused, Vec::<(String, String)>::new());
        assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
        let text = llrm_mir::print::module(&emitted.module);
        assert!(text.contains(&format!("  %1 = {cast} double %0 to i16\n  ret i16 %1\n")), "{text}");
        assert!(!text.contains("lrint"), "{text}");
    }
}

/// `SQR(x)`: the float functions are LLVM's intrinsics.
#[test]
fn a_float_function_is_its_intrinsic() {
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 2 }];
    let root = Instruction::new(1, Op::Fsqrt, vec![2], vec![Operand::value_ref(1)]);
    let block = Block::new(1, vec![root], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let mut function = Function::new(1, "ROOT#", 2, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    program.modules[0].types.push(Type::new(2, "double", TypeKind::Float, 8));

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %1 = call double @llvm.sqrt.f64(double %0)\n  ret double %1\n"), "{text}");
}

/// A DATA statement in the first block marks the function's own entry as
/// an external entry, which every caller already takes.
#[test]
fn an_external_entry_at_the_entry_needs_nothing_more() {
    let mut function = difference();
    function.external_entries = vec![1];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
}

/// An unsigned byte index was scaled at its own width, mixing an `i8` with
/// an `i16` that both verifiers reject; an index is the pointer's index
/// width, extended as its type says.
#[test]
fn an_index_is_extended_to_the_pointers_index_width() {
    use crate::model::{ArrayElement, Place, Storage};
    let values = vec![Value { id: 1, r#type: 3 }, Value { id: 2, r#type: 1 }];
    let element = Operand::ArrayElement(ArrayElement { place: 1, indices: vec![Operand::value_ref(1)] });
    let load = Instruction::new(1, Op::Load, vec![2], vec![element]);
    let block = Block::new(1, vec![load], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let places = vec![Place::new(1, "A", 2, Storage::Local, -512)];
    let mut function = Function::new(1, "AT%", 1, values, places, vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    let mut array = Type::new(2, "array", TypeKind::Array, 512);
    (array.element, array.rank, array.bounds) = (Some(1), 1, vec![(0, 255)]);
    let mut byte = Type::new(3, "byte", TypeKind::Integer, 1);
    byte.signed = Some(false);
    program.modules[0].types.extend([array, byte]);

    let emitted = emit(&program).remove(0);
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %2 = zext i8 %0 to i16\n  %3 = sub i16 %2, 0\n  %4 = getelementptr inbounds i16, ptr %1, i16 %3\n"), "{text}");
}

/// `LEN(s)` of a heap string: its length is the word four bytes before its
/// data.
#[test]
fn a_descriptor_field_is_read_where_the_layout_puts_it() {
    use crate::model::{DescriptorField, DescriptorPlace};
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 1 }];
    let length = Operand::DescriptorPlace(DescriptorPlace { base: 1, field: DescriptorField::Length, r#type: 1 });
    let load = Instruction::new(1, Op::Load, vec![2], vec![length]);
    let block = Block::new(1, vec![load], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let mut function = Function::new(1, "LENGTH%", 1, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    let mut pointer = Type::new(2, "near*string", TypeKind::Pointer, 2);
    pointer.address = crate::model::AddressKind::Near;
    program.modules[0].types.push(pointer);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("  %1 = getelementptr i8, ptr %0, i16 -4\n  %2 = load i16, ptr %1\n"), "{text}");
}

/// Nib's 16.16 fixed point is the frontend's promise, `llvm.smul.fix` and
/// `llvm.sdiv.fix`: spelled as sext, a 64-bit multiply or divide, a shift
/// and a trunc, it counted five instructions to unroll's budget, and T075's
/// loops were refused.
#[test]
fn fixed_point_arithmetic_is_the_fixed_point_intrinsics() {
    let mut long = Type::new(2, "long", TypeKind::Integer, 4);
    long.signed = Some(true);
    let mut byte = Type::new(3, "byte", TypeKind::Integer, 1);
    byte.signed = Some(true);
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 2 }, Value { id: 3, r#type: 2 }, Value { id: 4, r#type: 2 }];
    let sixteen = Operand::constant(3, 16);
    let instructions = vec![
        Instruction::new(1, Op::FixedMul, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2), sixteen.clone()]),
        Instruction::new(2, Op::FixedDiv, vec![4], vec![Operand::value_ref(3), Operand::value_ref(2), sixteen]),
    ];
    let block = Block::new(1, instructions, Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(4)], Vec::new()));
    let mut function = Function::new(1, "SCALE", 2, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1, 2];
    let mut program = program(function);
    program.modules[0].types.extend([long, byte]);

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = "  %2 = call i32 @llvm.smul.fix.i32(i32 %0, i32 %1, i32 16)\n  %3 = call i32 @llvm.sdiv.fix.i32(i32 %2, i32 %1, i32 16)\n  ret i32 %3\n";
    assert!(text.contains(body), "{text}");
}

/// HIR's frame starts zeroed, so a local read before any store is 0; MIR's
/// allocas started uninitialized, which a pass may take as any value.
/// Places that overlap share their bytes: one alloca holds both, zeroed by
/// memset as clang zeroes an aggregate.
#[test]
fn locals_are_zeroed_and_overlapping_ones_share_an_alloca() {
    use crate::model::{Place, Storage};
    let mut long = Type::new(2, "long", TypeKind::Integer, 4);
    long.signed = Some(true);
    let values = vec![Value { id: 1, r#type: 1 }, Value { id: 2, r#type: 1 }];
    let instructions = vec![
        Instruction::new(1, Op::Load, vec![1], vec![Operand::place_ref(2)]),
        Instruction::new(2, Op::Load, vec![2], vec![Operand::place_ref(3)]),
    ];
    let block = Block::new(1, instructions, Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(2)], Vec::new()));
    let places = vec![Place::new(1, "L", 2, Storage::Local, -8), Place::new(2, "LOW", 1, Storage::Local, -8), Place::new(3, "X", 1, Storage::Local, -2)];
    let function = Function::new(1, "F%", 1, values, places, vec![block], 1);
    let mut program = program(function);
    program.modules[0].types.push(long);

    let emitted = emit(&program).remove(0);
    assert_eq!(llrm_mir::lint::poison(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let entry = "  %0 = alloca [4 x i8]\n  %1 = alloca i16\n  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)\n  store i16 0, ptr %1\n";
    assert!(text.contains(entry), "{text}");
}

/// A parameter's facts are its LLVM attributes, which LICM and EarlyCSE
/// ask; with none, a view descriptor's loads never left a loop.
#[test]
fn the_facts_of_a_parameter_become_its_attributes() {
    use crate::facts::{Builder, Subject};
    use llrm_mir::facts::Fact;
    let mut program = program(difference());
    let mut facts = Builder::new("test");
    let second = Subject::Param { function: 1, index: 1 };
    facts.state(second, Fact::NoAlias).state(second, Fact::ReadOnly).state(second, Fact::Dereferenceable(10)).state(second, Fact::NonNull);
    program.modules[0].facts = facts.finish();
    let text = llrm_mir::print::module(&emit(&program).remove(0).module);
    assert!(text.contains("(i16 %0, i16 noalias readonly dereferenceable(10) nonnull %1)"), "{text}");
}

/// A string comparison compares its callee's sign with zero; it was
/// refused, and with it every procedure of qbdemo that compares strings.
#[test]
fn a_string_comparison_compares_its_callees_sign() {
    use crate::model::{CallAbi, CallDistance, FloatReturn, StackCleanup};
    let mut function = difference();
    let mut compare = Instruction::new(1, Op::StringGt, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2)]);
    compare.callee = Some("B$SCMP".to_owned());
    function.blocks[0].instructions = vec![compare];
    function.calls = vec![CallAbi { instruction: 1, order: vec![0, 1], cleanup: StackCleanup::Callee, distance: CallDistance::Far, callee: None, float_return: FloatReturn::Register }];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("%2 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(i16 %0, i16 %1)\n  %3 = icmp sgt i16 %2, 0\n  %4 = sext i1 %3 to i16"), "{text}");
}

/// A string comparison's callee is stated a three-way compare: its sign
/// says which string is greater, nothing of how often. Without it
/// branchprob read `a$ > b$` as a likely `x > 0`.
#[test]
fn a_string_comparisons_callee_is_a_three_way_compare() {
    use crate::model::{CallAbi, CallDistance, FloatReturn, StackCleanup};
    let mut function = difference();
    let mut compare = Instruction::new(1, Op::StringGt, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2)]);
    compare.callee = Some("B$SCMP".to_owned());
    function.blocks[0].instructions = vec![compare];
    function.calls = vec![CallAbi { instruction: 1, order: vec![0, 1], cleanup: StackCleanup::Callee, distance: CallDistance::Far, callee: None, float_return: FloatReturn::Register }];
    let emitted = emit(&program(function)).remove(0);
    let module = &emitted.module;
    let callee = module.global(module.named("llrm.qb.B$SCMP").expect("declared")).function().expect("a function");
    assert!(llrm_mir::facts::Facts::of(&callee.attrs).three_way_compare(), "{}", llrm_mir::print::module(module));
}

/// A port read and write are calls of the target's port intrinsics, the
/// port a word; they were refused, and with them every procedure that
/// sets the palette.
#[test]
fn ports_are_the_targets_intrinsics() {
    let mut function = difference();
    let read = Instruction::new(1, Op::PortIn, vec![3], vec![Operand::value_ref(1)]);
    let write = Instruction::new(2, Op::PortOut, vec![], vec![Operand::value_ref(1), Operand::value_ref(3)]);
    function.blocks[0].instructions = vec![read, write];
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("%2 = call i16 @llrm.ia16.in.i16(i16 %0)\n  call void @llrm.ia16.out.i16(i16 %0, i16 %2)"), "{text}");
}

/// A module calling DEF SEG, INKEY$ and RUN, with b$seg named, linked
/// against the runtime `promises` states: its runtime module's text and
/// the linked module's.
fn promised(promises: &crate::model::RuntimePromises) -> (String, String) {
    use crate::mir::{emit, runtime};
    use crate::model::{DataLinkage, DataObject};
    let call = |id, callee: &str| {
        let mut call = Instruction::new(id, Op::Call, Vec::new(), Vec::new());
        call.callee = Some(callee.to_owned());
        call
    };
    let mut function = difference();
    function.blocks[0].instructions.extend([call(2, "B$DSEG"), call(3, "B$INKY"), call(4, "B$RUN")]);
    let mut program = program(function);
    let mut segment = DataObject::new(3, "b$seg", vec![0, 0]);
    (segment.linkage, segment.addressed) = (DataLinkage::External, false);
    program.modules[0].data = vec![segment];
    let emitted = emit(&program).remove(0);
    let runtime = runtime(&[(&emitted, &program.modules[0])], promises).unwrap();
    let text = llrm_mir::print::module(&runtime);
    assert_eq!(llrm_mir::print::module(&llrm_mir::parse::module(&text).unwrap()), text);
    let linked = llrm_mir::program::Program::new(vec![emitted.module], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap().with_runtime(runtime).unwrap();
    assert_eq!(llrm_mir::verify::verify(&linked.modules[0]), Vec::<String>::new());
    (text, llrm_mir::print::module(&linked.modules[0]))
}

/// `routine`'s declaration in `text`.
fn declaration<'t>(text: &'t str, routine: &str) -> &'t str {
    text.lines().find(|line| line.starts_with("declare") && line.contains(&format!("@llrm.qb.{routine}("))).unwrap_or_else(|| panic!("{text}"))
}

/// The runtime's promise was the old raise's `WRITERS` table alone, so the
/// rich MIR took every routine to write b$seg and to run program code. It
/// is stated in the runtime module and so on the declarations: DEF SEG
/// writes b$seg, INKEY$ writes none of the named cells, and a routine that
/// may run program code promises nothing.
#[test]
fn a_runtime_promise_is_stated_on_its_routines() {
    let (runtime, text) = promised(&crate::model::RuntimePromises::of(["B$RUN"], [("b$seg", ["B$DSEG"])], []));
    assert!(runtime.contains("!llrm.named = !{!0}\n!llrm.writes = !{!1, !2}\n"), "{runtime}");
    assert!(runtime.contains("!0 = !{ptr @b$seg}\n!1 = !{ptr addrspace(1) @llrm.qb.B$DSEG, ptr @b$seg}\n!2 = !{ptr addrspace(1) @llrm.qb.B$INKY}\n"), "{runtime}");
    for (routine, promised) in [("B$DSEG", true), ("B$INKY", true), ("B$RUN", false)] {
        assert_eq!(declaration(&text, routine).contains("nocallback"), promised, "{text}");
    }
}

/// The HIR emitter had no fact to state `nounwind` by, so a call to a
/// routine that raises no error was taken to unwind.
#[test]
fn a_routine_that_raises_no_error_is_nounwind() {
    let (_, text) = promised(&crate::model::RuntimePromises::of(["B$RUN"], [("b$seg", ["B$DSEG"])], ["B$INKY", "B$RUN"]));
    for (routine, promised) in [("B$DSEG", false), ("B$INKY", true), ("B$RUN", true)] {
        assert_eq!(declaration(&text, routine).contains("nounwind"), promised, "{text}");
    }
}

/// A value defined in a block the HIR lists after its use's block, though
/// it dominates it -- UBOUND's ranking block does. Emitting blocks in HIR
/// order refused the function: "value 3 used before its definition".
#[test]
fn a_value_is_emitted_before_a_use_listed_ahead_of_it() {
    let mut function = difference();
    let sub = function.blocks[0].instructions.remove(0);
    let jump = |to| Terminator::new(TerminatorKind::Jump, Vec::new(), vec![to]);
    let ret = std::mem::replace(&mut function.blocks[0].terminator, jump(3));
    function.blocks.push(Block::new(2, Vec::new(), ret));
    function.blocks.push(Block::new(3, vec![sub], jump(2)));
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
}

/// A FOR loop's step promises its counter fits, and the rich route dropped
/// the promise: its add reached MIR without `nsw`.
#[test]
fn a_no_signed_wrap_fact_is_nsw() {
    use crate::facts::{Builder, Subject};
    let mut function = difference();
    function.blocks[0].instructions[0].op = Op::Add;
    let mut program = program(function);
    let mut facts = Builder::new("test");
    facts.state(Subject::Instruction { function: 1, id: 1 }, llrm_mir::facts::Fact::NoSignedWrap);
    program.modules[0].facts = facts.finish();
    let text = llrm_mir::print::module(&emit(&program).remove(0).module);
    assert!(text.contains("%2 = add nsw i16 %0, %1"), "{text}");
}

/// C's promises: a type's aliasing class tags its accesses, pointer
/// arithmetic stays inbounds, truth is one, a restrict parameter has no
/// size, locals start indeterminate, and a value-less return gives poison.
#[test]
fn a_languages_promises_reach_mir() {
    use crate::model::{AliasClass, IndirectPlace, Place, Storage};
    let mut boolean = Type::new(2, "bool", TypeKind::Boolean, 2);
    boolean.signed = Some(false);
    let values = vec![Value { id: 1, r#type: 3 }, Value { id: 2, r#type: 3 }, Value { id: 3, r#type: 1 }, Value { id: 4, r#type: 2 }];
    let mut advance = Instruction::new(1, Op::PtrOffset, vec![2], vec![Operand::value_ref(1), Operand::constant(1, 2)]);
    let at = Operand::IndirectPlace(IndirectPlace { base: 2, offset: 0, r#type: 1, volatile: false, origin: None, allocation: None });
    let instructions = vec![
        advance,
        Instruction::new(2, Op::Load, vec![3], vec![at]),
        Instruction::new(3, Op::Lt, vec![4], vec![Operand::value_ref(3), Operand::constant(1, 0)]),
        Instruction::new(4, Op::Store, vec![], vec![Operand::place_ref(1), Operand::value_ref(4)]),
    ];
    let block = Block::new(1, instructions, Terminator::new(TerminatorKind::Return, Vec::new(), Vec::new()));
    let mut function = Function::new(1, "f", 1, values, vec![Place::new(1, "t", 2, Storage::Local, 0)], vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    program.zeroed_locals = false;
    program.modules[0].facts = vec![
        crate::facts::Stated { subject: crate::facts::Subject::Param { function: 1, index: 0 }, fact: llrm_mir::facts::Fact::NoAlias, source: None },
        crate::facts::Stated { subject: crate::facts::Subject::Instruction { function: 1, id: 1 }, fact: llrm_mir::facts::Fact::InBounds, source: None },
    ];
    program.modules[0].types.extend([boolean, Type::new(3, "near", TypeKind::Pointer, 2)]);
    program.modules[0].alias_classes = vec![
        AliasClass { name: "root".to_owned(), parent: None, types: Vec::new() },
        AliasClass { name: "int2".to_owned(), parent: Some("root".to_owned()), types: vec![1] },
    ];

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = "(ptr noalias %0) addrspace(1) {\nb1:\n  %1 = alloca i16\n  %2 = getelementptr inbounds i8, ptr %0, i16 2\n  %3 = load i16, ptr %2, !tbaa !7\n  %4 = icmp slt i16 %3, 0\n  %5 = zext i1 %4 to i16\n  store i16 %5, ptr %1, !tbaa !2\n  ret i16 poison\n}";
    assert!(text.contains(body), "{text}");
    assert!(text.contains("!6 = !{!\"int2\", !5, i64 0}\n!7 = !{!6, !6, i64 0}"), "{text}");
}

/// An exported object is defined, a literal private, and another module's
/// object of no bytes only declared; each keeps its alignment.
#[test]
fn data_linkage_is_the_languages() {
    use crate::model::{DataLinkage, DataObject};
    let mut program = program(difference());
    let mut exported = DataObject::new(1, "shown", vec![1, 0]);
    exported.linkage = DataLinkage::Exported;
    let mut literal = DataObject::new(2, "L", vec![65, 0]);
    (literal.linkage, literal.readonly) = (DataLinkage::Private, true);
    let mut imported = DataObject::new(3, "elsewhere", Vec::new());
    imported.linkage = DataLinkage::External;
    program.modules[0].data = vec![exported, literal, imported];
    program.modules[0].facts = vec![crate::facts::Stated { subject: crate::facts::Subject::Object(1), fact: llrm_mir::facts::Fact::Align(2), source: None }];

    let text = llrm_mir::print::module(&emit(&program).remove(0).module);
    assert!(text.contains("@shown = global [2 x i8] c\"\\01\\00\", align 2\n@L = private constant [2 x i8] c\"A\\00\"\n@elsewhere = external global [0 x i8]\n"), "{text}");
}

/// A function's address, called through: C's function pointers.
#[test]
fn a_call_through_a_functions_address() {
    use crate::model::{CallAbi, CallDistance, FloatReturn, StackCleanup};
    let mut function = difference();
    let mut address = Instruction::new(2, Op::Address, vec![4], Vec::new());
    address.callee = Some("DIFF%".to_owned());
    let call = Instruction::new(3, Op::Call, vec![5], vec![Operand::value_ref(4), Operand::value_ref(3), Operand::value_ref(1)]);
    function.blocks[0].instructions.extend([address, call]);
    function.blocks[0].terminator.operands = vec![Operand::value_ref(5)];
    function.values.extend([Value { id: 4, r#type: 2 }, Value { id: 5, r#type: 1 }]);
    function.calls = vec![CallAbi { instruction: 3, order: vec![1, 0], cleanup: StackCleanup::Caller, distance: CallDistance::Far, callee: None, float_return: FloatReturn::Register }];
    let mut program = program(function);
    program.modules[0].types.push(Type::new(2, "far", TypeKind::Pointer, 4));

    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("%3 = call addrspace(1) i16 @\"DIFF%\"(i16 %2, i16 %0)"), "{text}");
}

/// A routine that only reads what its arguments reach: C's strlen.
#[test]
fn a_routine_reading_its_arguments_is_argmem_read() {
    let module = llrm_mir::parse::module("declare i16 @_strlen(ptr)\ndeclare void @_puts(ptr)\n").unwrap();
    let promises = crate::model::RuntimePromises { reads_arguments: vec!["_strlen".to_owned()], ..Default::default() };
    let runtime = llrm_mir::print::module(&crate::mir::promised(&[(&module, std::collections::HashMap::new())], &promises).unwrap());
    assert!(runtime.contains("declare i16 @_strlen(ptr nocapture) memory(argmem: read)\n") && !runtime.contains("puts"), "{runtime}");
}

/// The blocks of `function` reached from `from`.
fn reached(function: &Function, from: impl IntoIterator<Item = i64>) -> std::collections::BTreeSet<i64> {
    let mut seen = std::collections::BTreeSet::new();
    let mut pending: Vec<i64> = from.into_iter().collect();
    while let Some(id) = pending.pop() {
        let Some(block) = function.blocks.iter().find(|one| one.id == id) else { continue };
        if seen.insert(id) {
            pending.extend(block.terminator.targets.iter().chain(block.terminator.cases.iter().map(|(_, to)| to)).copied());
        }
    }
    seen
}

/// qb-qrender's main.bas falls from its body into its ON ERROR handler's
/// label, as BASIC allows, and the rich route refused it: "@__main: block
/// 2, which both the body and its error handler run". Emitted in both, a
/// shared block is ordinary code in the body: ERL there reads the line the
/// handler last took, RESUME there raises "RESUME without error" (20), and
/// the handler run to the module's end raises "No RESUME" (19).
#[test]
fn a_block_the_body_and_its_module_handler_share_is_emitted_in_both() {
    // `10 ON ERROR GOTO 100: 20 ERROR 5: 30 PRINT "body"; ERL`
    // `100 PRINT "h"; ERR; ERL: 110 IF ERR = 0 THEN RESUME NEXT`
    let program = crate::codec::decode(include_str!("fixtures/handler_fallthrough.json")).expect("decodes");
    let main = program.modules[0].functions.iter().find(|one| one.name == "__main").expect("the module body");
    let handler = main.error_handler.expect("a module handler");
    // The body runs from its entry and from where RESUME continues it.
    let theirs = reached(main, [handler]);
    let body = reached(main, std::iter::once(main.entry).chain(main.external_entries.iter().copied().filter(|one| !theirs.contains(one))));
    let shared = &body & &theirs;
    assert!(!shared.is_empty(), "the fixture no longer falls into its handler");
    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
    let text = llrm_mir::print::module(&emitted.module);
    let body = text.split("define ").find(|one| one.contains("@__main()")).expect("the body");
    let outlined = text.split("define ").find(|one| one.contains("@__main$handler(i16 %0, i16 %1)")).expect("the handler");
    assert!(body.contains("@llrm.qb.B$SERR(i16 20)") && body.contains("load i16, ptr @$QB$ERL\n"), "{body}");
    assert!(outlined.contains("@llrm.qb.B$SERR(i16 19)"), "{outlined}");
    // RESUME clears ERL on VBDOS, as legacy prints `h 0 0` after it; QB 4.5
    // keeps it, `h 0 20`.
    let cleared = |runtime| {
        let program = crate::model::Program { runtime, ..program.clone() };
        let text = llrm_mir::print::module(&emit(&program).remove(0).module);
        text.split("define ").find(|one| one.contains("@__main$handler(i16 %0, i16 %1)")).expect("the handler").contains("store i16 0, ptr @$QB$ERL")
    };
    assert!(cleared(RuntimeProfile::Vbdos) && !cleared(RuntimeProfile::Qb45));
}

/// The emission input a fixture holds, as llrm-qb hands it on.
fn fixture(text: &str) -> Program {
    crate::codec::decode(text).expect("decodes")
}

/// `name`'s function in `program`'s module.
fn function<'p>(program: &'p Program, name: &str) -> &'p Function {
    program.modules[0].functions.iter().find(|one| one.name == name).expect("the function")
}

fn emits(program: &Program) {
    let emitted = emit(program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    assert_eq!(llrm_mir::verify::verify(&emitted.module), Vec::<String>::new());
}

/// GORILLA.BAS's DATA blocks are no entry once their rows are laid out,
/// yet the body emits them, and one ran into the module's end, a statement
/// the handler reaches too: left out of the body, "no entry found for key".
#[test]
fn a_block_nothing_enters_still_leads_the_body() {
    let program = fixture(include_str!("fixtures/handler_data_block.json"));
    let main = function(&program, "__main");
    let theirs = reached(main, [main.error_handler.expect("a handler")]);
    let entered = reached(main, std::iter::once(main.entry).chain(main.external_entries.iter().copied()));
    // Premise: a block nothing enters jumps into the handler's.
    let orphan = main.blocks.iter().find(|one| !entered.contains(&one.id) && !theirs.contains(&one.id) && one.terminator.targets.iter().any(|to| theirs.contains(to)));
    assert!(orphan.is_some(), "the fixture no longer has the shape");
    emits(&program);
}

/// GORILLA.BAS's RESUME continued at a statement the handler also runs,
/// which the body had left out: "a RESUME into block 5, the error
/// handler's".
#[test]
fn a_statement_the_handler_runs_is_the_bodys_too() {
    let program = fixture(include_str!("fixtures/handler_statement_entry.json"));
    let main = function(&program, "__main");
    let theirs = reached(main, [main.error_handler.expect("a handler")]);
    // Premise: RESUME may continue at a statement the handler runs.
    let rows = program.modules[0].statements().expect("a statement table");
    assert!(rows.iter().any(|one| one.function == main.id && theirs.contains(&one.block)), "the fixture no longer has the shape");
    emits(&program);
}

/// GORILLA.BAS's PlayGame erases its arrays after its last statement row,
/// END SUB's, and RESUME NEXT after that erase had nowhere to go: "a
/// RESUME NEXT past the last statement". It continues at END SUB.
#[test]
fn resume_next_past_end_sub_continues_at_its_end() {
    let program = fixture(include_str!("fixtures/resume_past_end_sub.json"));
    let sub = function(&program, "S");
    let rows = program.modules[0].statements().expect("a statement table");
    let last = rows.iter().filter(|one| one.function == sub.id).map(|one| one.instruction).max().expect("rows");
    // Premise: a call after the last statement row begins.
    assert!(sub.calls.iter().any(|one| one.instruction > last), "the fixture no longer has the shape");
    emits(&program);
}

/// A stated fact becomes its carrier on what it is stated of, and nowhere
/// else; `noalias` is spelled only by `llrm_mir::facts`.
#[test]
fn a_stated_fact_becomes_its_carrier() {
    use crate::facts::{Builder, Subject};
    use llrm_mir::facts::Fact;
    let mut program = program(difference());
    let mut facts = Builder::new("test");
    facts.state(Subject::Param { function: 1, index: 1 }, Fact::NoAlias);
    program.modules[0].facts = facts.finish();
    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("(i16 %0, i16 noalias %1)"), "{text}");
}

/// A fact of a kind it is not stated of was lowered to whatever the subject
/// was; the verifier refuses it, as it does a subject the module lacks.
#[test]
fn a_fact_of_the_wrong_subject_is_refused() {
    use crate::facts::{Builder, Stated, Subject};
    use llrm_mir::facts::Fact;
    let refusal = |subject| {
        let mut program = program(difference());
        program.modules[0].facts = vec![Stated { subject, fact: Fact::NoAlias, source: None }];
        crate::verify::verify(&program).unwrap_err().0
    };
    assert!(refusal(Subject::Callable(1)).contains("noalias is not stated of a callable"));
    assert!(refusal(Subject::Param { function: 1, index: 2 }).contains("noalias is stated of a param the module lacks"));
    assert!(refusal(Subject::Param { function: 9, index: 0 }).contains("the module lacks"));
    let mut program = program(difference());
    let mut facts = Builder::new("test");
    facts.state(Subject::Param { function: 1, index: 0 }, Fact::NoAlias);
    program.modules[0].facts = facts.finish();
    assert!(crate::verify::verify(&program).is_ok());
}

/// A freedom of floating arithmetic stated of an integer subtraction, or a
/// wrap fact of a floating add, was lowered to a flag on an instruction it
/// means nothing for; the verifier refuses both, and takes the right pairs.
#[test]
fn a_float_freedom_or_a_wrap_fact_of_the_wrong_operation_is_refused() {
    use crate::facts::{Stated, Subject};
    use llrm_mir::facts::Fact;
    // `difference` is two integers' sub; `floats` two doubles' fadd.
    let floats = || {
        let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 2 }, Value { id: 3, r#type: 2 }];
        let add = Instruction::new(1, Op::Fadd, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2)]);
        let block = Block::new(1, vec![add], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(3)], Vec::new()));
        let mut function = Function::new(1, "f", 2, values, Vec::new(), vec![block], 1);
        function.parameters = vec![1, 2];
        let mut program = program(function);
        let mut double = Type::new(2, "double", TypeKind::Float, 8);
        double.evaluation = crate::model::FloatEvaluation::Binary64;
        program.modules[0].types.push(double);
        program
    };
    let stated = |mut program: crate::model::Program, fact| {
        program.modules[0].facts = vec![Stated { subject: Subject::Instruction { function: 1, id: 1 }, fact, source: None }];
        crate::verify::verify(&program).map_err(|error| error.0)
    };
    for fact in [Fact::Reassoc, Fact::NoNaNs, Fact::NoInfs, Fact::NoSignedZeros, Fact::AllowReciprocal] {
        assert!(stated(program(difference()), fact).unwrap_err().contains("is stated of an instruction that is no floating operation"), "{}", fact.key());
        assert_eq!(stated(floats(), fact), Ok(()), "{}", fact.key());
    }
    for fact in [Fact::NoSignedWrap, Fact::NoUnsignedWrap] {
        assert!(stated(floats(), fact).unwrap_err().contains("is stated of an instruction that is no integer add, sub or mul"), "{}", fact.key());
        assert_eq!(stated(program(difference()), fact), Ok(()), "{}", fact.key());
    }
}

/// Stated facts cross the wire and come back the same, source and all.
#[test]
fn stated_facts_survive_the_codec() {
    use crate::facts::{Builder, Subject};
    use llrm_mir::facts::Fact;
    let mut program = program(difference());
    let mut facts = Builder::new("c");
    facts.state_at(Subject::Param { function: 1, index: 0 }, Fact::NoAlias, 12);
    program.modules[0].facts = facts.finish();
    let text = crate::codec::encode(&program, None).unwrap();
    assert!(text.contains("\"facts\":[{\"fact\":\"noalias\",\"function\":1,\"id\":0,\"source\":\"c:12\",\"subject\":\"param\"}]"), "{text}");
    assert_eq!(crate::codec::decode(&text).unwrap().modules[0].facts, program.modules[0].facts);
}

/// Schema 1 had `promises` and `nowrap`, which no longer exist; a program
/// that says it is schema 1 is refused by its version, not by whichever
/// field the decoder meets first.
#[test]
fn a_program_of_the_old_schema_is_refused_by_its_version() {
    let mut program = program(difference());
    program.schema = 1;
    assert!(crate::verify::verify(&program).unwrap_err().0.contains("unsupported HIR schema 1"));
}

/// The same, as JSON: the old schema's `promises` field was what it was
/// refused for.
#[test]
fn old_json_is_refused_by_its_schema() {
    let text = crate::codec::encode(&program(difference()), None).unwrap().replace("\"schema\":5", "\"promises\":[],\"schema\":1");
    assert!(crate::codec::decode(&text).unwrap_err().0.contains("unsupported HIR schema 1"));
}

/// A fact of a data object the module lacks is refused, as one of a
/// parameter it lacks is.
#[test]
fn an_alignment_of_no_object_is_refused() {
    use crate::facts::{Stated, Subject};
    let mut program = program(difference());
    program.modules[0].facts = vec![Stated { subject: Subject::Object(9), fact: llrm_mir::facts::Fact::Align(2), source: None }];
    assert!(crate::verify::verify(&program).unwrap_err().0.contains("object the module lacks"));
}

/// What the language promises of a call's pointer argument is stated of that
/// argument and reaches the call as its attributes: the callee writes the
/// first bytes before reading any, reads none and keeps no copy.
#[test]
fn facts_of_a_call_argument_are_its_call_site_attributes() {
    use crate::facts::{Builder, Subject};
    use crate::model::{CallAbi, CallDistance, FloatReturn, StackCleanup};
    use llrm_mir::facts::Fact;
    let values = vec![Value { id: 1, r#type: 1 }];
    let mut call = Instruction::new(1, Op::Call, Vec::new(), vec![Operand::value_ref(1)]);
    call.callee = Some("B$FILL".to_owned());
    let block = Block::new(1, vec![call], Terminator::new(TerminatorKind::Return, Vec::new(), Vec::new()));
    let mut function = Function::new(1, "f", 0, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    function.calls = vec![CallAbi { instruction: 1, order: vec![0], cleanup: StackCleanup::Callee, distance: CallDistance::Far, callee: None, float_return: FloatReturn::Register }];
    let mut program = program(function);
    let mut facts = Builder::new("test");
    let argument = Subject::Operand { function: 1, instruction: 1, operand: 0 };
    facts.state(argument, Fact::NoCapture).state(argument, Fact::WriteOnly).state(argument, Fact::Initializes(4));
    program.modules[0].facts = facts.finish();
    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("@llrm.qb.B$FILL(i16 nocapture writeonly initializes((0, 4)) %0)"), "{text}");
}

/// A fact of an operand the instruction lacks is refused.
#[test]
fn a_fact_of_an_operand_the_instruction_lacks_is_refused() {
    use crate::facts::{Stated, Subject};
    let mut program = program(difference());
    program.modules[0].facts = vec![Stated { subject: Subject::Operand { function: 1, instruction: 1, operand: 0 }, fact: llrm_mir::facts::Fact::NoCapture, source: None }];
    assert!(crate::verify::verify(&program).unwrap_err().0.contains("operand the module lacks"));
}

/// An access through a pointer is inbounds when the language says of that
/// operand that it is; the next access, of the same place, is not.
#[test]
fn inbounds_is_a_fact_of_an_operand() {
    use crate::facts::{Builder, Subject};
    use crate::model::IndirectPlace;
    use llrm_mir::facts::Fact;
    let at = || Operand::IndirectPlace(IndirectPlace { base: 1, offset: 2, r#type: 1, volatile: false, origin: None, allocation: None });
    let values = vec![Value { id: 1, r#type: 3 }, Value { id: 2, r#type: 1 }, Value { id: 3, r#type: 1 }];
    let loads = vec![Instruction::new(1, Op::Load, vec![2], vec![at()]), Instruction::new(2, Op::Load, vec![3], vec![at()])];
    let block = Block::new(1, loads, Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(3)], Vec::new()));
    let mut function = Function::new(1, "f", 1, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1];
    let mut program = program(function);
    program.modules[0].types.push(Type::new(3, "near", TypeKind::Pointer, 2));
    let mut facts = Builder::new("test");
    facts.state(Subject::Operand { function: 1, instruction: 1, operand: 0 }, Fact::InBounds);
    program.modules[0].facts = facts.finish();
    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert_eq!(text.matches("getelementptr inbounds i8, ptr %0, i16 2").count(), 1, "{text}");
    assert_eq!(text.matches("getelementptr i8, ptr %0, i16 2").count(), 1, "{text}");
}

/// What a language lets a pass do to a floating operation reaches the
/// instruction as its fast-math flags: `-on` and alternate math were stated
/// nowhere, so no float fold could ask.
#[test]
fn floating_freedoms_are_fast_math_flags() {
    use crate::facts::{Builder, Subject};
    use llrm_mir::facts::Fact;
    let values = vec![Value { id: 1, r#type: 2 }, Value { id: 2, r#type: 2 }, Value { id: 3, r#type: 2 }];
    let add = Instruction::new(1, Op::Fadd, vec![3], vec![Operand::value_ref(1), Operand::value_ref(2)]);
    let block = Block::new(1, vec![add], Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(3)], Vec::new()));
    let mut function = Function::new(1, "f", 2, values, Vec::new(), vec![block], 1);
    function.parameters = vec![1, 2];
    let mut program = program(function);
    program.modules[0].types.push(Type::new(2, "double", TypeKind::Float, 8));
    let mut facts = Builder::new("test");
    for fact in [Fact::Reassoc, Fact::NoNaNs, Fact::NoInfs, Fact::NoSignedZeros, Fact::AllowReciprocal] {
        facts.state(Subject::Instruction { function: 1, id: 1 }, fact);
    }
    program.modules[0].facts = facts.finish();
    let emitted = emit(&program).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let text = llrm_mir::print::module(&emitted.module);
    assert!(text.contains("= fadd reassoc nnan ninf nsz arcp double %0, %1"), "{text}");
}
