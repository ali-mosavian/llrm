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
