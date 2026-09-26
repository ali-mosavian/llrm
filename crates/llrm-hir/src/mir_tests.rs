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
    assert!(text.ends_with("define i16 @\"DIFF%\"(i16 %0, i16 %1) {\nb1:\n  %2 = sub i16 %0, %1\n  ret i16 %2\n}\n"), "{text}");
}

/// A refused internal function was left `declare internal`, which LLVM
/// rejects; as LLVM's `deleteBody` does, it becomes external.
#[test]
fn a_refused_function_is_an_external_declaration() {
    let mut function = difference();
    function.linkage = FunctionLinkage::Internal;
    function.error_handler = Some(1);
    let emitted = emit(&program(function)).remove(0);
    assert_eq!(emitted.refused, [("DIFF%".to_owned(), "an ON ERROR handler".to_owned())]);
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
    let body = "  %2 = alloca [30 x i8]\n  %3 = sub i16 %1, 0\n  %4 = sub i16 %0, 1\n  %5 = mul i16 %3, 3\n  %6 = add i16 %5, %4\n  %7 = getelementptr inbounds i16, ptr %2, i16 %6\n  %8 = load i16, ptr %7\n  ret i16 %8\n";
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
