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
